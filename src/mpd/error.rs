use std::{fmt, io};

/// An `ACK` error returned by the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ack {
    pub code: u32,
    /// Index of the failing command inside a command list.
    pub index: u32,
    pub command: String,
    pub message: String,
}

impl fmt::Display for Ack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.command.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.command, self.message)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MpdError {
    #[error("connection error: {0}")]
    Io(#[from] io::Error),
    #[error("MPD error: {0}")]
    Ack(Ack),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("connection closed by server")]
    Closed,
}

impl MpdError {
    /// Whether the connection is unusable after this error.
    pub fn is_fatal(&self) -> bool {
        !matches!(self, Self::Ack(_))
    }
}

pub type MpdResult<T> = Result<T, MpdError>;
