use crate::connection::Connection;
use crate::list::ContentList;
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
    pub list: ContentList,
}

impl App {
    pub fn new(addrs: &str) -> Self {
        let mut conn = Connection::new(addrs).unwrap();
        let mut vec: VecDeque<String> = VecDeque::new();
        Self::get_queue(&mut conn, &mut vec);
        Self {
            running: true,
            conn,
            play_deque: vec,
            list: ContentList::new(),
        }
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
}

fn to_vecdeque(filenames: &Vec<String>) -> VecDeque<String> {
    let mut v: VecDeque<String> = VecDeque::new();
    v = filenames.iter().map(|x| x.to_string()).collect();
    v
}
