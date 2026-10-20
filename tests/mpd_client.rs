//! Talks to a scripted fake MPD server over real TCP sockets.

use std::time::Duration;

use rmptui::mpd::{
    self, Address, Command, Connection, MpdError, MpdEvent, Query, Reply, Request, Subsystem,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::mpsc,
    time::timeout,
};

/// Serves one connection: greets, then answers each request line with the
/// response produced by `respond`. Received lines are forwarded to `seen`.
async fn fake_server<F>(respond: F) -> (Address, mpsc::UnboundedReceiver<String>)
where
    F: Fn(&str) -> String + Send + Sync + Clone + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = Address::Tcp(listener.local_addr().unwrap().to_string());
    let (seen_tx, seen_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            let respond = respond.clone();
            let seen = seen_tx.clone();
            tokio::spawn(async move {
                let (read, mut write) = socket.into_split();
                write.write_all(b"OK MPD 0.24.0\n").await.unwrap();
                let mut lines = BufReader::new(read).lines();
                let mut list: Option<Vec<String>> = None;
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = seen.send(line.clone());
                    let response = match (line.as_str(), list.as_mut()) {
                        ("command_list_ok_begin", _) => {
                            list = Some(Vec::new());
                            continue;
                        }
                        ("command_list_end", Some(commands)) => {
                            let mut out = String::new();
                            for command in commands.iter() {
                                let body = respond(command);
                                out.push_str(body.strip_suffix("OK\n").unwrap_or(&body));
                                out.push_str("list_OK\n");
                            }
                            list = None;
                            out + "OK\n"
                        }
                        (_, Some(commands)) => {
                            commands.push(line);
                            continue;
                        }
                        // Block forever on idle, like a quiet server.
                        ("idle", None) => std::future::pending().await,
                        (_, None) => respond(&line),
                    };
                    if write.write_all(response.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    (address, seen_rx)
}

fn responder(line: &str) -> String {
    match line {
        "status" => {
            "volume: 50\nstate: play\nplaylist: 3\nplaylistlength: 2\nsongid: 1\nOK\n".into()
        }
        "currentsong" => "file: a.flac\nTitle: A\nId: 1\nPos: 0\nOK\n".into(),
        r#"password "secret""# => "OK\n".into(),
        r#"addid "x.mp3""# => "Id: 42\nOK\n".into(),
        r#"play "nope""# => "ACK [2@0] {play} Bad song index\n".into(),
        _ => "OK\n".into(),
    }
}

#[tokio::test]
async fn executes_commands_and_lists() {
    let (address, mut seen) = fake_server(responder).await;
    let mut conn = Connection::connect(&address, Some("secret")).await.unwrap();
    assert_eq!(conn.version(), "0.24.0");
    assert_eq!(seen.recv().await.unwrap(), r#"password "secret""#);

    let pairs = conn.execute(&Command::new("status")).await.unwrap();
    assert!(pairs.contains(&("state".into(), "play".into())));

    let responses = conn
        .execute_list(&[Command::new("status"), Command::new("currentsong")])
        .await
        .unwrap();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[1][0], ("file".into(), "a.flac".into()));

    let error = conn
        .execute(&Command::new("play").arg("nope"))
        .await
        .unwrap_err();
    assert!(matches!(error, MpdError::Ack(ack) if ack.code == 2 && ack.command == "play"));
    // The connection stays usable after an ACK.
    conn.execute(&Command::new("ping")).await.unwrap();
}

#[tokio::test]
async fn worker_answers_queries_and_runs_compound_requests() {
    let (address, mut seen) = fake_server(responder).await;
    let (tx, mut events) = mpsc::unbounded_channel::<MpdEvent>();
    let handle = mpd::spawn(address, None, tx);

    assert!(matches!(
        next(&mut events).await,
        MpdEvent::Connected { .. }
    ));

    handle.query(Query::Status);
    match next(&mut events).await {
        MpdEvent::Reply(Reply::Status { status, current }) => {
            assert_eq!(status.volume, Some(50));
            assert_eq!(current.unwrap().title(), Some("A"));
        }
        other => panic!("unexpected event {other:?}"),
    }

    handle.request(Request::AddAndPlay(vec!["x.mp3".into(), "y.mp3".into()]));
    // Drain the requests seen by the server until the playid arrives.
    let mut lines = Vec::new();
    while !lines.iter().any(|l: &String| l.starts_with("playid")) {
        lines.push(
            timeout(Duration::from_secs(5), seen.recv())
                .await
                .unwrap()
                .unwrap(),
        );
    }
    assert!(lines.contains(&r#"addid "x.mp3""#.to_owned()));
    assert!(lines.contains(&r#"add "y.mp3""#.to_owned()));
    assert!(lines.contains(&r#"playid "42""#.to_owned()));

    handle.exec(Command::new("play").arg("nope"));
    assert!(
        matches!(next(&mut events).await, MpdEvent::Error(msg) if msg.contains("Bad song index"))
    );
}

async fn next(events: &mut mpsc::UnboundedReceiver<MpdEvent>) -> MpdEvent {
    timeout(Duration::from_secs(5), events.recv())
        .await
        .unwrap()
        .unwrap()
}

#[test]
fn subsystem_names() {
    assert_eq!(
        "stored_playlist".parse::<Subsystem>().unwrap(),
        Subsystem::StoredPlaylist
    );
}
