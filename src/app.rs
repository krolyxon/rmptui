use crate::browser::FileBrowser;
use crate::connection::Connection;
use crate::list::ContentList;
use crate::ui::InputMode;
use mpd::Client;

// Application result type
pub type AppResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Application
#[derive(Debug)]
pub struct App {
    /// check if app is running
    pub running: bool,
    pub conn: Connection,
    pub queue_list: ContentList<String>,
    pub pl_list: ContentList<String>,
    pub selected_tab: SelectedTab,
    pub browser: FileBrowser,

    // Search
    pub inputmode: InputMode,
    pub search_input: String,
    pub cursor_position: usize,
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

        Self::get_queue(&mut conn, &mut queue_list.list);

        let browser = FileBrowser::new();
        Ok(Self {
            running: true,
            conn,
            queue_list,
            pl_list,
            selected_tab: SelectedTab::DirectoryBrowser,
            browser,
            inputmode: InputMode::Normal,
            search_input: String::new(),
            cursor_position: 0,
        })
    }

    pub fn tick(&mut self) {
        self.conn.update_status();
        self.update_queue();
        // self.browser.update_directory(&mut self.conn).unwrap();
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn get_queue(conn: &mut Connection, vec: &mut Vec<String>) {
        // conn.conn.queue().unwrap().into_iter().for_each(|x| {
        //     if let Some(title) = x.title {
        //         if let Some(artist) = x.artist {
        //             vec.push(format!("{} - {}", artist, title));
        //         } else {
        //             vec.push(title)
        //         }
        //     } else {
        //         vec.push(x.file)
        //     }
        // });
        conn.conn.queue().unwrap().into_iter().for_each(|x| {
            vec.push(x.file);
        });
    }

    pub fn update_queue(&mut self) {
        self.queue_list.list.clear();
        Self::get_queue(&mut self.conn, &mut self.queue_list.list);
    }

    pub fn get_playlist(conn: &mut Client) -> AppResult<Vec<String>> {
        let list: Vec<String> = conn.playlists()?.iter().map(|p| p.clone().name).collect();
        Ok(list)
    }

    pub fn update_playlist(&mut self) -> AppResult<()> {
        Self::get_playlist(&mut self.conn.conn)?;
        Ok(())
    }

    pub fn cycle_tabls(&mut self) {
        self.selected_tab = match self.selected_tab {
            SelectedTab::DirectoryBrowser => SelectedTab::Queue,
            SelectedTab::Queue => SelectedTab::Playlists,
            SelectedTab::Playlists => SelectedTab::DirectoryBrowser,
        };
    }

    pub fn search_song(&mut self) -> AppResult<()> {
        let list = self
            .conn
            .songs_filenames
            .iter()
            .map(|f| f.as_str())
            .collect::<Vec<&str>>();
        let (filename, _) =
            rust_fuzzy_search::fuzzy_search_sorted(self.search_input.as_str(), &list)
                .get(0)
                .unwrap()
                .clone();

        let song = self.conn.get_song_with_only_filename(filename);
        self.conn.push(&song)?;

        Ok(())
    }

    // Cursor movements
    pub fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.cursor_position.saturating_sub(1);
        self.cursor_position = self.clamp_cursor(cursor_moved_left);
    }

    pub fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.cursor_position.saturating_add(1);
        self.cursor_position = self.clamp_cursor(cursor_moved_right);
    }

    pub fn enter_char(&mut self, new_char: char) {
        self.search_input.insert(self.cursor_position, new_char);

        self.move_cursor_right();
    }

    pub fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.cursor_position != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.cursor_position;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.search_input.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.search_input.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.search_input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    pub fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.search_input.len())
    }

    pub fn reset_cursor(&mut self) {
        self.cursor_position = 0;
    }
}
