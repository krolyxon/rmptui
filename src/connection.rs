use std::{
    collections::HashMap,
    error::Error,
    fmt,
    time::{Duration, Instant},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

use crate::AppResult;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Song {
    pub file: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub tags: Vec<(String, String)>,
    pub duration: Option<Duration>,
    pub place: Option<Place>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Place {
    pub pos: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Paused,
    #[default]
    Stopped,
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub state: PlaybackState,
    pub volume: i8,
    pub repeat: bool,
    pub random: bool,
    pub song: Option<Place>,
    pub elapsed: Duration,
    pub total: Duration,
    pub updating_db: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub songs: u32,
}

#[derive(Clone, Copy, Debug)]
pub enum VolumeStatus {
    Muted(i8),
    Unmuted,
}

#[derive(Clone, Debug)]
pub struct MpdSnapshot {
    pub status: Status,
    pub stats: Stats,
    pub current_song: Song,
    pub queue: Vec<Song>,
}

#[derive(Debug)]
struct MpdError(String);

impl fmt::Display for MpdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MpdError {}

#[derive(Debug)]
pub struct MpdClient {
    stream: BufReader<TcpStream>,
}

impl MpdClient {
    pub async fn connect(address: &str, password: Option<&str>) -> AppResult<Self> {
        let stream = TcpStream::connect(address).await?;
        let mut client = Self {
            stream: BufReader::new(stream),
        };
        let mut greeting = String::new();
        client.stream.read_line(&mut greeting).await?;
        if !greeting.starts_with("OK MPD ") {
            return Err(Box::new(MpdError(format!(
                "unexpected MPD greeting: {}",
                greeting.trim_end()
            ))));
        }
        if let Some(password) = password {
            client.command("password", &[password]).await?;
        }
        Ok(client)
    }

    async fn command(&mut self, name: &str, args: &[&str]) -> AppResult<Vec<String>> {
        let mut request = name.to_string();
        for arg in args {
            request.push(' ');
            request.push_str(&quote(arg));
        }
        request.push('\n');
        self.stream.get_mut().write_all(request.as_bytes()).await?;
        self.stream.get_mut().flush().await?;

        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            let read = self.stream.read_line(&mut line).await?;
            if read == 0 {
                return Err(Box::new(MpdError(
                    "MPD closed the connection while replying to a command".to_string(),
                )));
            }
            let line = line.trim_end_matches(['\r', '\n']);
            if line == "OK" {
                return Ok(lines);
            }
            if line.starts_with("ACK ") {
                return Err(Box::new(MpdError(format!("MPD rejected `{name}`: {line}"))));
            }
            lines.push(line.to_string());
        }
    }

    pub async fn status(&mut self) -> AppResult<Status> {
        let fields = fields(self.command("status", &[]).await?);
        let time = fields.get("time").and_then(|s| {
            let (elapsed, total) = s.split_once(':')?;
            Some((parse_duration(elapsed), parse_duration(total)))
        });
        let elapsed = fields
            .get("elapsed")
            .map(|s| parse_duration(s))
            .or_else(|| time.map(|(elapsed, _)| elapsed))
            .unwrap_or_default();
        let total = fields
            .get("duration")
            .map(|s| parse_duration(s))
            .or_else(|| time.map(|(_, total)| total))
            .unwrap_or_default();

        Ok(Status {
            state: match fields.get("state").map(String::as_str) {
                Some("play") => PlaybackState::Playing,
                Some("pause") => PlaybackState::Paused,
                _ => PlaybackState::Stopped,
            },
            volume: fields
                .get("volume")
                .and_then(|s| s.parse().ok())
                .unwrap_or(-1),
            repeat: bool_field(&fields, "repeat"),
            random: bool_field(&fields, "random"),
            song: fields
                .get("song")
                .and_then(|s| s.parse().ok())
                .map(|pos| Place { pos }),
            elapsed,
            total,
            updating_db: fields.get("updating_db").and_then(|s| s.parse().ok()),
        })
    }

    pub async fn stats(&mut self) -> AppResult<Stats> {
        let fields = fields(self.command("stats", &[]).await?);
        Ok(Stats {
            songs: fields
                .get("songs")
                .and_then(|s| s.parse().ok())
                .unwrap_or_default(),
        })
    }

    pub async fn current_song(&mut self) -> AppResult<Song> {
        Ok(parse_songs(self.command("currentsong", &[]).await?)
            .into_iter()
            .next()
            .unwrap_or_default())
    }

    pub async fn queue(&mut self) -> AppResult<Vec<Song>> {
        Ok(parse_songs(self.command("playlistinfo", &[]).await?))
    }

    pub async fn snapshot(&mut self) -> AppResult<MpdSnapshot> {
        let status = self.status().await?;
        let stats = self.stats().await?;
        let current_song = self.current_song().await?;
        let queue = self.queue().await?;
        Ok(MpdSnapshot {
            status,
            stats,
            current_song,
            queue,
        })
    }

    pub async fn list_files(&mut self, path: &str) -> AppResult<Vec<(String, String)>> {
        let lines = if path.is_empty() || path == "." {
            self.command("listfiles", &[]).await?
        } else {
            self.command("listfiles", &[path]).await?
        };
        Ok(lines
            .into_iter()
            .filter_map(|line| {
                let (kind, value) = parse_field(&line)?;
                matches!(kind.as_str(), "file" | "directory").then_some((kind, value))
            })
            .collect())
    }

    pub async fn ls_info(&mut self, path: &str) -> AppResult<(Vec<String>, Vec<Song>)> {
        let lines = if path.is_empty() || path == "." {
            self.command("lsinfo", &[]).await?
        } else {
            self.command("lsinfo", &[path]).await?
        };
        let mut directories = Vec::new();
        let mut songs = Vec::new();
        let mut current: Option<Song> = None;
        for line in lines {
            let Some((key, value)) = parse_field(&line) else {
                continue;
            };
            match key.as_str() {
                "directory" => directories.push(value),
                "file" => {
                    if let Some(song) = current.take() {
                        songs.push(song);
                    }
                    current = Some(Song {
                        file: value,
                        ..Song::default()
                    });
                }
                "playlist" => {}
                _ => {
                    if let Some(song) = current.as_mut() {
                        add_metadata(song, key, value);
                    }
                }
            }
        }
        if let Some(song) = current {
            songs.push(song);
        }
        Ok((directories, songs))
    }

    pub async fn playlists(&mut self) -> AppResult<Vec<String>> {
        Ok(self
            .command("listplaylists", &[])
            .await?
            .into_iter()
            .filter_map(|line| parse_field(&line))
            .filter_map(|(key, value)| (key == "playlist").then_some(value))
            .collect())
    }

    pub async fn playlist(&mut self, name: &str) -> AppResult<Vec<Song>> {
        Ok(parse_songs(
            self.command("listplaylistinfo", &[name]).await?,
        ))
    }

    pub async fn add(&mut self, file: &str) -> AppResult<()> {
        self.command("add", &[file]).await?;
        Ok(())
    }

    pub async fn play(&mut self, position: Option<u32>) -> AppResult<()> {
        match position {
            Some(position) => self.command("play", &[&position.to_string()]).await?,
            None => self.command("play", &[]).await?,
        };
        Ok(())
    }

    pub async fn pause(&mut self, paused: bool) -> AppResult<()> {
        self.command("pause", &[if paused { "1" } else { "0" }])
            .await?;
        Ok(())
    }

    pub async fn toggle_pause(&mut self) -> AppResult<()> {
        self.command("pause", &[]).await?;
        Ok(())
    }

    pub async fn set_repeat(&mut self, enabled: bool) -> AppResult<()> {
        self.command("repeat", &[if enabled { "1" } else { "0" }])
            .await?;
        Ok(())
    }

    pub async fn set_random(&mut self, enabled: bool) -> AppResult<()> {
        self.command("random", &[if enabled { "1" } else { "0" }])
            .await?;
        Ok(())
    }

    pub async fn set_volume(&mut self, volume: i8) -> AppResult<()> {
        self.command("setvol", &[&volume.clamp(0, 100).to_string()])
            .await?;
        Ok(())
    }

    pub async fn seek(&mut self, position: u32, elapsed: Duration) -> AppResult<()> {
        self.command(
            "seek",
            &[&position.to_string(), &elapsed.as_secs().to_string()],
        )
        .await?;
        Ok(())
    }

    pub async fn next(&mut self) -> AppResult<()> {
        self.command("next", &[]).await?;
        Ok(())
    }

    pub async fn previous(&mut self) -> AppResult<()> {
        self.command("previous", &[]).await?;
        Ok(())
    }

    pub async fn clear(&mut self) -> AppResult<()> {
        self.command("clear", &[]).await?;
        Ok(())
    }

    pub async fn delete(&mut self, position: u32) -> AppResult<()> {
        self.command("delete", &[&position.to_string()]).await?;
        Ok(())
    }

    pub async fn swap(&mut self, first: u32, second: u32) -> AppResult<()> {
        self.command("swap", &[&first.to_string(), &second.to_string()])
            .await?;
        Ok(())
    }

    pub async fn load_playlist(&mut self, name: &str) -> AppResult<()> {
        self.append_playlist(name).await?;
        self.play(None).await
    }

    pub async fn append_playlist(&mut self, name: &str) -> AppResult<()> {
        self.command("load", &[name]).await?;
        Ok(())
    }

    pub async fn add_to_playlist(&mut self, playlist: &str, file: &str) -> AppResult<()> {
        self.command("playlistadd", &[playlist, file]).await?;
        Ok(())
    }

    pub async fn delete_playlist_song(&mut self, playlist: &str, position: usize) -> AppResult<()> {
        self.command("playlistdelete", &[playlist, &position.to_string()])
            .await?;
        Ok(())
    }

    pub async fn move_playlist_song(
        &mut self,
        playlist: &str,
        from: usize,
        to: usize,
    ) -> AppResult<()> {
        self.command(
            "playlistmove",
            &[playlist, &from.to_string(), &to.to_string()],
        )
        .await?;
        Ok(())
    }

    pub async fn delete_playlist(&mut self, playlist: &str) -> AppResult<()> {
        self.command("rm", &[playlist]).await?;
        Ok(())
    }

    pub async fn rename_playlist(&mut self, from: &str, to: &str) -> AppResult<()> {
        self.command("rename", &[from, to]).await?;
        Ok(())
    }

    pub async fn update_database(&mut self) -> AppResult<()> {
        self.command("rescan", &[]).await?;
        Ok(())
    }

    pub async fn idle(&mut self) -> AppResult<()> {
        self.command(
            "idle",
            &[
                "database",
                "stored_playlist",
                "player",
                "playlist",
                "mixer",
                "options",
            ],
        )
        .await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Connection {
    pub client: MpdClient,
    pub current_song: Song,
    pub stats: Stats,
    pub status: Status,
    pub volume_status: VolumeStatus,
    last_status_update: Instant,
}

impl Connection {
    pub async fn connect(address: &str, password: Option<&str>) -> AppResult<Self> {
        let mut client = MpdClient::connect(address, password).await?;
        let status = client.status().await?;
        let stats = client.stats().await?;
        let current_song = client.current_song().await?;
        Ok(Self {
            client,
            current_song,
            stats,
            status,
            volume_status: VolumeStatus::Unmuted,
            last_status_update: Instant::now(),
        })
    }

    pub fn apply_snapshot(&mut self, status: Status, stats: Stats, current_song: Song) {
        self.status = status;
        self.stats = stats;
        self.current_song = current_song;
        self.last_status_update = Instant::now();
    }

    pub async fn update_status(&mut self) -> AppResult<()> {
        self.status = self.client.status().await?;
        self.stats = self.client.stats().await?;
        self.current_song = self.client.current_song().await?;
        self.last_status_update = Instant::now();
        Ok(())
    }

    pub fn get_progress_ratio(&self) -> f64 {
        let total = self.status.total.as_secs_f64();
        if total <= 0.0 {
            0.0
        } else {
            (self.current_elapsed().as_secs_f64() / total).clamp(0.0, 1.0)
        }
    }

    pub fn current_elapsed(&self) -> Duration {
        let elapsed = if self.status.state == PlaybackState::Playing {
            self.status
                .elapsed
                .saturating_add(self.last_status_update.elapsed())
        } else {
            self.status.elapsed
        };
        if self.status.total.is_zero() {
            elapsed
        } else {
            elapsed.min(self.status.total)
        }
    }

    pub async fn push(&mut self, song: &Song) -> AppResult<()> {
        self.client.add(&song.file).await?;
        let queue = self.client.queue().await?;
        if !queue.is_empty() {
            self.client.play(Some(queue.len() as u32 - 1)).await?;
        }
        self.update_status().await
    }

    pub async fn load_playlist(&mut self, playlist: &str) -> AppResult<()> {
        self.client.load_playlist(playlist).await?;
        self.update_status().await
    }

    pub async fn add_to_playlist(&mut self, playlist: &str, song: &Song) -> AppResult<()> {
        self.client.add_to_playlist(playlist, &song.file).await
    }

    pub fn song_from_file(filename: &str) -> Song {
        Song {
            file: filename.to_string(),
            ..Song::default()
        }
    }

    pub fn now_playing(&self) -> String {
        match (&self.current_song.title, &self.current_song.artist) {
            (Some(title), Some(artist)) => format!("\"{title}\" By {artist}"),
            (Some(title), None) => title.clone(),
            _ if self.current_song.file.is_empty() => "No Song playing or in Queue".to_string(),
            _ => self.current_song.file.clone(),
        }
    }

    pub async fn toggle_pause(&mut self) -> AppResult<()> {
        self.client.toggle_pause().await?;
        self.update_status().await
    }

    pub async fn pause(&mut self) -> AppResult<()> {
        self.client.pause(true).await?;
        self.update_status().await
    }

    pub async fn toggle_repeat(&mut self) -> AppResult<()> {
        self.client.set_repeat(!self.status.repeat).await?;
        self.update_status().await
    }

    pub async fn toggle_random(&mut self) -> AppResult<()> {
        self.client.set_random(!self.status.random).await?;
        self.update_status().await
    }

    pub async fn inc_volume(&mut self, amount: i8) -> AppResult<()> {
        let volume = self.status.volume.saturating_add(amount).min(100);
        self.volume_status = VolumeStatus::Unmuted;
        self.client.set_volume(volume).await?;
        self.update_status().await
    }

    pub async fn dec_volume(&mut self, amount: i8) -> AppResult<()> {
        let volume = self.status.volume.saturating_sub(amount).max(0);
        self.volume_status = VolumeStatus::Unmuted;
        self.client.set_volume(volume).await?;
        self.update_status().await
    }

    pub async fn refresh_song_list(&mut self) -> AppResult<()> {
        self.stats = self.client.stats().await?;
        Ok(())
    }
}

fn fields(lines: Vec<String>) -> HashMap<String, String> {
    lines
        .into_iter()
        .filter_map(|line| parse_field(&line))
        .collect()
}

fn parse_songs(lines: Vec<String>) -> Vec<Song> {
    let mut songs = Vec::new();
    let mut current: Option<Song> = None;
    for line in lines {
        let Some((key, value)) = parse_field(&line) else {
            continue;
        };
        if key == "file" {
            if let Some(song) = current.take() {
                songs.push(song);
            }
            current = Some(Song {
                file: value,
                ..Song::default()
            });
        } else if let Some(song) = current.as_mut() {
            if key.eq_ignore_ascii_case("pos") {
                song.place = value.parse().ok().map(|pos| Place { pos });
            } else if key == "duration" || key == "Time" {
                song.duration = Some(parse_duration(&value));
            } else {
                add_metadata(song, key, value);
            }
        }
    }
    if let Some(song) = current {
        songs.push(song);
    }
    songs
}

fn add_metadata(song: &mut Song, key: String, value: String) {
    match key.to_ascii_lowercase().as_str() {
        "title" => song.title = Some(value),
        "artist" => song.artist = Some(value),
        "duration" | "time" => song.duration = Some(parse_duration(&value)),
        _ => song.tags.push((key, value)),
    }
}

fn parse_field(line: &str) -> Option<(String, String)> {
    let (key, value) = line.split_once(": ")?;
    Some((key.to_string(), unquote(value)))
}

fn unquote(value: &str) -> String {
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    let mut result = String::with_capacity(value.len());
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            result.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            result.push(character);
        }
    }
    if escaped {
        result.push('\\');
    }
    result
}

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn bool_field(fields: &HashMap<String, String>, key: &str) -> bool {
    fields.get(key).is_some_and(|value| value == "1")
}

fn parse_duration(value: &str) -> Duration {
    value
        .parse::<f64>()
        .ok()
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::TcpListener,
    };

    #[test]
    fn parses_quoted_mpd_values_and_escapes() {
        assert_eq!(
            parse_field(r#"Title: "A \"quoted\" song\\file""#),
            Some(("Title".to_string(), "A \"quoted\" song\\file".to_string()))
        );
        assert_eq!(quote(r#"A "song"\path"#), r#""A \"song\"\\path""#);
    }

    #[test]
    fn parses_songs_with_tags_and_queue_positions() {
        let songs = parse_songs(vec![
            "file: music/example.flac".to_string(),
            "Title: Example".to_string(),
            "Artist: Someone".to_string(),
            "duration: 124.5".to_string(),
            "Pos: 3".to_string(),
            "file: music/another.ogg".to_string(),
        ]);
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].title.as_deref(), Some("Example"));
        assert_eq!(songs[0].artist.as_deref(), Some("Someone"));
        assert_eq!(songs[0].duration, Some(Duration::from_secs_f64(124.5)));
        assert_eq!(songs[0].place, Some(Place { pos: 3 }));
        assert_eq!(songs[1].file, "music/another.ogg");
    }

    #[tokio::test]
    async fn sends_quoted_commands_over_the_mpd_tcp_protocol() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut stream = BufReader::new(stream);
            stream
                .get_mut()
                .write_all(b"OK MPD 0.23.0\n")
                .await
                .unwrap();
            let mut command = String::new();
            stream.read_line(&mut command).await.unwrap();
            assert_eq!(command, "play \"0\"\n");
            stream.get_mut().write_all(b"OK\n").await.unwrap();
        });

        let mut client = MpdClient::connect(&address.to_string(), None)
            .await
            .unwrap();
        client.command("play", &["0"]).await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn omits_empty_root_paths_from_mpd_commands() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut stream = BufReader::new(stream);
            stream
                .get_mut()
                .write_all(b"OK MPD 0.23.0\n")
                .await
                .unwrap();
            for expected in ["lsinfo\n", "listfiles\n"] {
                let mut command = String::new();
                stream.read_line(&mut command).await.unwrap();
                assert_eq!(command, expected);
                stream.get_mut().write_all(b"OK\n").await.unwrap();
            }
        });

        let mut client = MpdClient::connect(&address.to_string(), None)
            .await
            .unwrap();
        client.ls_info("").await.unwrap();
        client.list_files("").await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn sends_stored_playlist_edit_commands() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut stream = BufReader::new(stream);
            stream
                .get_mut()
                .write_all(b"OK MPD 0.23.0\n")
                .await
                .unwrap();
            for expected in [
                "playlistdelete \"mix \\\"one\\\"\" \"2\"\n",
                "playlistmove \"mix \\\"one\\\"\" \"1\" \"2\"\n",
                "rm \"mix \\\"one\\\"\"\n",
            ] {
                let mut command = String::new();
                stream.read_line(&mut command).await.unwrap();
                assert_eq!(command, expected);
                stream.get_mut().write_all(b"OK\n").await.unwrap();
            }
        });

        let mut client = MpdClient::connect(&address.to_string(), None)
            .await
            .unwrap();
        client.delete_playlist_song("mix \"one\"", 2).await.unwrap();
        client
            .move_playlist_song("mix \"one\"", 1, 2)
            .await
            .unwrap();
        client.delete_playlist("mix \"one\"").await.unwrap();
        server.await.unwrap();
    }
}
