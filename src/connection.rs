use mpd::song::Song;
use mpd::Client;

pub struct Connection {
    pub conn: Client,
}

impl Connection {
    pub fn new(addrs: &str) -> Result<Self, mpd::error::Error> {
        let conn = Client::connect(addrs)?;
        Ok(Self { conn })
    }

    pub fn play_fzf(&mut self) {
        let mut ss: Vec<String> = Vec::new();
        self.get_file_tree_into_vec(&mut ss, ".", ".");

        let choice: usize;
        let fzf_choice = rust_fzf::select(ss.clone(), Vec::new()).unwrap();
        if let Some(index) = ss.iter().position(|s| s == fzf_choice.get(0).unwrap()) {
            choice = index;
        } else {
            return;
        }

        let song = self.get_song(ss.get(choice).unwrap());

        if self.conn.queue().unwrap().is_empty() {
            self.push(song);
            self.conn.play().unwrap();
        } else {
            self.push(song);
            self.conn.next().unwrap();
        }
    }

    fn get_file_tree_into_vec(&mut self, vec: &mut Vec<String>, path: &str, dir_append: &str) {
        let songs = self.conn.listfiles(path).unwrap_or_default();
        for (i, s) in songs {
            // Output of listfiles contains the last-modified thing, we dont want that
            if i != "Last-Modified" {
                if i == "directory" {
                    self.get_file_tree_into_vec(vec, &s, &s);
                } else {
                    // We parse the string as float because the output of listfiles contains some random numbers, we dont want that
                    if !s.parse::<f32>().is_ok() {
                        let mut sam: String = String::new();
                        sam.push_str(dir_append);
                        sam.push_str(r"/");
                        sam.push_str(s.as_str());
                        vec.push(sam);
                    }
                }
            }
        }
    }

    fn push(&mut self, song: Song) {
            self.conn.push(song).unwrap();

    }

    fn get_song(&self, filename: &str) -> Song {
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

        if current_song.is_ok() {
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
}
