use std::time::Duration;

use crate::browser::FileBrowser;
use crate::connection::Connection;
use crate::list::ContentList;
use crate::ui::InputMode;
use mpd::{Client, Song};

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

    // playlist variables
    // used to show playlist popup
    pub playlist_popup: bool,
    pub append_list: ContentList<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum SelectedTab {
    DirectoryBrowser,
    Queue,
    Playlists,
}

impl App {
    pub fn builder(addrs: &str) -> AppResult<Self> {
        let mut conn = Connection::new(addrs).unwrap();
        let mut queue_list = ContentList::new();
        let mut pl_list = ContentList::new();

        pl_list.list = Self::get_playlist(&mut conn.conn)?;
        pl_list.list.sort();

        let append_list = Self::get_append_list(&mut conn.conn)?;
        Self::get_queue(&mut conn, &mut queue_list.list);

        let browser = FileBrowser::new();

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
            append_list,
        })
    }

    pub fn tick(&mut self) {
        self.conn.update_status();
        self.update_queue();
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
                let (_, file) = self.browser.filetree.get(self.browser.selected).unwrap();

                let mut status = false;
                for (i, song) in self.queue_list.list.clone().iter().enumerate() {
                    if song.file.contains(file) {
                        self.conn.conn.delete(i as u32).unwrap();
                        status = true;
                    }
                }

                if !status {
                    if let Some(full_path) = &self.conn.get_full_path(file) {
                        let song = self.conn.get_song_with_only_filename(full_path);
                        self.conn.conn.push(&song)?;
                    }
                }

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
                    if song.file.contains(&file) {
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
            let index = self
                .queue_list
                .list
                .iter()
                .position(|x| x.file.contains(path));

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
            _ => {}
        }
    }

    pub fn enter_char(&mut self, new_char: char) {
        match self.inputmode {
            InputMode::PlaylistRename => {
                self.pl_newname_input.insert(self.pl_cursor_pos, new_char);
                self.move_cursor_right();
            }
            InputMode::Editing => {
                self.search_input.insert(self.search_cursor_pos, new_char);
                self.move_cursor_right();
            }
            _ => {}
        }
    }

    pub fn delete_char(&mut self) {
        let is_not_cursor_leftmost = match self.inputmode {
            InputMode::PlaylistRename => self.pl_cursor_pos != 0,
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
                _ => 0,
            };

            let from_left_to_current_index = current_index - 1;

            if self.inputmode == InputMode::PlaylistRename {
                // Getting all characters before the selected character.
                let before_char_to_delete = self
                    .pl_newname_input
                    .chars()
                    .take(from_left_to_current_index);
                // Getting all characters after selected character.
                let after_char_to_delete = self.pl_newname_input.chars().skip(current_index);
                // Put all characters together except the selected one.
                // By leaving the selected one out, it is forgotten and therefore deleted.
                self.pl_newname_input = before_char_to_delete.chain(after_char_to_delete).collect();
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

    pub fn change_playlist_name(&mut self) -> AppResult<()> {
        if self.selected_tab == SelectedTab::Playlists {
            self.inputmode = InputMode::PlaylistRename;
        }
        Ok(())
    }
}
