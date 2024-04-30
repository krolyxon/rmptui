use mpd::Song;

#[derive(Debug)]
pub struct Queue {
    pub list: Vec<Song>,
    pub index: usize,
}

impl Queue {
    pub fn new() -> Self {
        Queue {
            list: Vec::new(),
            index: 0,
        }
    }

    // Go to next item in list
    pub fn next(&mut self) {
        let len = self.list.len();
        if len != 0 {
            if self.index < len - 1 {
                self.index += 1;
            }
        }
    }

    /// Go to previous item in list
    pub fn prev(&mut self) {
        if self.index != 0 {
            self.index -= 1;
        }
    }

    pub fn reset_index(&mut self) {
        self.index = 0;
    }
}
