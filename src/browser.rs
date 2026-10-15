use std::path::Path;

use crate::{
    connection::{Connection, Song},
    utils::is_audio_file,
    AppResult,
};

#[derive(Debug, Default)]
pub struct FileBrowser {
    pub entries: Vec<BrowserEntry>,
    pub selected: usize,
    pub prev_selected: usize,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserEntry {
    Directory(String),
    Song(Song),
}

impl BrowserEntry {
    pub fn name(&self) -> &str {
        match self {
            Self::Directory(name) => name,
            Self::Song(song) => Path::new(&song.file)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&song.file),
        }
    }
}

impl FileBrowser {
    pub fn new() -> Self {
        Self {
            path: ".".to_string(),
            ..Self::default()
        }
    }

    pub async fn update_directory(&mut self, conn: &mut Connection) -> AppResult<()> {
        let query_path = self.path.trim_start_matches("./").trim_matches('/');
        let (directories, songs) = conn.client.ls_info(query_path).await?;
        let mut entries = Vec::new();
        for directory in directories {
            if let Some(name) = Path::new(&directory)
                .file_name()
                .and_then(|name| name.to_str())
            {
                if !name.starts_with('.') {
                    entries.push(BrowserEntry::Directory(name.to_string()));
                }
            }
        }

        for song in songs {
            let Some(name) = Path::new(&song.file)
                .file_name()
                .and_then(|name| name.to_str())
            else {
                continue;
            };
            if is_audio_file(name) {
                entries.push(BrowserEntry::Song(song));
            }
        }

        entries.sort_by(|a, b| {
            let kind_order = |entry: &BrowserEntry| match entry {
                BrowserEntry::Directory(_) => 0,
                BrowserEntry::Song(_) => 1,
            };
            kind_order(a).cmp(&kind_order(b)).then_with(|| {
                let name_a = a.name();
                let name_b = b.name();
                let number_a = name_a.parse::<u32>().unwrap_or(u32::MAX);
                let number_b = name_b.parse::<u32>().unwrap_or(u32::MAX);
                number_a
                    .cmp(&number_b)
                    .then_with(|| name_a.to_lowercase().cmp(&name_b.to_lowercase()))
            })
        });

        self.entries = entries;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        Ok(())
    }

    pub fn next(&mut self) {
        if self.selected < self.entries.len().saturating_sub(1) {
            self.selected += 1;
        }
    }

    pub fn prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub async fn handle_go_back(&mut self, conn: &mut Connection) -> AppResult<()> {
        if self.path != "." {
            self.path = Path::new(&self.path)
                .parent()
                .and_then(Path::to_str)
                .filter(|path| !path.is_empty())
                .unwrap_or(".")
                .to_string();
            self.update_directory(conn).await?;
        }
        self.selected = self.prev_selected.min(self.entries.len().saturating_sub(1));
        Ok(())
    }
}
