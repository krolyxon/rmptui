use crate::{app::AppResult, connection::Connection};

#[derive(Debug)]
pub struct FileBrowser {
    pub filetree: Vec<(String, String)>,
    pub selected: usize,
    pub prev_selected: usize,
    pub path: String,
    pub prev_path: String,
}

impl FileBrowser {
    pub fn new() -> FileBrowser {
        FileBrowser {
            filetree: Vec::new(),
            selected: 0,
            prev_selected: 0,
            path: ".".to_string(),
            prev_path: ".".to_string(),
        }
    }

    pub fn update_directory(&mut self, conn: &mut Connection) -> AppResult<()> {
        self.filetree.clear();
        self.filetree = conn
            .conn
            .listfiles(self.path.as_str())?
            .into_iter()
            .filter(|(f, _)| f == "directory" || f == "file")
            .collect::<Vec<(String, String)>>();

        Ok(())
    }

    // Go to next item in filetree
    pub fn next(&mut self) {
        // if self.selected < self.filetree.len() - 1 {
        //     self.selected += 1;
        // }

        if self.selected == self.filetree.len() - 1 {
            self.selected = 0;
        } else {
            self.selected += 1;
        }
    }

    /// Go to previous item in filetree
    pub fn prev(&mut self) {
        if self.selected == 0 {
            self.selected = self.filetree.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    pub fn handle_go_back(&mut self, conn: &mut Connection) -> AppResult<()> {
        if self.prev_path != "." {
            let r = self.path.rfind("/").unwrap();
            self.path = self.path.as_str()[..r].to_string();
            self.update_directory(conn)?;
        } else {
            self.path = self.prev_path.clone();
            self.update_directory(conn)?;
        }

        self.selected = self.prev_selected;
        Ok(())
    }
}
