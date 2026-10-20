//! An asynchronous MPD connection over TCP or a Unix socket.

use std::{fmt, path::PathBuf};

use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufStream},
    net::TcpStream,
};

use super::{
    error::{MpdError, MpdResult},
    protocol::{self, Command, Line, Pair},
};

/// Where the server listens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Address {
    /// `host:port`, with IPv6 hosts in brackets.
    Tcp(String),
    Unix(PathBuf),
}

impl Address {
    /// Whether the server runs on this machine.
    pub fn is_local(&self) -> bool {
        match self {
            Self::Unix(_) => true,
            Self::Tcp(addr) => {
                let host = addr
                    .rsplit_once(':')
                    .map_or(addr.as_str(), |(host, _)| host)
                    .trim_start_matches('[')
                    .trim_end_matches(']');
                matches!(host, "localhost" | "127.0.0.1" | "::1") || host.starts_with("127.")
            }
        }
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tcp(addr) => f.write_str(addr),
            Self::Unix(path) => write!(f, "{}", path.display()),
        }
    }
}

trait Socket: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Socket for T {}

pub struct Connection {
    stream: BufStream<Box<dyn Socket>>,
    version: String,
    line: Vec<u8>,
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl Connection {
    /// Connects, reads the greeting and authenticates if `password` is set.
    pub async fn connect(address: &Address, password: Option<&str>) -> MpdResult<Self> {
        let socket: Box<dyn Socket> = match address {
            Address::Tcp(addr) => {
                let stream = TcpStream::connect(addr).await?;
                stream.set_nodelay(true)?;
                Box::new(stream)
            }
            #[cfg(unix)]
            Address::Unix(path) => Box::new(tokio::net::UnixStream::connect(path).await?),
            #[cfg(not(unix))]
            Address::Unix(_) => {
                return Err(MpdError::Protocol(
                    "Unix sockets are not supported on this platform".into(),
                ));
            }
        };
        let mut conn = Self {
            stream: BufStream::new(socket),
            version: String::new(),
            line: Vec::new(),
        };
        let greeting = conn.read_line().await?;
        conn.version = protocol::parse_greeting(&greeting)?;
        if let Some(password) = password.filter(|p| !p.is_empty()) {
            conn.execute(&Command::new("password").arg(password))
                .await?;
        }
        Ok(conn)
    }

    /// The protocol version announced by the server.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Runs one command and collects its response.
    pub async fn execute(&mut self, command: &Command) -> MpdResult<Vec<Pair>> {
        self.send(command.as_str()).await?;
        self.read_response().await
    }

    /// Runs commands atomically as a command list, returning one response per
    /// command.
    pub async fn execute_list(&mut self, commands: &[Command]) -> MpdResult<Vec<Vec<Pair>>> {
        let mut request = String::from("command_list_ok_begin\n");
        for command in commands {
            request.push_str(command.as_str());
            request.push('\n');
        }
        request.push_str("command_list_end");
        self.send(&request).await?;

        let mut responses = Vec::with_capacity(commands.len());
        let mut current = Vec::new();
        loop {
            let line = self.read_line().await?;
            match protocol::parse_line(&line)? {
                Line::Pair(pair) => current.push(pair),
                Line::ListOk => responses.push(std::mem::take(&mut current)),
                Line::Ok => return Ok(responses),
            }
        }
    }

    /// Runs commands as a list when there is more than one, discarding output.
    pub async fn execute_all(&mut self, commands: &[Command]) -> MpdResult<()> {
        match commands {
            [] => Ok(()),
            [command] => self.execute(command).await.map(drop),
            _ => self.execute_list(commands).await.map(drop),
        }
    }

    async fn send(&mut self, request: &str) -> MpdResult<()> {
        self.stream.write_all(request.as_bytes()).await?;
        self.stream.write_all(b"\n").await?;
        self.stream.flush().await?;
        Ok(())
    }

    async fn read_response(&mut self) -> MpdResult<Vec<Pair>> {
        let mut pairs = Vec::new();
        loop {
            let line = self.read_line().await?;
            match protocol::parse_line(&line)? {
                Line::Pair(pair) => pairs.push(pair),
                Line::Ok => return Ok(pairs),
                Line::ListOk => {
                    return Err(MpdError::Protocol("unexpected list_OK".into()));
                }
            }
        }
    }

    async fn read_line(&mut self) -> MpdResult<String> {
        self.line.clear();
        if self.stream.read_until(b'\n', &mut self.line).await? == 0 {
            return Err(MpdError::Closed);
        }
        if self.line.last() == Some(&b'\n') {
            self.line.pop();
        }
        // Tags are UTF-8 by protocol; tolerate broken files instead of failing.
        Ok(String::from_utf8_lossy(&self.line).into_owned())
    }
}
