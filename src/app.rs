use std::time::Duration;

use crate::{
    browser::{BrowserEntry, FileBrowser},
    config::Config,
    connection::{Connection, MpdSnapshot, Song},
    list::ContentList,
    ui::InputMode,
    utils::is_audio_file,
    AppResult,
};
use ratatui::widgets::{ListState, TableState};

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub conn: Connection,
    pub config: Config,
    pub browser: FileBrowser,
    pub queue_list: ContentList<Song>,
    pub pl_list: ContentList<String>,
    pub playlist_focus: PlaylistFocus,
    pub playlist_song_index: usize,
    pub selected_tab: SelectedTab,
    pub inputmode: InputMode,
    pub search_input: String,
    pub search_cursor_pos: usize,
    pub browser_search_results: Vec<usize>,
    pub browser_search_index: usize,
    pub pl_newname_input: String,
    pub pl_cursor_pos: usize,
    pub pl_new_pl_input: String,
    pub pl_new_pl_cursor_pos: usize,
    pub pl_new_pl_songs_buffer: Vec<Song>,
    pub playlist_popup: bool,
    pub append_list: ContentList<String>,
    pub should_update_song_list: bool,
    pub queue_state: TableState,
    pub browser_state: TableState,
    pub playlists_state: ListState,
    pub playlist_songs_state: TableState,
    pub playlist_contents: Vec<Song>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SelectedTab {
    DirectoryBrowser,
    Queue,
    Playlists,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum PlaylistFocus {
    #[default]
    List,
    Songs,
}

impl App {
    pub async fn new(address: &str, password: Option<&str>, config: Config) -> AppResult<Self> {
        let mut conn = Connection::connect(address, password).await?;
        let mut queue_list = ContentList::new();
        queue_list.list = conn.client.queue().await?;
        let mut pl_list = ContentList::new();
        pl_list.list = conn.client.playlists().await?;
        pl_list.list.sort();
        let append_list = Self::get_append_list(&mut conn).await?;
        let mut browser = FileBrowser::new();
        browser.update_directory(&mut conn).await?;

        if let Some(position) = conn.current_song.place {
            queue_list.index = position.pos as usize;
        }

        let mut app = Self {
            running: true,
            conn,
            config,
            browser,
            queue_list,
            pl_list,
            playlist_focus: PlaylistFocus::List,
            playlist_song_index: 0,
            selected_tab: SelectedTab::Queue,
            inputmode: InputMode::Normal,
            search_input: String::new(),
            search_cursor_pos: 0,
            browser_search_results: Vec::new(),
            browser_search_index: 0,
            pl_newname_input: String::new(),
            pl_cursor_pos: 0,
            pl_new_pl_input: String::new(),
            pl_new_pl_cursor_pos: 0,
            pl_new_pl_songs_buffer: Vec::new(),
            playlist_popup: false,
            append_list,
            should_update_song_list: false,
            queue_state: TableState::default(),
            browser_state: TableState::default(),
            playlists_state: ListState::default(),
            playlist_songs_state: TableState::default(),
            playlist_contents: Vec::new(),
        };
        app.refresh_playlist_view().await?;
        Ok(app)
    }

    pub async fn apply_snapshot(&mut self, snapshot: MpdSnapshot) -> AppResult<()> {
        let MpdSnapshot {
            status,
            stats,
            current_song,
            queue,
        } = snapshot;
        self.conn.apply_snapshot(status, stats, current_song);
        self.queue_list.list = queue;
        if self.queue_list.list.is_empty() {
            self.queue_list.index = 0;
        } else {
            self.queue_list.index = self.queue_list.index.min(self.queue_list.list.len() - 1);
        }

        if self.should_update_song_list && self.conn.status.updating_db.is_none() {
            self.conn.refresh_song_list().await?;
            self.browser.update_directory(&mut self.conn).await?;
            self.should_update_song_list = false;
        }
        Ok(())
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub async fn update_queue(&mut self) -> AppResult<()> {
        self.queue_list.list = self.conn.client.queue().await?;
        if self.queue_list.list.is_empty() {
            self.queue_list.index = 0;
        } else {
            self.queue_list.index = self.queue_list.index.min(self.queue_list.list.len() - 1);
        }
        Ok(())
    }

    pub async fn refresh_playlists(&mut self) -> AppResult<()> {
        self.pl_list.list = self.conn.client.playlists().await?;
        self.pl_list.list.sort();
        self.pl_list.index = self
            .pl_list
            .index
            .min(self.pl_list.list.len().saturating_sub(1));
        self.append_list = Self::get_append_list(&mut self.conn).await?;
        self.refresh_playlist_view().await
    }

    pub async fn refresh_playlist_view(&mut self) -> AppResult<()> {
        self.playlist_contents = match self.pl_list.list.get(self.pl_list.index) {
            Some(name) => self.conn.client.playlist(name).await?,
            None => Vec::new(),
        };
        self.playlist_song_index = self
            .playlist_song_index
            .min(self.playlist_contents.len().saturating_sub(1));
        Ok(())
    }

    pub async fn delete_selected_playlist_song(&mut self) -> AppResult<()> {
        let Some(playlist) = self.pl_list.list.get(self.pl_list.index) else {
            return Ok(());
        };
        if self.playlist_song_index >= self.playlist_contents.len() {
            return Ok(());
        }
        self.conn
            .client
            .delete_playlist_song(playlist, self.playlist_song_index)
            .await?;
        self.refresh_playlist_view().await
    }

    pub async fn move_selected_playlist_song(&mut self, down: bool) -> AppResult<()> {
        let Some(playlist) = self.pl_list.list.get(self.pl_list.index) else {
            return Ok(());
        };
        let target = if down {
            self.playlist_song_index.checked_add(1)
        } else {
            self.playlist_song_index.checked_sub(1)
        };
        let Some(target) = target.filter(|target| *target < self.playlist_contents.len()) else {
            return Ok(());
        };
        self.conn
            .client
            .move_playlist_song(playlist, self.playlist_song_index, target)
            .await?;
        self.playlist_song_index = target;
        self.refresh_playlist_view().await
    }

    pub async fn delete_selected_playlist(&mut self) -> AppResult<()> {
        let Some(playlist) = self.pl_list.list.get(self.pl_list.index) else {
            return Ok(());
        };
        self.conn.client.delete_playlist(playlist).await?;
        self.refresh_playlists().await?;
        Ok(())
    }

    pub async fn play_selected_playlist_song(&mut self) -> AppResult<()> {
        let Some(playlist) = self.pl_list.list.get(self.pl_list.index) else {
            return Ok(());
        };
        if self.playlist_song_index >= self.playlist_contents.len() {
            return Ok(());
        }
        let queue_position = self.conn.client.queue().await?.len();
        self.conn.client.append_playlist(playlist).await?;
        self.conn
            .client
            .play(Some((queue_position + self.playlist_song_index) as u32))
            .await?;
        self.update_queue().await?;
        self.conn.update_status().await
    }

    pub async fn get_append_list(conn: &mut Connection) -> AppResult<ContentList<String>> {
        let mut list = ContentList::new();
        list.list.push("Current Playlist".to_string());
        list.list.push("New Playlist".to_string());
        list.list.extend(conn.client.playlists().await?);
        Ok(list)
    }

    pub async fn toggle_selected_item(&mut self) -> AppResult<()> {
        match self.selected_tab {
            SelectedTab::DirectoryBrowser => {
                let Some(entry) = self.browser.entries.get(self.browser.selected).cloned() else {
                    return Ok(());
                };
                match entry {
                    BrowserEntry::Directory(name) => {
                        let path = join_library_path(&self.browser.path, &name);
                        for (entry_type, file) in self.conn.client.list_files(&path).await? {
                            if entry_type == "file" && is_audio_file(&file) {
                                self.conn.client.add(&file).await?;
                            }
                        }
                    }
                    BrowserEntry::Song(song) => {
                        let positions: Vec<u32> = self
                            .queue_list
                            .list
                            .iter()
                            .enumerate()
                            .filter_map(|(index, queued)| {
                                (queued.file == song.file).then_some(index as u32)
                            })
                            .collect();
                        if positions.is_empty() {
                            self.conn.client.add(&song.file).await?;
                        } else {
                            for position in positions.into_iter().rev() {
                                self.conn.client.delete(position).await?;
                            }
                        }
                    }
                }
                if self.browser.selected < self.browser.entries.len().saturating_sub(1) {
                    self.browser.selected += 1;
                }
            }
            SelectedTab::Queue => {
                if !self.queue_list.list.is_empty() {
                    self.conn
                        .client
                        .delete(self.queue_list.index as u32)
                        .await?;
                }
            }
            SelectedTab::Playlists => {}
        }
        self.update_queue().await?;
        self.conn.update_status().await?;
        Ok(())
    }

    pub fn cycle_tabs(&mut self) {
        self.selected_tab = match self.selected_tab {
            SelectedTab::Queue => SelectedTab::DirectoryBrowser,
            SelectedTab::DirectoryBrowser => SelectedTab::Playlists,
            SelectedTab::Playlists => SelectedTab::Queue,
        };
    }

    pub async fn handle_enter(&mut self) -> AppResult<()> {
        let Some(entry) = self.browser.entries.get(self.browser.selected).cloned() else {
            return Ok(());
        };
        match entry {
            BrowserEntry::Directory(name) => {
                self.browser.prev_selected = self.browser.selected;
                self.browser.path = join_library_path(&self.browser.path, &name);
                self.browser.selected = 0;
                self.browser.update_directory(&mut self.conn).await?;
            }
            BrowserEntry::Song(song) => {
                if let Some(index) = self
                    .queue_list
                    .list
                    .iter()
                    .position(|queued| queued.file == song.file)
                {
                    self.conn.client.play(Some(index as u32)).await?;
                } else {
                    self.conn.push(&song).await?;
                    self.update_queue().await?;
                }
                self.conn.update_status().await?;
            }
        }
        Ok(())
    }

    pub async fn append_selected_to(&mut self) -> AppResult<()> {
        let Some(playlist) = self.append_list.list.get(self.append_list.index).cloned() else {
            return Ok(());
        };
        let songs = match self.selected_tab {
            SelectedTab::Queue => self
                .queue_list
                .list
                .get(self.queue_list.index)
                .cloned()
                .into_iter()
                .collect(),
            SelectedTab::DirectoryBrowser => {
                let Some(entry) = self.browser.entries.get(self.browser.selected).cloned() else {
                    return Ok(());
                };
                match entry {
                    BrowserEntry::Song(song) => vec![song],
                    BrowserEntry::Directory(name) => {
                        let path = join_library_path(&self.browser.path, &name);
                        self.conn
                            .client
                            .list_files(&path)
                            .await?
                            .into_iter()
                            .filter_map(|(kind, file)| {
                                (kind == "file" && is_audio_file(&file))
                                    .then(|| Connection::song_from_file(&file))
                            })
                            .collect()
                    }
                }
            }
            SelectedTab::Playlists => {
                let Some(name) = self.pl_list.list.get(self.pl_list.index) else {
                    return Ok(());
                };
                self.conn.client.playlist(name).await?
            }
        };

        if playlist == "Current Playlist" {
            if self.selected_tab == SelectedTab::Playlists {
                if let Some(name) = self.pl_list.list.get(self.pl_list.index) {
                    self.conn.load_playlist(name).await?;
                    self.update_queue().await?;
                }
            } else {
                for song in songs {
                    self.conn.client.add(&song.file).await?;
                }
                self.update_queue().await?;
            }
        } else if playlist == "New Playlist" {
            self.pl_new_pl_songs_buffer = songs;
            self.inputmode = InputMode::NewPlaylist;
        } else {
            for song in songs {
                self.conn.add_to_playlist(&playlist, &song).await?;
            }
            self.refresh_playlists().await?;
        }
        self.playlist_popup = false;
        self.append_list.index = 0;
        Ok(())
    }

    pub fn move_cursor_left(&mut self) {
        let cursor = self.cursor_position().saturating_sub(1);
        self.set_cursor_position(cursor);
    }

    pub fn move_cursor_right(&mut self) {
        let cursor = self.cursor_position().saturating_add(1);
        self.set_cursor_position(cursor);
    }

    fn cursor_position(&self) -> usize {
        match self.inputmode {
            InputMode::PlaylistRename => self.pl_cursor_pos,
            InputMode::Editing => self.search_cursor_pos,
            InputMode::NewPlaylist => self.pl_new_pl_cursor_pos,
            InputMode::Normal => 0,
        }
    }

    fn set_cursor_position(&mut self, position: usize) {
        let length = match self.inputmode {
            InputMode::PlaylistRename => self.pl_newname_input.chars().count(),
            InputMode::Editing => self.search_input.chars().count(),
            InputMode::NewPlaylist => self.pl_new_pl_input.chars().count(),
            InputMode::Normal => 0,
        };
        let position = position.min(length);
        match self.inputmode {
            InputMode::PlaylistRename => self.pl_cursor_pos = position,
            InputMode::Editing => self.search_cursor_pos = position,
            InputMode::NewPlaylist => self.pl_new_pl_cursor_pos = position,
            InputMode::Normal => {}
        }
    }

    pub fn enter_char(&mut self, character: char) {
        let (text, cursor) = match self.inputmode {
            InputMode::PlaylistRename => (&mut self.pl_newname_input, self.pl_cursor_pos),
            InputMode::Editing => (&mut self.search_input, self.search_cursor_pos),
            InputMode::NewPlaylist => (&mut self.pl_new_pl_input, self.pl_new_pl_cursor_pos),
            InputMode::Normal => return,
        };
        let byte_index = text
            .char_indices()
            .nth(cursor)
            .map(|(index, _)| index)
            .unwrap_or(text.len());
        text.insert(byte_index, character);
        self.move_cursor_right();
    }

    pub fn delete_char(&mut self) {
        let cursor = self.cursor_position();
        if cursor == 0 {
            return;
        }
        let text = match self.inputmode {
            InputMode::PlaylistRename => &mut self.pl_newname_input,
            InputMode::Editing => &mut self.search_input,
            InputMode::NewPlaylist => &mut self.pl_new_pl_input,
            InputMode::Normal => return,
        };
        let start = text.char_indices().nth(cursor - 1).map(|(i, _)| i);
        let end = text
            .char_indices()
            .nth(cursor)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        if let Some(start) = start {
            text.replace_range(start..end, "");
            self.move_cursor_left();
        }
    }

    pub fn reset_cursor(&mut self) {
        self.set_cursor_position(0);
    }

    pub fn format_time(time: Duration) -> String {
        let seconds = time.as_secs();
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let seconds = seconds % 60;
        if hours == 0 {
            format!("{minutes:02}:{seconds:02}")
        } else {
            format!("{hours:02}:{minutes:02}:{seconds:02}")
        }
    }

    pub fn handle_scroll_up(&mut self) {
        match self.selected_tab {
            SelectedTab::Queue => self.queue_list.prev(),
            SelectedTab::DirectoryBrowser => self.browser.prev(),
            SelectedTab::Playlists => self.pl_list.prev(),
        }
    }

    pub fn handle_scroll_down(&mut self) {
        match self.selected_tab {
            SelectedTab::Queue => self.queue_list.next(),
            SelectedTab::DirectoryBrowser => self.browser.next(),
            SelectedTab::Playlists => self.pl_list.next(),
        }
    }
}

pub fn join_library_path(parent: &str, child: &str) -> String {
    let parent = parent.trim_matches('/').trim_start_matches("./");
    if parent.is_empty() || parent == "." {
        child.trim_start_matches("./").to_string()
    } else {
        format!("{parent}/{}", child.trim_start_matches("./"))
    }
}
