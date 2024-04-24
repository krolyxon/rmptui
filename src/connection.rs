use mpd::song::Song;
use mpd::{Client, State};
use simple_dmenu::dmenu;
use std::process::Command;

pub type Result<T> = core::result::Result<T, Error>;
pub type Error = Box<dyn std::error::Error>;

#[derive(Debug)]
pub struct Connection {
    pub conn: Client,
    pub songs_filenames: Vec<String>,
    // pub state: String,
}

impl Connection {
    /// Create a new connection
    pub fn new(addrs: &str) -> Result<Self> {
        let mut conn = Client::connect(addrs).unwrap();
        let songs_filenames: Vec<String> = conn
            .listall()
            .unwrap()
            .into_iter()
            .map(|x| x.file)
            .collect();

        Ok(Self {
            conn,
            songs_filenames,
            // state: "Stopped".to_string(),
        })
    }

    /// Fzf prompt for selecting song
    pub fn play_fzf(&mut self) -> Result<()> {
        is_installed("fzf").map_err(|ex| ex)?;

        let ss = &self.songs_filenames;
        let fzf_choice = rust_fzf::select(ss.clone(), Vec::new()).unwrap();
        let index = get_choice_index(&self.songs_filenames, fzf_choice.get(0).unwrap());
        let song = self.get_song_with_only_filename(ss.get(index).unwrap());
        self.push(&song)?;

        Ok(())
    }

    /// Dmenu prompt for selecting songs
    pub fn play_dmenu(&mut self) -> Result<()> {
        is_installed("dmenu").map_err(|ex| ex)?;
        let ss: Vec<&str> = self.songs_filenames.iter().map(|x| x.as_str()).collect();
        let op = dmenu!(iter &ss; args "-l", "30");
        let index = get_choice_index(&self.songs_filenames, &op);
        let song = self.get_song_with_only_filename(ss.get(index).unwrap());
        self.push(&song)?;
        Ok(())
    }

    // pub fn update_state(&mut self) {
    //     match self.conn.status().unwrap().state {
    //         State::Stop => self.state = "Stopped".to_string(),
    //         State::Play => self.state = "Playing".to_string(),
    //         State::Pause => self.state = "Paused".to_string(),
    //     }
    // }

    /// push the given song to queue
    pub fn push(&mut self, song: &Song) -> Result<()> {
        if self.conn.queue().unwrap().is_empty() {
            self.conn.push(song).unwrap();
            self.conn.play().unwrap();
        } else {
            self.conn.push(song)?;
            if self.conn.status()?.state == State::Stop {
                self.conn.play()?;
            }
            self.conn.next().unwrap();
        }

        Ok(())
    }

    /// Push all songs of a playlist into queue
    pub fn push_playlist(&mut self, playlist: &str) -> Result<()> {
        let songs: Vec<Song> = self.conn.playlist(playlist)?;

        for song in songs {
            if self.songs_filenames.contains(&song.file) {
                let song = self.get_song_with_only_filename(&song.file);
                self.conn.push(&song)?;
                self.conn.play()?;
            }
        }
        Ok(())
    }

    /// Given a filename, get instance of Song with only filename
    pub fn get_song_with_only_filename(&self, filename: &str) -> Song {
        Song {
            file: filename.to_string(),
            artist: None,
            title: None,
            duration: None,
            last_mod: None,
            name: None,
            place: None,
            range: None,
            tags: vec![("".to_string(), "".to_string())],
        }
    }

    /// get current playing song
    pub fn get_current_song(&mut self) -> Option<String> {
        self.conn.currentsong().unwrap().unwrap_or_default().title
    }

    /// Print status to stdout
    pub fn status(&mut self) {
        let current_song = self.conn.currentsong();
        let status = self.conn.status().unwrap();

        if current_song.is_ok() && status.state != State::Stop {
            let song = current_song.unwrap();
            if let Some(s) = song {
                println!("{} - {}", s.artist.unwrap(), s.title.unwrap());
            }
        }
        println!(
            "volume: {}\trepeat: {}\trandom: {}\tsingle: {}\tconsume: {}",
            status.volume, status.repeat, status.random, status.single, status.consume
        );
    }

    /// Gives title of current playing song
    pub fn now_playing(&mut self) -> Result<Option<String>> {
        let song = self.conn.currentsong()?.unwrap_or_default();
        if let Some(s) = song.title {
            if let Some(a) = song.artist {
                return Ok(Some(format!("{} - {}", s, a)));
            } else {
                return Ok(Some(s));
            }
        } else {
            return Ok(Some(song.file));
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

    // Volume controls
    /// Sets the volume
    pub fn set_volume(&mut self, u: String) {
        let cur = self.conn.status().unwrap().volume;
        let sym = u.get(0..1).unwrap();
        let u: i8 = u.parse::<i8>().unwrap();
        if sym == "+" || sym == "-" {
            self.conn.volume(cur + u).unwrap();
        } else {
            self.conn.volume(u).unwrap();
        }
    }
}

/// Gets the index of the string from the Vector
fn get_choice_index(ss: &Vec<String>, selection: &str) -> usize {
    let mut choice: usize = 0;
    if let Some(index) = ss.iter().position(|s| s == selection) {
        choice = index;
    }

    choice
}

/// Checks if given program is installed in your system
fn is_installed(ss: &str) -> Result<()> {
    let output = Command::new("which")
        .arg(ss)
        .output()
        .expect("Failed to execute command");
    if output.status.success() {
        Ok(())
    } else {
        let err = format!("{} not installed", ss);
        Err(err.into())
    }
}
