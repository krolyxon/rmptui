#[derive(Debug)]
pub struct ContentList<T> {
    pub list: Vec<T>,
    pub index: usize,
}

impl<T> ContentList<T> {
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
            index: 0,
        }
    }

    pub fn next(&mut self) {
        if let Some(last) = self.list.len().checked_sub(1) {
            self.index = self.index.saturating_add(1).min(last);
        }
    }

    pub fn prev(&mut self) {
        self.index = self.index.saturating_sub(1);
    }
}

impl<T> Default for ContentList<T> {
    fn default() -> Self {
        Self::new()
    }
}
