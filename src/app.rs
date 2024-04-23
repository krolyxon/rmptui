use crate::connection::Connection;
use crate::list::ContentList;
use mpd::Client;
use std::collections::VecDeque;

// Application result type
pub type AppResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Application
#[derive(Debug)]
pub struct App {
    /// check if app is running
    pub running: bool,
    pub conn: Connection,
    pub play_deque: VecDeque<String>,
    pub song_list: ContentList<String>,
    pub queue_list: ContentList<String>,
    pub pl_list: ContentList<String>,
}

impl App {
    pub fn builder(addrs: &str) -> AppResult<Self> {
        let mut conn = Connection::new(addrs).unwrap();
        let mut vec: VecDeque<String> = VecDeque::new();
        let mut pl_list = ContentList::new();
        pl_list.list = Self::get_playlist(&mut conn.conn)?;
        Self::get_queue(&mut conn, &mut vec);

        let mut song_list = ContentList::new();
        song_list.list = conn.songs_filenames.clone();

        Ok(Self {
            running: true,
            conn,
            play_deque: vec,
            song_list,
            queue_list: ContentList::new(),
            pl_list,
        })
    }

    pub fn tick(&self) {}

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn get_queue(conn: &mut Connection, vec: &mut VecDeque<String>) {
        conn.conn.queue().unwrap().into_iter().for_each(|x| {
            if let Some(title) = x.title {
                if let Some(artist) = x.artist {
                    vec.push_back(format!("{} - {}", artist, title));
                } else {
                    vec.push_back(title)
                }
            } else {
                vec.push_back(x.file)
            }
        });
    }

    pub fn update_queue(&mut self) {
        self.play_deque.clear();
        Self::get_queue(&mut self.conn, &mut self.play_deque);
    }

    pub fn get_playlist(conn: &mut Client) -> AppResult<Vec<String>> {
        let list: Vec<String> = conn.playlists()?.iter().map(|p| p.clone().name).collect();

        Ok(list)
    }

    pub fn update_playlist(&mut self) -> AppResult<()> {
        Self::get_playlist(&mut self.conn.conn)?;
        Ok(())
    }
}
