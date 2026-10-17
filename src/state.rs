//! The client-side mirror of the server state.

use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::mpd::{PlayState, PlaylistInfo, Song, Status};

#[derive(Debug)]
pub struct MpdState {
    pub connected: bool,
    pub status: Status,
    /// When `status` was received, for interpolating the elapsed time.
    status_at: Instant,
    pub current: Option<Song>,
    pub queue: Vec<Song>,
    /// The queue version `queue` reflects; `None` until first loaded.
    queue_version: Option<u32>,
    /// Queue ids by file, for "is this song queued?" checks.
    queued: HashMap<String, u32>,
    pub playlists: Vec<PlaylistInfo>,
    /// Bumped whenever the music database changes, to invalidate caches.
    pub db_version: u64,
    pub music_dir: Option<PathBuf>,
}

impl Default for MpdState {
    fn default() -> Self {
        Self {
            connected: false,
            status: Status::default(),
            status_at: Instant::now(),
            current: None,
            queue: Vec::new(),
            queue_version: None,
            queued: HashMap::new(),
            playlists: Vec::new(),
            db_version: 0,
            music_dir: None,
        }
    }
}

impl MpdState {
    /// Elapsed time of the current song, interpolated while playing.
    pub fn elapsed(&self) -> Duration {
        let elapsed = match self.status.state {
            PlayState::Play => self.status.elapsed + self.status_at.elapsed(),
            _ => self.status.elapsed,
        };
        match self.status.duration {
            Some(duration) => elapsed.min(duration),
            None => elapsed,
        }
    }

    pub fn is_playing(&self) -> bool {
        self.status.state == PlayState::Play
    }

    /// Stores a fresh status. Returns whether the current song changed.
    pub fn set_status(&mut self, status: Status, current: Option<Song>) -> bool {
        let changed = self.current.as_ref().map(|s| (&s.file, s.id))
            != current.as_ref().map(|s| (&s.file, s.id));
        self.status = status;
        self.status_at = Instant::now();
        self.current = current;
        changed
    }

    /// The queue version to request changes from, if the queue is stale.
    pub fn stale_queue_since(&self) -> Option<u32> {
        match self.queue_version {
            None => Some(0),
            Some(version) if version != self.status.playlist_version => Some(version),
            Some(_) => None,
        }
    }

    /// Applies the result of `plchanges`.
    pub fn apply_queue_changes(&mut self, status: &Status, since: u32, changes: Vec<Song>) {
        if since == 0 {
            self.queue.clear();
        } else if self.queue_version != Some(since) {
            // A diff against a version we no longer have.
            return;
        }
        let length = status.playlist_length as usize;
        self.queue.resize_with(length, Song::default);
        for song in changes {
            if let Some(slot) = song.pos.and_then(|pos| self.queue.get_mut(pos as usize)) {
                *slot = song;
            }
        }
        self.queue_version = Some(status.playlist_version);
        self.queued.clear();
        for song in &self.queue {
            if let Some(id) = song.id {
                self.queued.entry(song.file.clone()).or_insert(id);
            }
        }
    }

    /// Forgets everything, e.g. after a reconnect.
    pub fn reset_queue(&mut self) {
        self.queue_version = None;
    }

    /// The queue id of `file`, if it is queued.
    pub fn queued_id(&self, file: &str) -> Option<u32> {
        self.queued.get(file).copied()
    }

    pub fn is_queued(&self, file: &str) -> bool {
        self.queued.contains_key(file)
    }

    pub fn is_current(&self, song: &Song) -> bool {
        song.id.is_some() && song.id == self.status.song_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(file: &str, pos: u32, id: u32) -> Song {
        Song {
            file: file.into(),
            pos: Some(pos),
            id: Some(id),
            ..Song::default()
        }
    }

    fn status(version: u32, length: u32) -> Status {
        Status {
            playlist_version: version,
            playlist_length: length,
            ..Status::default()
        }
    }

    #[test]
    fn applies_full_and_incremental_changes() {
        let mut state = MpdState::default();
        assert_eq!(state.stale_queue_since(), Some(0));
        state.apply_queue_changes(
            &status(5, 3),
            0,
            vec![song("a", 0, 1), song("b", 1, 2), song("c", 2, 3)],
        );
        state.status = status(5, 3);
        assert_eq!(state.stale_queue_since(), None);
        assert_eq!(state.queued_id("b"), Some(2));

        // "b" deleted: "c" moves up and the queue shrinks.
        state.apply_queue_changes(&status(6, 2), 5, vec![song("c", 1, 3)]);
        let files: Vec<_> = state.queue.iter().map(|s| s.file.as_str()).collect();
        assert_eq!(files, ["a", "c"]);
        assert!(!state.is_queued("b"));

        // A diff against an unknown version is ignored.
        state.apply_queue_changes(&status(9, 1), 7, vec![]);
        assert_eq!(state.queue.len(), 2);
    }

    #[test]
    fn detects_song_changes() {
        let mut state = MpdState::default();
        assert!(state.set_status(Status::default(), Some(song("a", 0, 1))));
        assert!(!state.set_status(Status::default(), Some(song("a", 0, 1))));
        assert!(state.set_status(Status::default(), None));
    }
}
