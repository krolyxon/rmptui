use std::time::Duration;

use crate::browser::FileBrowser;
use crate::connection::Connection;
use crate::list::ContentList;
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
}
