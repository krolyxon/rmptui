use std::ffi::OsStr;
use std::path::Path;

use mpd::Song;

use crate::{app::AppResult, connection::Connection};

#[derive(Debug)]
pub struct FileBrowser {
    pub filetree: Vec<(String, String)>,
    pub selected: usize,
    pub prev_selected: usize,
    pub path: String,
    pub prev_path: String,
    pub songs: Vec<Song>,
}

// https://stackoverflow.com/questions/72392835/check-if-a-file-is-of-a-given-type
pub trait FileExtension {
    fn has_extension<S: AsRef<str>>(&self, extensions: &[S]) -> bool;
}

impl<P: AsRef<Path>> FileExtension for P {
    fn has_extension<S: AsRef<str>>(&self, extensions: &[S]) -> bool {
        if let Some(ref extension) = self.as_ref().extension().and_then(OsStr::to_str) {
            return extensions
                .iter()
                .any(|x| x.as_ref().eq_ignore_ascii_case(extension));
        }

        false
    }
}

impl FileBrowser {
    pub fn new() -> FileBrowser {
        FileBrowser {
            filetree: Vec::new(),
            selected: 0,
            prev_selected: 0,
            path: ".".to_string(),
            prev_path: ".".to_string(),
            songs: vec![],
        }
    }

    pub fn update_directory(&mut self, conn: &mut Connection) -> AppResult<()> {
        self.filetree.clear();
        self.filetree = conn
            .conn
            .listfiles(self.path.as_str())?
            .into_iter()
            .filter(|(f, l)| {
                f == "directory"
                    || f == "file" && Path::new(l).has_extension(&["mp3", "ogg", "flac", "m4a", "wav", "aac" ,"opus", "ape", "wma", "mpc", "aiff", "dff", "mp2", "mka"])
            })
            .collect::<Vec<(String, String)>>();

        self.songs.clear();
        for (t, song) in self.filetree.iter() {
            if t == "file" {
                let v = conn
                    .conn
                    .lsinfo(Song {
                        file: conn
                            .get_full_path(song)
                            .unwrap_or_else(|| "Not a song".to_string()),
                        ..Default::default()
                    })
                    .unwrap_or_else(|_| {
                        vec![Song {
                            file: "Not a song".to_string(),
                            ..Default::default()
                        }]
                    });

                self.songs.push(v.get(0).unwrap().clone());
            } else {
                let v = Song {
                    file: "".to_string(),
                    ..Default::default()
                };

                self.songs.push(v);
            }
        }

        Ok(())
    }

    // Go to next item in filetree
    pub fn next(&mut self) {
        // if self.selected < self.filetree.len() - 1 {
        //     self.selected += 1;
        // }

        if self.selected == self.filetree.len() - 1 {
            self.selected = 0;
        } else {
            self.selected += 1;
        }
    }

    /// Go to previous item in filetree
    pub fn prev(&mut self) {
        if self.selected == 0 {
            self.selected = self.filetree.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    pub fn handle_go_back(&mut self, conn: &mut Connection) -> AppResult<()> {
        if self.prev_path != "." {
            let r = self.path.rfind("/").unwrap();
            self.path = self.path.as_str()[..r].to_string();
            self.update_directory(conn)?;
        } else {
            self.path = self.prev_path.clone();
            self.update_directory(conn)?;
        }

        self.selected = self.prev_selected;
        Ok(())
    }
}
