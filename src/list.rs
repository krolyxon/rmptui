#[derive(Debug)]
pub struct ContentList<T> {
    pub list: Vec<T>,
    pub index: usize,
}

impl<T> ContentList<T> {
    pub fn new() -> Self {
        ContentList {
            list: Vec::new(),
            index: 0,
        }
    }

    // Go to next item in list
    pub fn next(&mut self) {
        let len = self.list.len();
        if len != 0 && self.index < len - 1 {
            self.index += 1;
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

impl<T> Default for ContentList<T> {
    fn default() -> Self {
        Self::new()
    }
}
