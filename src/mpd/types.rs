//! Typed models built from MPD responses.

use std::{path::Path, str::FromStr, time::Duration};

use super::protocol::{Pair, split_groups};

/// A song as reported by MPD (library entry or queue item).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Song {
    /// Library-relative path or URL.
    pub file: String,
    /// Queue id, present for queue items only.
    pub id: Option<u32>,
    /// Queue position, present for queue items only.
    pub pos: Option<u32>,
    pub duration: Option<Duration>,
    /// Tag values in server order. A tag may occur multiple times.
    pub tags: Vec<(String, String)>,
}

impl Song {
    pub fn from_pairs(pairs: &[Pair]) -> Self {
        let mut song = Self::default();
        for (key, value) in pairs {
            match key.as_str() {
                "file" => song.file.clone_from(value),
                "Id" => song.id = value.parse().ok(),
                "Pos" => song.pos = value.parse().ok(),
                "duration" => song.duration = parse_seconds(value),
                // Older servers only report the integral `Time`.
                "Time" if song.duration.is_none() => song.duration = parse_seconds(value),
                "Last-Modified" | "Added" | "Format" | "Time" | "Range" | "Prio" => {}
                _ => song.tags.push((key.clone(), value.clone())),
            }
        }
        song
    }

    /// The first value of `tag` (case-insensitive).
    pub fn tag(&self, tag: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(tag))
            .map(|(_, value)| value.as_str())
    }

    pub fn title(&self) -> Option<&str> {
        self.tag("Title")
    }

    pub fn artist(&self) -> Option<&str> {
        self.tag("Artist")
    }

    pub fn album(&self) -> Option<&str> {
        self.tag("Album")
    }

    pub fn album_artist(&self) -> Option<&str> {
        self.tag("AlbumArtist").or_else(|| self.artist())
    }

    /// The file name without its directory.
    pub fn file_name(&self) -> &str {
        self.file.rsplit('/').next().unwrap_or(&self.file)
    }

    /// The title, falling back to the file name without extension.
    pub fn display_title(&self) -> &str {
        self.title().unwrap_or_else(|| {
            let name = self.file_name();
            Path::new(name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(name)
        })
    }

    /// Track number without a `/total` suffix.
    pub fn track(&self) -> Option<&str> {
        self.tag("Track").map(|t| t.split('/').next().unwrap_or(t))
    }

    /// Sort key for album order: disc, then track number.
    pub fn disc_track(&self) -> (u32, u32) {
        let number = |tag: &str| {
            self.tag(tag)
                .and_then(|t| t.split('/').next())
                .and_then(|t| t.trim().parse().ok())
                .unwrap_or(0)
        };
        (number("Disc"), number("Track"))
    }
}

pub fn parse_songs(pairs: &[Pair]) -> Vec<Song> {
    split_groups(pairs, &["file"])
        .into_iter()
        .map(Song::from_pairs)
        .collect()
}

/// An entry of `lsinfo`.
#[derive(Clone, Debug, PartialEq)]
pub enum DirEntry {
    Directory(String),
    Song(Song),
    Playlist(String),
}

impl DirEntry {
    /// The library path of the entry.
    pub fn path(&self) -> &str {
        match self {
            Self::Directory(path) | Self::Playlist(path) => path,
            Self::Song(song) => &song.file,
        }
    }

    /// The last path component, as shown in the browser.
    pub fn name(&self) -> &str {
        let path = self.path();
        path.rsplit('/').next().unwrap_or(path)
    }
}

pub fn parse_entries(pairs: &[Pair]) -> Vec<DirEntry> {
    split_groups(pairs, &["file", "directory", "playlist"])
        .into_iter()
        .filter_map(|group| {
            let (key, value) = group.first()?;
            Some(match key.as_str() {
                "directory" => DirEntry::Directory(value.clone()),
                "playlist" => DirEntry::Playlist(value.clone()),
                _ => DirEntry::Song(Song::from_pairs(group)),
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayState {
    #[default]
    Stop,
    Play,
    Pause,
}

/// The tri-state used by `single` and `consume`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OnOff {
    #[default]
    Off,
    On,
    Oneshot,
}

impl OnOff {
    fn parse(value: &str) -> Self {
        match value {
            "1" => Self::On,
            "oneshot" => Self::Oneshot,
            _ => Self::Off,
        }
    }

    pub fn is_on(self) -> bool {
        self != Self::Off
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    /// `None` when the server has no mixer.
    pub volume: Option<u8>,
    pub repeat: bool,
    pub random: bool,
    pub single: OnOff,
    pub consume: OnOff,
    /// The queue version, bumped on every queue change.
    pub playlist_version: u32,
    pub playlist_length: u32,
    pub state: PlayState,
    /// Queue position of the current song.
    pub song: Option<u32>,
    pub song_id: Option<u32>,
    pub elapsed: Duration,
    pub duration: Option<Duration>,
    /// Kilobits per second.
    pub bitrate: Option<u32>,
    /// `samplerate:bits:channels`.
    pub audio: Option<String>,
    pub updating_db: bool,
    pub error: Option<String>,
}

impl Status {
    pub fn from_pairs(pairs: &[Pair]) -> Self {
        let mut status = Self::default();
        for (key, value) in pairs {
            match key.as_str() {
                "volume" => status.volume = value.parse().ok(),
                "repeat" => status.repeat = value == "1",
                "random" => status.random = value == "1",
                "single" => status.single = OnOff::parse(value),
                "consume" => status.consume = OnOff::parse(value),
                "playlist" => status.playlist_version = parse_or_default(value),
                "playlistlength" => status.playlist_length = parse_or_default(value),
                "state" => {
                    status.state = match value.as_str() {
                        "play" => PlayState::Play,
                        "pause" => PlayState::Pause,
                        _ => PlayState::Stop,
                    }
                }
                "song" => status.song = value.parse().ok(),
                "songid" => status.song_id = value.parse().ok(),
                "elapsed" => status.elapsed = parse_seconds(value).unwrap_or_default(),
                "duration" => status.duration = parse_seconds(value),
                "bitrate" => status.bitrate = value.parse().ok().filter(|&b| b > 0),
                "audio" => status.audio = Some(value.clone()),
                "updating_db" => status.updating_db = true,
                "error" => status.error = Some(value.clone()),
                _ => {}
            }
        }
        status
    }
}

/// A stored playlist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistInfo {
    pub name: String,
}

pub fn parse_playlists(pairs: &[Pair]) -> Vec<PlaylistInfo> {
    let mut playlists: Vec<PlaylistInfo> = pairs
        .iter()
        .filter(|(key, _)| key == "playlist")
        .map(|(_, name)| PlaylistInfo { name: name.clone() })
        .collect();
    playlists.sort_by_key(|p| p.name.to_lowercase());
    playlists
}

/// An album with its (optional) release date, from `list album group date`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Album {
    pub date: String,
    pub name: String,
}

pub fn parse_albums(pairs: &[Pair]) -> Vec<Album> {
    let mut albums = Vec::new();
    let mut date = String::new();
    for (key, value) in pairs {
        if key.eq_ignore_ascii_case("Date") {
            date.clone_from(value);
        } else if key.eq_ignore_ascii_case("Album") {
            albums.push(Album {
                date: date.clone(),
                name: value.clone(),
            });
        }
    }
    albums.sort();
    albums.dedup();
    albums
}

/// Values of a single tag from `list <tag>`.
pub fn parse_tag_values(pairs: &[Pair], tag: &str) -> Vec<String> {
    let mut values: Vec<String> = pairs
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(tag))
        .map(|(_, value)| value.clone())
        .collect();
    values.sort_by_cached_key(|v| v.to_lowercase());
    values.dedup();
    values
}

/// An MPD idle subsystem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Subsystem {
    Database,
    Update,
    StoredPlaylist,
    Playlist,
    Player,
    Mixer,
    Output,
    Options,
    Other,
}

impl FromStr for Subsystem {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "database" => Self::Database,
            "update" => Self::Update,
            "stored_playlist" => Self::StoredPlaylist,
            "playlist" => Self::Playlist,
            "player" => Self::Player,
            "mixer" => Self::Mixer,
            "output" => Self::Output,
            "options" => Self::Options,
            _ => Self::Other,
        })
    }
}

pub fn parse_subsystems(pairs: &[Pair]) -> Vec<Subsystem> {
    pairs
        .iter()
        .filter(|(key, _)| key == "changed")
        .filter_map(|(_, value)| value.parse().ok())
        .collect()
}

fn parse_seconds(value: &str) -> Option<Duration> {
    value
        .parse::<f64>()
        .ok()
        .filter(|s| s.is_finite() && *s >= 0.0)
        .map(Duration::from_secs_f64)
}

fn parse_or_default<T: FromStr + Default>(value: &str) -> T {
    value.parse().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(items: &[(&str, &str)]) -> Vec<Pair> {
        items
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn parses_queue_songs() {
        let songs = parse_songs(&pairs(&[
            ("file", "a/01 - One.flac"),
            ("Title", "One"),
            ("Artist", "X"),
            ("Track", "1/10"),
            ("duration", "61.5"),
            ("Pos", "0"),
            ("Id", "7"),
            ("file", "b/two.mp3"),
            ("Time", "30"),
        ]));
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].id, Some(7));
        assert_eq!(songs[0].track(), Some("1"));
        assert_eq!(songs[0].duration, Some(Duration::from_millis(61_500)));
        assert_eq!(songs[1].display_title(), "two");
        assert_eq!(songs[1].duration, Some(Duration::from_secs(30)));
        assert_eq!(songs[1].artist(), None);
    }

    #[test]
    fn parses_lsinfo_entries() {
        let entries = parse_entries(&pairs(&[
            ("directory", "music/rock"),
            ("Last-Modified", "2024"),
            ("file", "music/song.mp3"),
            ("Title", "S"),
            ("playlist", "mix"),
        ]));
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0], DirEntry::Directory("music/rock".into()));
        assert_eq!(entries[0].name(), "rock");
        assert!(matches!(&entries[1], DirEntry::Song(s) if s.title() == Some("S")));
        assert_eq!(entries[2], DirEntry::Playlist("mix".into()));
    }

    #[test]
    fn parses_status() {
        let status = Status::from_pairs(&pairs(&[
            ("volume", "-1"),
            ("repeat", "1"),
            ("single", "oneshot"),
            ("playlist", "12"),
            ("playlistlength", "3"),
            ("state", "pause"),
            ("songid", "4"),
            ("elapsed", "1.250"),
            ("bitrate", "0"),
        ]));
        assert_eq!(status.volume, None);
        assert!(status.repeat && !status.random);
        assert_eq!(status.single, OnOff::Oneshot);
        assert_eq!(status.playlist_version, 12);
        assert_eq!(status.state, PlayState::Pause);
        assert_eq!(status.elapsed, Duration::from_millis(1250));
        assert_eq!(status.bitrate, None);
    }

    #[test]
    fn parses_grouped_albums() {
        let albums = parse_albums(&pairs(&[
            ("Date", "2016"),
            ("Album", "B"),
            ("Date", "2001"),
            ("Album", "A"),
            ("Album", "C"),
        ]));
        let names: Vec<_> = albums.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["A", "C", "B"]);
    }

    #[test]
    fn parses_subsystems() {
        let subs = parse_subsystems(&pairs(&[("changed", "player"), ("changed", "mixer")]));
        assert_eq!(subs, [Subsystem::Player, Subsystem::Mixer]);
    }
}
