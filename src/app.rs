use std::{path::Path, time::Duration};

use crate::browser::FileBrowser;
use crate::connection::Connection;
use crate::list::ContentList;
use crate::ui::InputMode;
use crate::utils::FileExtension;
use mpd::{Client, Song};
use ratatui::widgets::{ListState, TableState};

// Application result type
pub type AppResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Application
#[derive(Debug)]
pub struct App {
    pub running: bool,                 // Check if app is running
    pub conn: Connection,              // Connection
    pub browser: FileBrowser,          // Directory browser
    pub queue_list: ContentList<Song>, // Stores the current playing queue
    pub pl_list: ContentList<String>,  // Stores list of playlists
    pub selected_tab: SelectedTab,     // Used to switch between tabs

    // Search
    pub inputmode: InputMode,     // Defines input mode, Normal or Search
    pub search_input: String,     // Stores the userinput to be searched
    pub search_cursor_pos: usize, // Stores the cursor position for searching

    pub pl_newname_input: String, // Stores the new name of the playlist
    pub pl_cursor_pos: usize,     // Stores the cursor position for renaming playlist

    pub pl_new_pl_input: String,  // Stores the name of new playlist to be created
    pub pl_new_pl_cursor_pos: usize, // Stores the cursor position of new playlist to be created
    pub pl_new_pl_songs_buffer: Vec<Song>, // Buffer for songs that need to be added to the newly created playlist

    // playlist variables
    // used to show playlist popup
    pub playlist_popup: bool,
    pub append_list: ContentList<String>,

    // Determines if the database should be updated or not
    pub should_update_song_list: bool,

    // States
    pub queue_state: TableState,
    pub browser_state: TableState,
    pub playlists_state: ListState,
}

#[derive(Debug, PartialEq, Clone)]
pub enum SelectedTab {
    DirectoryBrowser,
    Queue,
    Playlists,
}

impl App {
    pub fn builder(addrs: &str) -> AppResult<Self> {
        let mut conn = Connection::builder(addrs)?;
        let mut queue_list = ContentList::new();
        let mut pl_list = ContentList::new();

        pl_list.list = Self::get_playlist(&mut conn.conn)?;
        pl_list.list.sort();

        let append_list = Self::get_append_list(&mut conn.conn)?;
        Self::get_queue(&mut conn, &mut queue_list.list);

        let browser = FileBrowser::new();

        let queue_state = TableState::new();
        let browser_state = TableState::new();
        let playlists_state = ListState::default();

        Ok(Self {
            running: true,
            conn,
            queue_list,
            pl_list,
            selected_tab: SelectedTab::Queue,
            browser,
            inputmode: InputMode::Normal,
            search_input: String::new(),
            pl_newname_input: String::new(),
            search_cursor_pos: 0,
            pl_cursor_pos: 0,
            playlist_popup: false,
            pl_new_pl_input: String::new(),
            pl_new_pl_cursor_pos: 0,
            pl_new_pl_songs_buffer: Vec::new(),
            append_list,
            should_update_song_list: false,
            queue_state,
            browser_state,
            playlists_state,
        })
    }

    pub fn tick(&mut self) -> AppResult<()> {
        self.conn.update_status();
        self.update_queue();

        // Deals with database update
        if self.should_update_song_list {
            if let None = self.conn.status.updating_db {
                // Update the songs list
                self.conn.songs_filenames = self
                    .conn
                    .conn
                    .listall()?
                    .into_iter()
                    .map(|x| x.file)
                    .collect();

                self.browser.update_directory(&mut self.conn)?;

                self.should_update_song_list = false;
            }
        }

        Ok(())
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn get_queue(conn: &mut Connection, vec: &mut Vec<Song>) {
        conn.conn.queue().unwrap().into_iter().for_each(|x| {
            vec.push(x);
        });
    }

    // Rescan the queue into queue_list
    pub fn update_queue(&mut self) {
        self.queue_list.list.clear();
        Self::get_queue(&mut self.conn, &mut self.queue_list.list);
    }

    pub fn get_playlist(conn: &mut Client) -> AppResult<Vec<String>> {
        let list: Vec<String> = conn.playlists()?.iter().map(|p| p.clone().name).collect();
        Ok(list)
    }

    pub fn get_append_list(conn: &mut Client) -> AppResult<ContentList<String>> {
        let mut list = ContentList::new();
        list.list.push("New Playlist".to_string());
        list.list.push("Current Playlist".to_string());
        for item in Self::get_playlist(conn)? {
            list.list.push(item.to_string());
        }

        Ok(list)
    }

    /// Handles the <Space> event key
    pub fn handle_add_or_remove_from_current_playlist(&mut self) -> AppResult<()> {
        match self.selected_tab {
            SelectedTab::DirectoryBrowser => {
                let (content_type, content) =
                    self.browser.filetree.get(self.browser.selected).unwrap();
                if content_type == "directory" {
                    let file = format!("{}/{}", self.browser.path, content);
                    let songs = self.conn.conn.listfiles(&file).unwrap_or_default();
                    for (t, f) in songs.iter() {
                        if t == "file" {
                            if Path::new(&f).has_extension(&[
                                "mp3", "ogg", "flac", "m4a", "wav", "aac", "opus", "ape", "wma",
                                "mpc", "aiff", "dff", "mp2", "mka",
                            ]) {
                                let path = file.clone() + "/" + f;
                                let full_path = path.strip_prefix("./").unwrap_or_else(|| "");
                                let song = self.conn.get_song_with_only_filename(&full_path);
                                self.conn.conn.push(&song)?;
                            }
                        }
                    }
                } else if content_type == "file" {
                    let mut status = false;
                    for (i, song) in self.queue_list.list.clone().iter().enumerate() {
                        let song_path = song.file.split("/").last().unwrap_or_default();
                        if song_path.eq(content) {
                            self.conn.conn.delete(i as u32).unwrap();
                            status = true;
                        }
                    }

                    if !status {
                        let mut filename = format!("{}/{}", self.browser.path, content);

                        // Remove "./" from the beginning of filename
                        filename.remove(0);
                        filename.remove(0);

                        let song = self.conn.get_song_with_only_filename(&filename);
                        self.conn.conn.push(&song)?;

                        // updating queue, to avoid multiple pushes of the same songs if we enter multiple times before the queue gets updated
                        self.update_queue();
                    }
                }

                // Highlight next row if possible
                if self.browser.selected != self.browser.filetree.len() - 1 {
                    self.browser.selected += 1;
                }
            }

            SelectedTab::Queue => {
                if self.queue_list.list.is_empty() {
                    return Ok(());
                }
                let file = self
                    .queue_list
                    .list
                    .get(self.queue_list.index)
                    .unwrap()
                    .file
                    .to_string();

                for (i, song) in self.queue_list.list.clone().iter().enumerate() {
                    if song.file.eq(&file) {
                        self.conn.conn.delete(i as u32).unwrap();
                        if self.queue_list.index == self.queue_list.list.len() - 1
                            && self.queue_list.index != 0
                        {
                            self.queue_list.index -= 1;
                        }
                    }
                }
            }

            _ => {}
        }

        self.update_queue();
        self.conn.update_status();
        Ok(())
    }

    /// Cycle through tabs
    pub fn cycle_tabls(&mut self) {
        self.selected_tab = match self.selected_tab {
            SelectedTab::Queue => SelectedTab::DirectoryBrowser,
            SelectedTab::DirectoryBrowser => SelectedTab::Playlists,
            SelectedTab::Playlists => SelectedTab::DirectoryBrowser,
        };
    }

    /// handles the Enter event on the directory browser
    pub fn handle_enter(&mut self) -> AppResult<()> {
        let browser = &mut self.browser;
        let (t, path) = browser.filetree.get(browser.selected).unwrap();
        if t == "directory" {
            if path != "." {
                browser.prev_path = browser.path.clone();
                browser.path = browser.prev_path.clone() + "/" + path;
                browser.update_directory(&mut self.conn)?;
                browser.prev_selected = browser.selected;
                browser.selected = 0;
            }
        } else {
            let index = self.queue_list.list.iter().position(|x| {
                let file = x.file.split("/").last().unwrap_or_default();
                file.eq(path)
            });

            if index.is_some() {
                self.conn.conn.switch(index.unwrap() as u32)?;
            } else {
                let mut filename = format!("{}/{}", browser.path, path);

                // Remove "./" from the beginning of filename
                filename.remove(0);
                filename.remove(0);

                let song = self.conn.get_song_with_only_filename(&filename);
                self.conn.push(&song)?;

                // updating queue, to avoid multiple pushes of the same songs if we enter multiple times before the queue gets updated
                self.update_queue();
            }
        }
        Ok(())
    }

    // Cursor movements
    pub fn move_cursor_left(&mut self) {
        match self.inputmode {
            InputMode::PlaylistRename => {
                let cursor_moved_left = self.pl_cursor_pos.saturating_sub(1);
                self.pl_cursor_pos = self.clamp_cursor(cursor_moved_left);
            }
            InputMode::Editing => {
                let cursor_moved_left = self.search_cursor_pos.saturating_sub(1);
                self.search_cursor_pos = self.clamp_cursor(cursor_moved_left);
            }
            InputMode::NewPlaylist => {
                let cursor_moved_left = self.pl_new_pl_cursor_pos.saturating_sub(1);
                self.pl_new_pl_cursor_pos = self.clamp_cursor(cursor_moved_left);
            }
            _ => {}
        }
    }

    pub fn move_cursor_right(&mut self) {
        match self.inputmode {
            InputMode::PlaylistRename => {
                let cursor_moved_right = self.pl_cursor_pos.saturating_add(1);
                self.pl_cursor_pos = self.clamp_cursor(cursor_moved_right);
            }
            InputMode::Editing => {
                let cursor_moved_right = self.search_cursor_pos.saturating_add(1);
                self.search_cursor_pos = self.clamp_cursor(cursor_moved_right);
            }

            InputMode::NewPlaylist => {
                let cursor_moved_right = self.pl_new_pl_cursor_pos.saturating_add(1);
                self.pl_new_pl_cursor_pos = self.clamp_cursor(cursor_moved_right);
            }

            _ => {}
        }
    }

    pub fn enter_char(&mut self, new_char: char) {
        match self.inputmode {
            InputMode::PlaylistRename => {
                self.pl_newname_input.insert(self.pl_cursor_pos, new_char);
            }
            InputMode::NewPlaylist => {
                self.pl_new_pl_input
                    .insert(self.pl_new_pl_cursor_pos, new_char);
            }
            InputMode::Editing => {
                self.search_input.insert(self.search_cursor_pos, new_char);
            }
            _ => {}
        }

        self.move_cursor_right();
    }

    pub fn delete_char(&mut self) {
        let is_not_cursor_leftmost = match self.inputmode {
            InputMode::PlaylistRename => self.pl_cursor_pos != 0,
            InputMode::NewPlaylist => self.pl_new_pl_cursor_pos != 0,
            InputMode::Editing => self.search_cursor_pos != 0,
            _ => false,
        };

        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = match self.inputmode {
                InputMode::Editing => self.search_cursor_pos,
                InputMode::PlaylistRename => self.pl_cursor_pos,
                InputMode::NewPlaylist => self.pl_new_pl_cursor_pos,
                _ => 0,
            };

            let from_left_to_current_index = current_index - 1;

            if self.inputmode == InputMode::PlaylistRename {
                let before_char_to_delete = self
                    .pl_newname_input
                    .chars()
                    .take(from_left_to_current_index);
                let after_char_to_delete = self.pl_newname_input.chars().skip(current_index);

                self.pl_newname_input = before_char_to_delete.chain(after_char_to_delete).collect();
                self.move_cursor_left();
            } else if self.inputmode == InputMode::NewPlaylist {
                let before_char_to_delete = self
                    .pl_new_pl_input
                    .chars()
                    .take(from_left_to_current_index);
                let after_char_to_delete = self.pl_new_pl_input.chars().skip(current_index);

                self.pl_new_pl_input = before_char_to_delete.chain(after_char_to_delete).collect();
                self.move_cursor_left();
            } else if self.inputmode == InputMode::Editing {
                let before_char_to_delete =
                    self.search_input.chars().take(from_left_to_current_index);
                let after_char_to_delete = self.search_input.chars().skip(current_index);

                self.search_input = before_char_to_delete.chain(after_char_to_delete).collect();
                self.move_cursor_left();
            }
        }
    }

    pub fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        match self.inputmode {
            InputMode::PlaylistRename => new_cursor_pos.clamp(0, self.pl_newname_input.len()),
            InputMode::NewPlaylist => new_cursor_pos.clamp(0, self.pl_new_pl_input.len()),
            InputMode::Editing => new_cursor_pos.clamp(0, self.search_input.len()),
            _ => 0,
        }
    }

    pub fn reset_cursor(&mut self) {
        match self.inputmode {
            InputMode::Editing => {
                self.search_cursor_pos = 0;
            }
            InputMode::PlaylistRename => {
                self.pl_cursor_pos = 0;
            }
            InputMode::NewPlaylist => {
                self.pl_new_pl_cursor_pos = 0;
            }
            _ => {}
        }
    }

    /// Given time in seconds, convert it to hh:mm:ss
    pub fn format_time(time: Duration) -> String {
        let time = time.as_secs();
        let h = time / 3600;
        let m = (time % 3600) / 60;
        let s = (time % 3600) % 60;
        if h == 0 {
            format!("{:02}:{:02}", m, s)
        } else {
            format!("{:02}:{:02}:{:02}", h, m, s)
        }
    }

    // Mouse event handlers
    pub fn handle_scroll_up(&mut self) {
        match self.selected_tab {
            SelectedTab::Queue => {
                self.queue_list.prev();
            }
            SelectedTab::DirectoryBrowser => {
                self.browser.prev();
            }
            SelectedTab::Playlists => {
                self.pl_list.prev();
            }
        }
    }

    pub fn handle_scroll_down(&mut self) {
        match self.selected_tab {
            SelectedTab::Queue => {
                self.queue_list.next();
            }
            SelectedTab::DirectoryBrowser => {
                self.browser.next();
            }
            SelectedTab::Playlists => {
                self.pl_list.next();
            }
        }
    }

    pub fn handle_mouse_left_click(&mut self, _x: u16, _y: u16) -> AppResult<()> {
        Ok(())
    }
}
