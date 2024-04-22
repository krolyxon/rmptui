use mpd::song::Song;
use mpd::{Client, State};
use simple_dmenu::dmenu;

pub struct Connection {
    pub conn: Client,
    pub songs_filenames: Vec<String>,
}

impl Connection {
    pub fn new(addrs: &str) -> Result<Self, mpd::error::Error> {
        let mut conn = Client::connect(addrs)?;
        let songs_filenames: Vec<String> = conn
            .listall()
            .unwrap()
            .into_iter()
            .map(|x| x.file)
            .collect();

        Ok(Self {
            conn,
            songs_filenames,
        })
    }

    pub fn play_fzf(&mut self) {
        let ss = &self.songs_filenames;
        let fzf_choice = rust_fzf::select(ss.clone(), Vec::new()).unwrap();
        let index = get_choice_index(&self.songs_filenames, fzf_choice.get(0).unwrap());
        let song = self.get_song_with_only_filename(ss.get(index).unwrap());
        self.push(&song);
    }

    pub fn play_dmenu(&mut self) {
        let ss: Vec<&str> = self.songs_filenames.iter().map(|x| x.as_str()).collect();
        let op = dmenu!(iter &ss; args "-l", "30");
        let index = get_choice_index(&self.songs_filenames, &op);
        let song = self.get_song_with_only_filename(ss.get(index).unwrap());
        self.push(&song);
    }

    fn push(&mut self, song: &Song) {
        if self.conn.queue().unwrap().is_empty() {
            self.conn.push(song).unwrap();
            self.conn.play().unwrap();
        } else {
            self.conn.push(song).unwrap();
            if self.conn.status().unwrap().state == State::Stop {
                self.conn.play().unwrap();
            }
            self.conn.next().unwrap();
        }
    }

    fn get_song_with_only_filename(&self, filename: &str) -> Song {
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

    // Playback controls
    pub fn pause(&mut self) {
        self.conn.pause(true).unwrap();
    }

    pub fn toggle_pause(&mut self) {
        self.conn.toggle_pause().unwrap();
    }

    // Volume controls
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

fn get_choice_index(ss: &Vec<String>, selection: &str) -> usize {
    let mut choice: usize = 0;
    if let Some(index) = ss.iter().position(|s| s == selection) {
        choice = index;
    }

    choice
}
