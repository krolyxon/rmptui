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
    pub song_list: ContentList<String>,
    pub queue_list: ContentList<String>,
    pub pl_list: ContentList<String>,
    pub selected_tab: SelectedTab,
}


#[derive(Debug, PartialEq, Clone)]
pub enum SelectedTab {
    SongList,
    Queue,
    Playlists,
}

impl SelectedTab {
    fn as_usize(&self) {
        match self {
            SelectedTab::SongList => 0,
            SelectedTab::Queue => 1,
            SelectedTab::Playlists => 2,
        };
    }
}

impl App {
    pub fn builder(addrs: &str) -> AppResult<Self> {
        let mut conn = Connection::new(addrs).unwrap();
        let mut queue_list = ContentList::new();
        let mut pl_list = ContentList::new();
        pl_list.list = Self::get_playlist(&mut conn.conn)?;

        Self::get_queue(&mut conn, &mut queue_list.list);

        let mut song_list = ContentList::new();
        song_list.list = conn.songs_filenames.clone();

        Ok(Self {
            running: true,
            conn,
            song_list,
            queue_list,
            pl_list,
            selected_tab: SelectedTab::SongList,
        })
    }

    pub fn tick(&self) {}

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn get_queue(conn: &mut Connection, vec: &mut Vec<String>) {
        conn.conn.queue().unwrap().into_iter().for_each(|x| {
            if let Some(title) = x.title {
                if let Some(artist) = x.artist {
                    vec.push(format!("{} - {}", artist, title));
                } else {
                    vec.push(title)
                }
            } else {
                vec.push(x.file)
            }
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
            SelectedTab::SongList => SelectedTab::Queue,
            SelectedTab::Queue => SelectedTab::Playlists,
            SelectedTab::Playlists=> SelectedTab::SongList,
        };
    }
}
