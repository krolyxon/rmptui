use crate::app::AppResult;
use crate::utils::is_installed;
use mpd::song::Song;
use mpd::{Client, State};
use simple_dmenu::dmenu;
use std::time::Duration;

/// Defines the current status of volume (Muted or UnMuted)
#[derive(Debug)]
pub enum VolumeStatus {
    Muted(i8),
    Unmuted,
}

#[derive(Debug)]
/// struct storing the mpd Client related stuff
pub struct Connection {
    pub conn: Client,
    pub songs_filenames: Vec<String>,
    pub state: String,
    pub elapsed: Duration,
    pub total_duration: Duration,
    pub current_song: Song,
    pub stats: mpd::Stats,
    pub status: mpd::Status,
    pub volume_status: VolumeStatus,
}

impl Connection {
    /// Create a new connection
    pub fn builder(addrs: &str) -> AppResult<Self> {
        let mut conn = Client::connect(addrs).unwrap_or_else(|_| {
            eprintln!("Error connecting to mpd server, Make sure mpd is running");
            std::process::exit(1);
        });

        let empty_song = Song {
            file: "No Song playing or in Queue".to_string(),
            ..Default::default()
        };

        let songs_filenames: Vec<String> = conn.listall()?.into_iter().map(|x| x.file).collect();

        let status = conn.status().unwrap();
        let (elapsed, total) = status.time.unwrap_or_default();
        let stats = conn.stats().unwrap_or_default();

        let current_song = conn
            .currentsong()
            .unwrap_or_else(|_| Some(empty_song.clone()))
            .unwrap_or(empty_song);

        let volume_status = VolumeStatus::Unmuted;

        Ok(Self {
            conn,
            songs_filenames,
            state: "Stopped".to_string(),
            elapsed,
            total_duration: total,
            current_song,
            stats,
            status,
            volume_status,
        })
    }

    /// Dmenu prompt for selecting songs
    pub fn play_dmenu(&mut self) -> AppResult<()> {
        if is_installed("dmenu") {
            let ss: Vec<&str> = self.songs_filenames.iter().map(|x| x.as_str()).collect();
            let op = dmenu!(iter &ss; args "-p", "Choose a song: ", "-l", "30");
            let index = ss.iter().position(|s| s == &op);
            if let Some(i) = index {
                let song = self.get_song_with_only_filename(ss.get(i).unwrap());
                self.push(&song)?;
            }
        }

        Ok(())
    }

    /// Update status
    pub fn update_status(&mut self) {
        let status = self.conn.status().unwrap();
        let stats = self.conn.stats().unwrap_or_default();

        let empty_song = self.get_song_with_only_filename("No Song playing or in Queue");

        let current_song = self
            .conn
            .currentsong()
            .unwrap_or_else(|_| Some(empty_song.clone()))
            .unwrap_or(empty_song);

        // Status
        self.status = status.clone();

        // Playback State
        self.state = match status.state {
            State::Stop => "Stopped".to_string(),
            State::Play => "Playing".to_string(),
            State::Pause => "Paused".to_string(),
        };

        // Progress
        let (elapsed, total) = status.time.unwrap_or_default();
        self.elapsed = elapsed;
        self.total_duration = total;

        // Current song
        self.current_song = current_song;

        // Stats
        self.stats = stats;
    }

    /// Get progress ratio of current playing song
    pub fn get_progress_ratio(&self) -> f64 {
        let total = self.total_duration.as_secs_f64();
        if total == 0.0 {
            0.0
        } else {
            let ratio = self.elapsed.as_secs_f64() / self.total_duration.as_secs_f64();
            if ratio > 1.0 || ratio == 0.0 {
                0.0
            } else {
                ratio
            }
        }
    }

    /// push the given song to queue
    pub fn push(&mut self, song: &Song) -> AppResult<()> {
        if self.conn.queue().unwrap().is_empty() {
            self.conn.push(song).unwrap();
            self.conn.play().unwrap();
        } else {
            self.conn.push(song)?;
            let len: u32 = (self.conn.queue().unwrap().len() - 1).try_into().unwrap();
            self.conn.switch(len)?;
            if self.conn.status()?.state == State::Stop {
                self.conn.play()?;
            }
        }

        Ok(())
    }

    /// Push all songs of a playlist into queue
    pub fn load_playlist(&mut self, playlist: &str) -> AppResult<()> {
        self.conn.load(playlist, ..)?;
        self.conn.play()?;
        Ok(())
    }

    /// Add given song to playlist
    pub fn add_to_playlist(&mut self, playlist: &str, song: &Song) -> AppResult<()> {
        self.conn.pl_push(playlist, song)?;
        Ok(())
    }

    /// Given a filename, get instance of Song with only filename
    pub fn get_song_with_only_filename(&self, filename: &str) -> Song {
        Song {
            file: filename.to_string(),
            ..Default::default()
        }
    }

    /// Given a song name from a directory, it returns the full path of the song in the database
    pub fn get_full_path(&self, short_path: &str) -> Option<&str> {
        for (i, f) in self.songs_filenames.iter().enumerate() {
            if f.contains(short_path) {
                return Some(self.songs_filenames.get(i).unwrap());
            }
        }
        None
    }

    /// Gives title of current playing song
    pub fn now_playing(&mut self) -> AppResult<Option<String>> {
        if let Some(s) = &self.current_song.title {
            if let Some(a) = &self.current_song.artist {
                Ok(Some(format!("\"{}\" By {}", s, a)))
            } else {
                Ok(Some(s.to_string()))
            }
        } else {
            Ok(Some(self.current_song.file.clone()))
        }
    }

    // Playback controls
    /// Pause playback
    pub fn pause(&mut self) {
        self.conn.pause(true).unwrap();
    }

    /// Toggles playback
    pub fn toggle_pause(&mut self) {
        self.conn.toggle_pause().unwrap();
    }

    /// Toggle Repeat mode
    pub fn toggle_repeat(&mut self) {
        let mode = self.status.repeat;
        self.conn.repeat(!mode).unwrap();
    }

    /// Toggle random mode
    pub fn toggle_random(&mut self) {
        let mode = self.status.random;
        self.conn.random(!mode).unwrap();
    }

    // Volume controls
    /// Increase Volume
    pub fn inc_volume(&mut self, v: i8) {
        let cur = self.status.volume;
        if cur + v <= 100 {
            self.volume_status = VolumeStatus::Unmuted;
            self.conn.volume(cur + v).unwrap();
        }
    }

    /// Decrease volume
    pub fn dec_volume(&mut self, v: i8) {
        let cur = self.status.volume;
        if cur - v >= 0 {
            self.volume_status = VolumeStatus::Unmuted;
            self.conn.volume(cur - v).unwrap();
        }
    }
}
