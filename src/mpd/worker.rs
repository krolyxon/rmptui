//! Background tasks that own the MPD connections.
//!
//! * The **idle** task blocks on `idle` and reports changed subsystems.
//! * The **command** task executes [`Request`]s sent through an [`MpdHandle`]
//!   and reports query results as [`Reply`]s.
//!
//! The UI never awaits the server: it sends requests and reacts to events.

use std::{path::PathBuf, time::Duration};

use tokio::{sync::mpsc, time::sleep};
use tracing::{debug, warn};

use super::{
    commands,
    connection::{Address, Connection},
    error::{MpdError, MpdResult},
    protocol::{Command, Pair},
    types::{
        Album, DirEntry, PlaylistInfo, Song, Status, Subsystem, parse_albums, parse_entries,
        parse_playlists, parse_songs, parse_subsystems, parse_tag_values,
    },
};

/// Data the UI asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Query {
    /// Status and current song.
    Status,
    /// Queue changes since the given queue version (`0` for everything).
    QueueChanges {
        since: u32,
    },
    Playlists,
    Directory(String),
    PlaylistSongs(String),
    /// All values of a tag, e.g. every `AlbumArtist`.
    TagValues(String),
    /// Albums whose `tag` equals `value`, grouped by date.
    Albums {
        tag: String,
        value: String,
    },
    /// Songs matching a filter expression.
    Find(String),
    /// The server's music directory (only answered on local connections).
    MusicDirectory,
}

/// The answer to a [`Query`].
#[derive(Clone, Debug)]
pub enum Reply {
    Status {
        status: Status,
        current: Option<Song>,
    },
    QueueChanges {
        status: Status,
        since: u32,
        changes: Vec<Song>,
    },
    Playlists(Vec<PlaylistInfo>),
    Directory {
        path: String,
        entries: Vec<DirEntry>,
    },
    PlaylistSongs {
        name: String,
        songs: Vec<Song>,
    },
    TagValues {
        tag: String,
        values: Vec<String>,
    },
    Albums {
        tag: String,
        value: String,
        albums: Vec<Album>,
    },
    Find {
        filter: String,
        songs: Vec<Song>,
    },
    MusicDirectory(Option<PathBuf>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Fire-and-forget commands, executed atomically as one command list.
    Exec(Vec<Command>),
    Query(Query),
    /// Appends songs and starts playing the first one.
    AddAndPlay(Vec<String>),
    /// Appends every song matching a filter and plays the first one.
    FindAddAndPlay(String),
    /// Appends a stored playlist and plays its song at `offset`.
    LoadAndPlay {
        name: String,
        offset: u32,
    },
}

/// Notifications from the background tasks.
#[derive(Clone, Debug)]
pub enum MpdEvent {
    Connected { version: String },
    Disconnected(String),
    Changed(Vec<Subsystem>),
    Reply(Reply),
    Error(String),
}

/// Cheap, cloneable sender for requests.
#[derive(Clone, Debug)]
pub struct MpdHandle {
    tx: mpsc::UnboundedSender<Request>,
}

impl MpdHandle {
    pub fn request(&self, request: Request) {
        // The worker lives as long as the application, so a closed channel
        // only happens during shutdown.
        let _ = self.tx.send(request);
    }

    pub fn exec(&self, command: Command) {
        self.request(Request::Exec(vec![command]));
    }

    pub fn exec_all(&self, commands: Vec<Command>) {
        if !commands.is_empty() {
            self.request(Request::Exec(commands));
        }
    }

    pub fn query(&self, query: Query) {
        self.request(Request::Query(query));
    }

    /// A handle whose requests go to `tx`, for tests.
    pub fn from_sender(tx: mpsc::UnboundedSender<Request>) -> Self {
        Self { tx }
    }
}

/// Starts both workers and returns the request handle.
pub fn spawn<E>(
    address: Address,
    password: Option<String>,
    events: mpsc::UnboundedSender<E>,
) -> MpdHandle
where
    E: From<MpdEvent> + Send + 'static,
{
    let (tx, rx) = mpsc::unbounded_channel();
    let emit = Emitter(events);
    tokio::spawn(idle_task(address.clone(), password.clone(), emit.clone()));
    tokio::spawn(command_task(address, password, rx, emit));
    MpdHandle { tx }
}

struct Emitter<E>(mpsc::UnboundedSender<E>);

impl<E> Clone for Emitter<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E: From<MpdEvent>> Emitter<E> {
    /// Returns `false` once the application stopped listening.
    fn emit(&self, event: MpdEvent) -> bool {
        self.0.send(event.into()).is_ok()
    }
}

const MAX_BACKOFF: Duration = Duration::from_secs(10);

async fn idle_task<E: From<MpdEvent>>(
    address: Address,
    password: Option<String>,
    emit: Emitter<E>,
) {
    let mut backoff = Duration::from_millis(500);
    let mut reported_down = false;
    loop {
        match Connection::connect(&address, password.as_deref()).await {
            Ok(mut conn) => {
                backoff = Duration::from_millis(500);
                if !emit.emit(MpdEvent::Connected {
                    version: conn.version().to_owned(),
                }) {
                    return;
                }
                let error = loop {
                    match conn.execute(&Command::new("idle")).await {
                        Ok(pairs) => {
                            if !emit.emit(MpdEvent::Changed(parse_subsystems(&pairs))) {
                                return;
                            }
                        }
                        Err(error) => break error,
                    }
                };
                warn!("idle connection lost: {error}");
                if !emit.emit(MpdEvent::Disconnected(error.to_string())) {
                    return;
                }
                reported_down = true;
            }
            Err(error) => {
                debug!("connecting to {address} failed: {error}");
                if !reported_down {
                    reported_down = true;
                    if !emit.emit(MpdEvent::Disconnected(format!(
                        "cannot connect to {address}: {error}"
                    ))) {
                        return;
                    }
                }
            }
        }
        sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

async fn command_task<E: From<MpdEvent>>(
    address: Address,
    password: Option<String>,
    mut requests: mpsc::UnboundedReceiver<Request>,
    emit: Emitter<E>,
) {
    let mut conn: Option<Connection> = None;
    while let Some(request) = requests.recv().await {
        // MPD drops connections that stay silent for `connection_timeout`, so a
        // failure on a reused connection is retried once on a fresh one.
        let mut attempts = if conn.is_some() { 2 } else { 1 };
        let result = loop {
            let c = match conn.as_mut() {
                Some(c) => c,
                None => match Connection::connect(&address, password.as_deref()).await {
                    Ok(c) => conn.insert(c),
                    Err(error) => break Err(error),
                },
            };
            attempts -= 1;
            match execute(c, &request).await {
                Err(error) if error.is_fatal() => {
                    conn = None;
                    if attempts == 0 {
                        break Err(error);
                    }
                }
                other => break other,
            }
        };
        let event = match result {
            Ok(Some(reply)) => MpdEvent::Reply(reply),
            Ok(None) => continue,
            Err(error) => {
                warn!("request {request:?} failed: {error}");
                MpdEvent::Error(error.to_string())
            }
        };
        if !emit.emit(event) {
            return;
        }
    }
}

async fn execute(conn: &mut Connection, request: &Request) -> MpdResult<Option<Reply>> {
    match request {
        Request::Exec(commands) => conn.execute_all(commands).await.map(|()| None),
        Request::Query(query) => run_query(conn, query).await.map(Some),
        Request::AddAndPlay(uris) => {
            let Some((first, rest)) = uris.split_first() else {
                return Ok(None);
            };
            let id = conn
                .execute(&commands::add_id(first))
                .await?
                .into_iter()
                .find(|(key, _)| key == "Id")
                .and_then(|(_, id)| id.parse().ok())
                .ok_or_else(|| MpdError::Protocol("addid returned no id".into()))?;
            let mut batch: Vec<Command> = rest.iter().map(|uri| commands::add(uri)).collect();
            batch.push(commands::play_id(id));
            conn.execute_all(&batch).await.map(|()| None)
        }
        Request::FindAddAndPlay(filter) => {
            let start = queue_length(conn).await?;
            conn.execute_all(&[commands::find_add(filter), commands::play_pos(start)])
                .await
                .map(|()| None)
        }
        Request::LoadAndPlay { name, offset } => {
            let start = queue_length(conn).await?;
            let entries = conn.execute(&Command::new("listplaylist").arg(name)).await?;
            conn.execute(&commands::load(name)).await?;
            // `load` silently skips entries the server cannot resolve, so the
            // playlist index is not necessarily the queue offset.
            let added = conn
                .execute(&Command::new("playlistinfo").arg(format!("{start}:")))
                .await?;
            let entries: Vec<&str> = files(&entries).collect();
            let added: Vec<&str> = files(&added).collect();
            let Some(pos) = queued_offset(&entries, &added, *offset as usize) else {
                return Ok(None);
            };
            conn.execute(&commands::play_pos(start + pos as u32))
                .await
                .map(|_| None)
        }
    }
}

fn files(pairs: &[Pair]) -> impl Iterator<Item = &str> {
    pairs
        .iter()
        .filter(|(key, _)| key == "file")
        .map(|(_, value)| value.as_str())
}

/// Where playlist entry `index` ended up among the songs `load` appended,
/// matching entries in order and skipping the ones that were dropped.
fn queued_offset(entries: &[&str], added: &[&str], index: usize) -> Option<usize> {
    if entries.len() == added.len() {
        return (index < added.len()).then_some(index);
    }
    let mut pos = 0;
    for (i, entry) in entries.iter().enumerate() {
        let matched = added.get(pos).is_some_and(|song| same_song(entry, song));
        if i == index {
            // The entry was skipped: play whatever followed it instead.
            return (pos < added.len()).then_some(pos);
        }
        if matched {
            pos += 1;
        }
    }
    None
}

/// Playlists may store absolute paths, which `load` turns into paths
/// relative to the music directory.
fn same_song(entry: &str, queued: &str) -> bool {
    entry == queued
        || entry
            .strip_suffix(queued)
            .is_some_and(|prefix| prefix.ends_with('/'))
}

async fn queue_length(conn: &mut Connection) -> MpdResult<u32> {
    let status = conn.execute(&Command::new("status")).await?;
    Ok(Status::from_pairs(&status).playlist_length)
}

async fn run_query(conn: &mut Connection, query: &Query) -> MpdResult<Reply> {
    Ok(match query {
        Query::Status => {
            let responses = conn
                .execute_list(&[Command::new("status"), Command::new("currentsong")])
                .await?;
            let [status, current] = <[_; 2]>::try_from(responses)
                .map_err(|_| MpdError::Protocol("short command list response".into()))?;
            let current = (!current.is_empty()).then(|| Song::from_pairs(&current));
            Reply::Status {
                status: Status::from_pairs(&status),
                current,
            }
        }
        Query::QueueChanges { since } => {
            let responses = conn
                .execute_list(&[
                    Command::new("status"),
                    Command::new("plchanges").arg(since.to_string()),
                ])
                .await?;
            let [status, changes] = <[_; 2]>::try_from(responses)
                .map_err(|_| MpdError::Protocol("short command list response".into()))?;
            Reply::QueueChanges {
                status: Status::from_pairs(&status),
                since: *since,
                changes: parse_songs(&changes),
            }
        }
        Query::Playlists => Reply::Playlists(parse_playlists(
            &conn.execute(&Command::new("listplaylists")).await?,
        )),
        Query::Directory(path) => {
            let entries = parse_entries(&conn.execute(&Command::new("lsinfo").arg(path)).await?);
            Reply::Directory {
                path: path.clone(),
                entries,
            }
        }
        Query::PlaylistSongs(name) => {
            let pairs = conn
                .execute(&Command::new("listplaylistinfo").arg(name))
                .await?;
            Reply::PlaylistSongs {
                name: name.clone(),
                songs: parse_songs(&pairs),
            }
        }
        Query::TagValues(tag) => {
            let pairs = conn.execute(&Command::new("list").arg(tag)).await?;
            Reply::TagValues {
                tag: tag.clone(),
                values: parse_tag_values(&pairs, tag),
            }
        }
        Query::Albums { tag, value } => {
            let pairs = conn
                .execute(
                    &Command::new("list")
                        .arg("Album")
                        .arg(super::protocol::filter_eq(tag, value))
                        .arg("group")
                        .arg("Date"),
                )
                .await?;
            Reply::Albums {
                tag: tag.clone(),
                value: value.clone(),
                albums: parse_albums(&pairs),
            }
        }
        Query::Find(filter) => {
            let mut songs = parse_songs(&conn.execute(&Command::new("find").arg(filter)).await?);
            songs.sort_by_key(Song::disc_track);
            Reply::Find {
                filter: filter.clone(),
                songs,
            }
        }
        Query::MusicDirectory => {
            let dir = match conn.execute(&Command::new("config")).await {
                Ok(pairs) => pairs
                    .into_iter()
                    .find(|(key, _)| key == "music_directory")
                    .map(|(_, dir)| PathBuf::from(dir)),
                // `config` is only permitted on local (Unix socket) connections.
                Err(MpdError::Ack(_)) => None,
                Err(error) => return Err(error),
            };
            Reply::MusicDirectory(dir)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::queued_offset;

    #[test]
    fn queued_offset_skips_dropped_entries() {
        let entries = ["gone.mp3", "a.mp3", "b.mp3", "missing.mp3", "c.mp3"];
        let added = ["a.mp3", "b.mp3", "c.mp3"];
        assert_eq!(queued_offset(&entries, &added, 1), Some(0));
        assert_eq!(queued_offset(&entries, &added, 2), Some(1));
        assert_eq!(queued_offset(&entries, &added, 4), Some(2));
        // A dropped entry plays the song after it.
        assert_eq!(queued_offset(&entries, &added, 0), Some(0));
        assert_eq!(queued_offset(&entries, &added, 3), Some(2));
    }

    #[test]
    fn queued_offset_matches_absolute_paths() {
        let entries = ["/music/x.mp3", "gone.mp3", "/music/y.mp3"];
        let added = ["x.mp3", "y.mp3"];
        assert_eq!(queued_offset(&entries, &added, 2), Some(1));
    }
}
