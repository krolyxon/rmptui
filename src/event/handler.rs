use crate::{
    app::{App, AppResult, SelectedTab},
    ui::InputMode,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

use super::{pl_append_keys, pl_rename_keys, search_keys};

pub fn handle_key_events(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    // Live search update
    if app.inputmode == InputMode::Editing {
        search_keys::handle_search_keys(key_event, app)?;
    } else if app.inputmode == InputMode::PlaylistRename {
        pl_rename_keys::handle_pl_rename_keys(key_event, app)?;
    } else if app.playlist_popup {
        pl_append_keys::hande_pl_append_keys(key_event, app)?;
    } else {
        // General KeyMaps
        match key_event.code {
            // Quit
            KeyCode::Char('q') => app.quit(),
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
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
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
            KeyCode::Char('h') | KeyCode::Left => match app.selected_tab {
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
                app.selected_tab = SelectedTab::Queue;
            }

            // Playing queue tab
            KeyCode::Char('2') => {
                app.selected_tab = SelectedTab::DirectoryBrowser;
            }

            // Playlists tab
            KeyCode::Char('3') => {
                app.selected_tab = SelectedTab::Playlists;
            }

            // Play next song
            KeyCode::Char('>') => {
                if !app.queue_list.list.is_empty() {
                    app.conn.conn.next()?;
                    app.update_queue();
                }
            }

            // Play previous song
            KeyCode::Char('<') => {
                if !app.queue_list.list.is_empty() {
                    app.conn.conn.prev()?;
                    app.update_queue();
                }
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
                if app.queue_list.index >= app.queue_list.list.len() - 1
                    && app.queue_list.index != 0
                {
                    app.queue_list.index -= 1;
                }

                app.conn.conn.delete(app.queue_list.index as u32)?;
                app.update_queue();
            }

            // Swap highlighted song with next one
            KeyCode::Char('J') => {
                let current: u32 = app.queue_list.index as u32;
                let next: u32 = if (current + 1) as usize == app.queue_list.list.len() {
                    app.queue_list.index as u32
                } else {
                    app.queue_list.index += 1;
                    (current + 1) as u32
                };
                app.conn.conn.swap(current, next)?;
                app.update_queue();
            }

            // Swap highlighted song with previous one
            KeyCode::Char('K') => {
                let current: u32 = app.queue_list.index as u32;
                let prev: u32 = if current == 0 {
                    app.queue_list.index as u32
                } else {
                    app.queue_list.index -= 1;
                    (current - 1) as u32
                };
                app.conn.conn.swap(current, prev)?;
                app.update_queue();
            }


            // Update MPD database
            KeyCode::Char('U') => {
                app.conn.conn.rescan()?;
                app.browser.update_directory(&mut app.conn)?;
            }

            // Search for songs
            KeyCode::Char('/') => {
                if app.inputmode == InputMode::Normal {
                    app.inputmode = InputMode::Editing;
                } else {
                    app.inputmode = InputMode::Normal;
                }
            }

            // Remove from Current Playlsit
            KeyCode::Char(' ') | KeyCode::Backspace => {
                app.handle_add_or_remove_from_current_playlist()?;
            }

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

            // Change playlist name
            KeyCode::Char('e') => app.change_playlist_name()?,
            _ => {}
        }
    }
    Ok(())
}
