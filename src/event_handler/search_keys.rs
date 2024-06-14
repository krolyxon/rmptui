use crate::{
    app::{App, AppResult, SelectedTab},
    ui::InputMode,
};
use crossterm::event::{KeyCode, KeyEvent};
use rust_fuzzy_search::{self, fuzzy_search_sorted};

pub fn handle_search_keys(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    match app.selected_tab {
        SelectedTab::DirectoryBrowser => {
            let list: Vec<&str> = app
                .browser
                .filetree
                .iter()
                .map(|(_, f)| f.as_str())
                .collect::<Vec<&str>>();

            let res: Vec<(&str, f32)> = fuzzy_search_sorted(&app.search_input, &list);
            let res = res.iter().map(|(x, _)| *x).collect::<Vec<&str>>();

            for (i, (_, item)) in app.browser.filetree.iter().enumerate() {
                if item.contains(res.first().unwrap()) {
                    app.browser.selected = i;
                }
            }
        }

        SelectedTab::Queue => {
            let list: Vec<&str> = app
                .queue_list
                .list
                .iter()
                .map(|f| f.file.as_str())
                .collect::<Vec<&str>>();
            let res: Vec<(&str, f32)> = fuzzy_search_sorted(&app.search_input, &list);
            let res = res.iter().map(|(x, _)| *x).collect::<Vec<&str>>();

            for (i, item) in app.queue_list.list.iter().enumerate() {
                if item.file.contains(res.first().unwrap()) {
                    app.queue_list.index = i;
                }
            }
        }

        SelectedTab::Playlists => {
            let list: Vec<&str> = app
                .pl_list
                .list
                .iter()
                .map(|f| f.as_str())
                .collect::<Vec<&str>>();
            let res: Vec<(&str, f32)> = fuzzy_search_sorted(&app.search_input, &list);
            let res = res.iter().map(|(x, _)| *x).collect::<Vec<&str>>();

            for (i, item) in app.pl_list.list.iter().enumerate() {
                if item.contains(res.first().unwrap()) {
                    app.pl_list.index = i;
                }
            }
        }
    }

    // Keybind for searching
    //
    // Keybinds for when the search prompt is visible
    match key_event.code {
        KeyCode::Esc => {
            app.inputmode = InputMode::Normal;
        }
        KeyCode::Char(to_insert) => {
            app.enter_char(to_insert);
        }
        KeyCode::Enter => {
            let list: Vec<&str> = app
                .browser
                .filetree
                .iter()
                .map(|(_, f)| f.as_str())
                .collect::<Vec<&str>>();

            let res: Vec<(&str, f32)> = fuzzy_search_sorted(&app.search_input, &list);
            let (res, _) = res.first().unwrap();

            for (i, (_, item)) in app.browser.filetree.iter().enumerate() {
                if item.contains(res) {
                    app.browser.selected = i;
                }
            }

            app.search_input.clear();
            app.reset_cursor();
            app.inputmode = InputMode::Normal;
        }

        KeyCode::Backspace => {
            app.delete_char();
        }

        KeyCode::Left => {
            app.move_cursor_left();
        }

        KeyCode::Right => {
            app.move_cursor_right();
        }

        _ => {}
    }
    Ok(())
}
