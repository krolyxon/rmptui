use std::path::Path;

use mpd::Song;

use crate::{app::AppResult, connection::Connection, utils::FileExtension};

#[derive(Debug)]
/// struct for working with directory browser tab in rmptui
pub struct FileBrowser {
    pub filetree: Vec<(String, String)>,
    pub selected: usize,
    pub prev_selected: usize,
    pub path: String,
    pub prev_path: String,
    pub songs: Vec<Song>,
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
        let mut file_vec: Vec<(String, String)> = vec![];
        let mut dir_vec: Vec<(String, String)> = vec![];
        for (t, f) in conn.conn.listfiles(self.path.as_str())?.into_iter() {
            if t == "directory" && !f.starts_with('.') {
                dir_vec.push((t, f));
            } else if t == "file"
                && Path::new(&f).has_extension(&[
                    "mp3", "ogg", "flac", "m4a", "wav", "aac", "opus", "ape", "wma", "mpc", "aiff",
                    "dff", "mp2", "mka",
                ])
            {
                file_vec.push((t, f));
            }
        }

        // dir_vec.sort_by(|a, b| a.1.cmp(&b.1));
        dir_vec.sort_by(|a, b| {
            let num_a = a.1.parse::<u32>().unwrap_or(u32::MAX);
            let num_b = b.1.parse::<u32>().unwrap_or(u32::MAX);
            num_a
                .cmp(&num_b)
                .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
        });

        file_vec.sort_by(|a, b| {
            let num_a = a.1.parse::<u32>().unwrap_or(u32::MAX);
            let num_b = b.1.parse::<u32>().unwrap_or(u32::MAX);
            num_a
                .cmp(&num_b)
                .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
        });

        dir_vec.extend(file_vec);
        self.filetree = dir_vec;

        // Add metadata
        self.songs.clear();
        for (t, song) in self.filetree.iter() {
            if t == "file" {
                let v = conn
                    .conn
                    .lsinfo(Song {
                        file: (self.path.clone() + "/" + song)
                            .strip_prefix("./")
                            .unwrap_or("")
                            .to_string(),
                        ..Default::default()
                    })
                    .unwrap_or_else(|_| {
                        vec![Song {
                            file: "Not a song".to_string(),
                            ..Default::default()
                        }]
                    });

                self.songs.push(v.first().unwrap().clone());
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
        if self.selected < self.filetree.len() - 1 {
            self.selected += 1;
        }
    }

    /// Go to previous item in filetree
    pub fn prev(&mut self) {
        if self.selected != 0 {
            self.selected -= 1;
        }
    }

    /// handles going back event
    pub fn handle_go_back(&mut self, conn: &mut Connection) -> AppResult<()> {
        if self.prev_path != "." {
            let r = self.path.rfind('/');
            if let Some(r) = r {
                self.path = self.path.as_str()[..r].to_string();
                self.update_directory(conn)?;
            }
        } else {
            self.path.clone_from(&self.prev_path);
            self.update_directory(conn)?;
        }

        self.selected = self.prev_selected;
        Ok(())
    }
}

impl Default for FileBrowser {
    fn default() -> Self {
        Self::new()
    }
}
