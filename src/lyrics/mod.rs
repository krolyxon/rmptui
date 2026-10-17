//! Finding lyrics for the current song.
//!
//! Sources, in order: an `.lrc` file next to the song, the on-disk cache, and
//! [LRCLIB](https://lrclib.net).

mod lrc;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

pub use lrc::{LyricLine, Lyrics};
use serde::Deserialize;
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::{debug, warn};

use crate::{config::LyricsConfig, event::AppEvent, mpd::Song, util};

/// What the UI knows about the lyrics of the current song.
#[derive(Clone, Debug, Default)]
pub enum LyricsStatus {
    #[default]
    Idle,
    Loading,
    Found(Lyrics),
    NotFound,
    Failed(String),
}

#[derive(Debug, Default)]
pub struct LyricsState {
    /// The song the status belongs to.
    pub file: Option<String>,
    pub status: LyricsStatus,
}

impl LyricsState {
    pub fn lyrics(&self) -> Option<&Lyrics> {
        match &self.status {
            LyricsStatus::Found(lyrics) => Some(lyrics),
            _ => None,
        }
    }
}

/// The result of a lookup, sent back as an [`AppEvent::Lyrics`].
pub type LookupResult = Result<Option<Lyrics>, String>;

pub struct LyricsService {
    config: LyricsConfig,
    http: reqwest::Client,
    events: mpsc::UnboundedSender<AppEvent>,
    task: Option<JoinHandle<()>>,
    /// Results of this session, including misses, to avoid repeated lookups.
    memory: HashMap<String, Option<Lyrics>>,
    pub state: LyricsState,
}

impl LyricsService {
    pub fn new(config: LyricsConfig, events: mpsc::UnboundedSender<AppEvent>) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!(
                "rmptui/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/krolyxon/rmptui)"
            ))
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            config,
            http,
            events,
            task: None,
            memory: HashMap::new(),
            state: LyricsState::default(),
        }
    }

    /// Makes sure lyrics for `song` are loaded or being loaded. With `force`,
    /// skips the session memory and the cache.
    pub fn ensure(&mut self, song: Option<&Song>, music_dir: Option<&Path>, force: bool) {
        let Some(song) = song else {
            self.cancel();
            self.state = LyricsState::default();
            return;
        };
        if !force && self.state.file.as_deref() == Some(song.file.as_str()) {
            return;
        }
        self.cancel();
        self.state.file = Some(song.file.clone());
        if !force && let Some(known) = self.memory.get(&song.file) {
            self.state.status = known
                .clone()
                .map_or(LyricsStatus::NotFound, LyricsStatus::Found);
            return;
        }
        self.state.status = LyricsStatus::Loading;
        let request = Lookup {
            file: song.file.clone(),
            artist: song.artist().map(str::to_owned),
            title: lookup_title(song),
            album: song.album().map(str::to_owned),
            duration: song.duration,
            local: music_dir.map(|dir| dir.join(&song.file).with_extension("lrc")),
            use_cache: self.config.cache && !force,
            write_cache: self.config.cache,
            online: self.config.online,
            base_url: self.config.lrclib_url.trim_end_matches('/').to_owned(),
        };
        let http = self.http.clone();
        let events = self.events.clone();
        self.task = Some(tokio::spawn(async move {
            let file = request.file.clone();
            let result = request.run(&http).await;
            let _ = events.send(AppEvent::Lyrics { file, result });
        }));
    }

    /// Applies a finished lookup. Returns whether it was for the current song.
    pub fn finish(&mut self, file: String, result: LookupResult) -> bool {
        match &result {
            Ok(found) => {
                self.memory.insert(file.clone(), found.clone());
            }
            Err(error) => warn!("lyrics lookup for {file} failed: {error}"),
        }
        if self.state.file.as_deref() != Some(file.as_str()) {
            return false;
        }
        self.task = None;
        self.state.status = match result {
            Ok(Some(lyrics)) => LyricsStatus::Found(lyrics),
            Ok(None) => LyricsStatus::NotFound,
            Err(error) => LyricsStatus::Failed(error),
        };
        true
    }

    fn cancel(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

struct Lookup {
    file: String,
    artist: Option<String>,
    title: Option<String>,
    album: Option<String>,
    duration: Option<Duration>,
    local: Option<PathBuf>,
    use_cache: bool,
    write_cache: bool,
    online: bool,
    base_url: String,
}

impl Lookup {
    async fn run(self, http: &reqwest::Client) -> LookupResult {
        if let Some(path) = &self.local
            && let Some(lyrics) = read_lyrics(path).await
        {
            debug!("lyrics for {} from {}", self.file, path.display());
            return Ok(Some(lyrics));
        }
        let (Some(artist), Some(title)) = (&self.artist, &self.title) else {
            return Ok(None);
        };
        let cache_path = cache_path(artist, title);
        if self.use_cache
            && let Some(lyrics) = read_lyrics(&cache_path).await
        {
            return Ok(Some(lyrics));
        }
        if !self.online {
            return Ok(None);
        }
        let Some(text) = self.fetch(http, artist, title).await? else {
            return Ok(None);
        };
        if self.write_cache {
            if let Some(parent) = cache_path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            if let Err(error) = tokio::fs::write(&cache_path, &text).await {
                warn!(
                    "cannot write lyrics cache {}: {error}",
                    cache_path.display()
                );
            }
        }
        let lyrics = Lyrics::parse(&text);
        Ok((!lyrics.is_empty()).then_some(lyrics))
    }

    /// Asks LRCLIB for an exact match first, then falls back to a search.
    async fn fetch(
        &self,
        http: &reqwest::Client,
        artist: &str,
        title: &str,
    ) -> Result<Option<String>, String> {
        let mut query = vec![
            ("artist_name", artist.to_owned()),
            ("track_name", title.to_owned()),
        ];
        if let Some(album) = &self.album {
            query.push(("album_name", album.clone()));
        }
        if let Some(duration) = self.duration {
            query.push(("duration", duration.as_secs().to_string()));
        }
        let response = http
            .get(format!("{}/api/get", self.base_url))
            .query(&query)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if response.status().is_success() {
            let record: Record = response.json().await.map_err(|e| e.to_string())?;
            if let Some(text) = record.text() {
                return Ok(Some(text));
            }
        } else if response.status() != reqwest::StatusCode::NOT_FOUND {
            return Err(format!("LRCLIB answered {}", response.status()));
        }

        let records: Vec<Record> = http
            .get(format!("{}/api/search", self.base_url))
            .query(&[("artist_name", artist), ("track_name", title)])
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        // Prefer synced lyrics, then plain ones.
        let synced = records
            .iter()
            .find_map(|r| r.synced_lyrics.clone().filter(|s| !s.trim().is_empty()));
        Ok(synced.or_else(|| records.iter().find_map(Record::text)))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    #[serde(default)]
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

impl Record {
    fn text(&self) -> Option<String> {
        if self.instrumental {
            return Some("♪ Instrumental ♪".to_owned());
        }
        self.synced_lyrics
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| self.plain_lyrics.clone().filter(|s| !s.trim().is_empty()))
    }
}

/// The title to look lyrics up by: the Title tag, or a title derived from
/// the file name (`05. Artist - Title (Explicit).mp3` → `Title`), without
/// markers that lyric databases do not include.
fn lookup_title(song: &Song) -> Option<String> {
    let mut title = song
        .title()
        .map_or_else(|| title_from_file_name(song), str::to_owned);
    for marker in [
        "(Explicit)",
        "[Explicit]",
        "(explicit)",
        "[explicit]",
        "(Clean)",
        "[Clean]",
    ] {
        title = title.replace(marker, "");
    }
    let title = title.trim().to_owned();
    // A bare track number is not a title.
    (!title.chars().all(|c| c.is_ascii_digit())).then_some(title)
}

/// Strips a leading track number (`05. `, `05 - `, `05_`) and `Artist - `.
fn title_from_file_name(song: &Song) -> String {
    let stem = song.display_title();
    let without_artist = |s: &str| -> Option<String> {
        let rest = s.strip_prefix(song.artist()?)?;
        Some(rest.trim_start_matches([' ', '-']).to_owned())
    };
    let digits = stem.len() - stem.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let without_track = (digits > 0 && stem[digits..].starts_with(['.', '-', '_', ' ']))
        .then(|| stem[digits..].trim_start_matches(['.', '-', '_', ' ']));
    without_artist(stem)
        .or_else(|| without_track.and_then(without_artist))
        .or_else(|| without_track.map(str::to_owned))
        .unwrap_or_else(|| stem.to_owned())
}

async fn read_lyrics(path: &Path) -> Option<Lyrics> {
    let text = tokio::fs::read_to_string(path).await.ok()?;
    let lyrics = Lyrics::parse(&text);
    (!lyrics.is_empty()).then_some(lyrics)
}

fn cache_path(artist: &str, title: &str) -> PathBuf {
    let name: String = format!("{artist} - {title}")
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | '\0') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .take(200)
        .collect();
    util::cache_dir().join("lyrics").join(format!("{name}.lrc"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_lookup_titles() {
        let song = |file: &str, tags: &[(&str, &str)]| Song {
            file: file.into(),
            tags: tags
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
            ..Song::default()
        };
        let untagged = song(
            "x/05. 50 Cent - Candy Shop (Explicit).mp3",
            &[("Artist", "50 Cent")],
        );
        assert_eq!(lookup_title(&untagged).as_deref(), Some("Candy Shop"));
        let tagged = song("x/a.flac", &[("Title", "Song [Explicit]")]);
        assert_eq!(lookup_title(&tagged).as_deref(), Some("Song"));
        assert_eq!(lookup_title(&song("x/01.mp3", &[])), None);
    }

    #[test]
    fn sanitizes_cache_names() {
        let path = cache_path("AC/DC", "T.N.T.");
        assert_eq!(path.file_name().unwrap(), "AC_DC - T.N.T..lrc");
    }

    #[test]
    fn prefers_synced_record_text() {
        let record: Record = serde_json::from_str(
            r#"{"instrumental":false,"plainLyrics":"plain","syncedLyrics":"[00:01.00]synced"}"#,
        )
        .unwrap();
        assert_eq!(record.text().as_deref(), Some("[00:01.00]synced"));
        let record: Record =
            serde_json::from_str(r#"{"instrumental":true,"plainLyrics":null,"syncedLyrics":null}"#)
                .unwrap();
        assert!(record.text().unwrap().contains("Instrumental"));
    }
}
