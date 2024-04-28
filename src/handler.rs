use std::time::Duration;

use crate::{
    app::{App, AppResult, SelectedTab},
    ui::InputMode,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rust_fuzzy_search::{self, fuzzy_search_sorted};

pub fn handle_key_events(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    if app.inputmode == InputMode::Editing {
        // Live search update
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
                    if item.contains(res.get(0).unwrap()) {
                        app.browser.selected = i;
                    }
                }
            }

            SelectedTab::Queue => {
                let list: Vec<&str> = app
                    .queue_list
                    .list
                    .iter()
                    .map(|f| f.as_str())
                    .collect::<Vec<&str>>();
                let res: Vec<(&str, f32)> = fuzzy_search_sorted(&app.search_input, &list);
                let res = res.iter().map(|(x, _)| *x).collect::<Vec<&str>>();

                for (i, item) in app.queue_list.list.iter().enumerate() {
                    if item.contains(res.get(0).unwrap()) {
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
                    if item.contains(res.get(0).unwrap()) {
                        app.pl_list.index = i;
                    }
                }
            }
        }

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
                let (res, _) = res.get(0).unwrap();

                for (i, (_, item)) in app.browser.filetree.iter().enumerate() {
                    if item.contains(res) {
                        app.browser.selected = i;
                    }
                }

                app.search_input.clear();
                app.inputmode = InputMode::Normal;
                app.reset_cursor();
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
    } else if app.playlist_popup {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                app.playlist_popup = false;
            }

            KeyCode::Char('j') | KeyCode::Down => app.append_list.next(),
            KeyCode::Char('k') | KeyCode::Up => app.append_list.prev(),

            KeyCode::Enter => {
                let pl_index = app.append_list.index;
                let pl_name = app.append_list.list.get(pl_index).unwrap();

                let s_index: usize;
                let mut short_path: String = String::new();
                match app.selected_tab {
                    SelectedTab::Queue => {
                        s_index = app.queue_list.index;
                        short_path = app.queue_list.list.get(s_index).unwrap().to_string();
                    }

                    SelectedTab::DirectoryBrowser => {
                        let (t, f) = app.browser.filetree.get(app.browser.selected).unwrap();
                        if t == "file" {
                            short_path = f.to_string();
                        }
                    }
                    _ => {}
                }

                let full_path = app.conn.get_full_path(&short_path)?;
                let song = app.conn.get_song_with_only_filename(&full_path);

                if pl_name == "Current Playlist" {
                    app.conn.conn.push(&song)?;
                    app.update_queue();
                } else {
                    app.conn.add_to_playlist(pl_name, &song)?;
                }

                // hide the playlist popup
                app.playlist_popup = false;
                app.append_list.index = 0;
            }
            _ => {}
        }
    } else {
        match key_event.code {
            // Quit
            KeyCode::Char('q') | KeyCode::Esc => app.quit(),
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if key_event.modifiers == KeyModifiers::CONTROL {
                    app.quit();
                } else {
                    app.conn.conn.clear()?;
                    app.conn.update_status();
                    app.queue_list.list.clear();
                    app.queue_list.reset_index();
                }
            }

            // Go Up
            KeyCode::Char('j') | KeyCode::Down => match app.selected_tab {
                SelectedTab::DirectoryBrowser => app.browser.next(),
                SelectedTab::Queue => app.queue_list.next(),
                SelectedTab::Playlists => app.pl_list.next(),
            },

            // Go down
            KeyCode::Char('k') | KeyCode::Up => match app.selected_tab {
                SelectedTab::DirectoryBrowser => app.browser.prev(),
                SelectedTab::Queue => app.queue_list.prev(),
                SelectedTab::Playlists => app.pl_list.prev(),
            },

            // Next directory
            KeyCode::Enter | KeyCode::Char('l') => {
                // app.update_queue();

                match app.selected_tab {
                    SelectedTab::DirectoryBrowser => {
                        app.handle_enter()?;
                    }

                    SelectedTab::Queue => {
                        app.conn.conn.switch(app.queue_list.index as u32)?;
                    }

                    SelectedTab::Playlists => {
                        app.conn
                            .load_playlist(app.pl_list.list.get(app.pl_list.index).unwrap())?;
                    }
                }
                app.conn.update_status();
            }

            // head back to previous directory
            KeyCode::Char('h') => match app.selected_tab {
                SelectedTab::DirectoryBrowser => {
                    app.browser.handle_go_back(&mut app.conn)?;
                }
                SelectedTab::Queue => {}
                SelectedTab::Playlists => {}
            },

            // Playback controls
            // Toggle Pause
            KeyCode::Char('p') => app.conn.toggle_pause(),

            // Pause
            KeyCode::Char('s') => app.conn.pause(),

            // Toggle rpeat
            KeyCode::Char('r') => {
                app.conn.toggle_repeat();
                app.conn.update_status();
            }

            // Toggle random
            KeyCode::Char('z') => {
                app.conn.toggle_random();
                app.conn.update_status();
            }

            // Dmenu prompt
            KeyCode::Char('D') => app.conn.play_dmenu()?,

            // add to queue
            KeyCode::Char('a') => app.playlist_popup = true,

            KeyCode::Right => {
                app.conn
                    .load_playlist(app.pl_list.list.get(app.pl_list.index).unwrap())?;
            }

            // Fast forward
            KeyCode::Char('f') => {
                let place = app.conn.conn.status().unwrap().song.unwrap().pos;
                let (pos, _) = app.conn.conn.status().unwrap().time.unwrap();
                let pos = Duration::from_secs(pos.as_secs().wrapping_add(2));
                app.conn.conn.seek(place, pos)?;
            }

            // backward
            KeyCode::Char('b') => {
                let place = app.conn.conn.status().unwrap().song.unwrap().pos;
                let (pos, _) = app.conn.conn.status().unwrap().time.unwrap();
                let pos = Duration::from_secs(pos.as_secs().wrapping_add(2));
                app.conn.conn.seek(place, pos)?;
            }

            // Cycle through tabs
            KeyCode::Tab => {
                app.cycle_tabls();
            }

            // Directory browser tab
            KeyCode::Char('1') => {
                app.selected_tab = SelectedTab::DirectoryBrowser;
            }

            // Playing queue tab
            KeyCode::Char('2') => {
                app.selected_tab = SelectedTab::Queue;
            }

            // Playlists tab
            KeyCode::Char('3') => {
                app.selected_tab = SelectedTab::Playlists;
            }

            // Play next song
            KeyCode::Char('>') => {
                app.conn.conn.next()?;
            }

            // Play previous song
            KeyCode::Char('<') => {
                app.conn.conn.prev()?;
            }

            // Volume controls
            KeyCode::Char('=') => {
                app.conn.inc_volume(2);
                app.conn.update_status();
            }

            KeyCode::Char('-') => {
                app.conn.dec_volume(2);
                app.conn.update_status();
            }

            // Delete highlighted song from the queue
            KeyCode::Char('d') => {
                if app.queue_list.index >= app.queue_list.list.len() - 1 {
                    if app.queue_list.index != 0 {
                        app.queue_list.index -= 1;
                    }
                }

                app.conn.conn.delete(app.queue_list.index as u32)?;
                app.update_queue();
            }

            // Update MPD database
            KeyCode::Char('U') => {
                app.conn.conn.update()?;
            }

            // Search for songs
            KeyCode::Char('/') => {
                app.inputmode = InputMode::toggle_editing_states(&app.inputmode);
            }

            // Remove from Current Playlsit
            KeyCode::Char(' ') | KeyCode::Backspace => {
                app.handle_remove_or_from_current_playlist()?;
            }

            // Change playlist name
            KeyCode::Char('e') => if app.selected_tab == SelectedTab::Playlists {},

            // go to top of list
            KeyCode::Char('g') => match app.selected_tab {
                SelectedTab::Queue => app.queue_list.index = 0,
                SelectedTab::DirectoryBrowser => app.browser.selected = 0,
                SelectedTab::Playlists => app.pl_list.index = 0,
            },

            // go to bottom of list
            KeyCode::Char('G') => match app.selected_tab {
                SelectedTab::Queue => app.queue_list.index = app.queue_list.list.len() - 1,
                SelectedTab::DirectoryBrowser => {
                    app.browser.selected = app.browser.filetree.len() - 1
                }
                SelectedTab::Playlists => app.pl_list.index = app.pl_list.list.len() - 1,
            },
            _ => {}
        }
    }
    Ok(())
}
