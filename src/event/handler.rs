use crate::{
    app::{App, AppResult, SelectedTab},
    connection::VolumeStatus,
    ui::InputMode,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use std::time::Duration;

use super::{pl_append_keys, pl_rename_keys, new_pl_keys, search_keys};

pub fn handle_key_events(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    // searching, playlist renaming, playlist appending
    if app.inputmode == InputMode::Editing {
        search_keys::handle_search_keys(key_event, app)?;
    } else if app.inputmode == InputMode::PlaylistRename {
        pl_rename_keys::handle_pl_rename_keys(key_event, app)?;
    } else if app.inputmode == InputMode::NewPlaylist {
        new_pl_keys::handle_new_pl_keys(key_event, app)?;
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

            // Playback controls
            // Toggle Pause
            KeyCode::Char('p') => {
                app.conn.toggle_pause();
                app.conn.update_status();
            }

            // Pause
            KeyCode::Char('s') => {
                app.conn.pause();
                app.conn.update_status();
            }

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
            KeyCode::Char('D') => {
                app.conn.play_dmenu()?;
                app.conn.update_status();
            }

            // add to queue
            KeyCode::Char('a') => app.playlist_popup = true,

            // Fast forward
            KeyCode::Char('f') => {
                if !app.queue_list.list.is_empty() {
                    let status = app.conn.conn.status().unwrap_or_default();
                    let place = status.song.unwrap_or_default().pos;
                    let (pos, _) = status.time.unwrap_or_default();
                    let pos = Duration::from_secs(pos.as_secs().wrapping_add(2));
                    app.conn.conn.seek(place, pos)?;
                    app.conn.update_status();
                }
            }

            // backward
            KeyCode::Char('b') => {
                if !app.queue_list.list.is_empty() {
                    let status = app.conn.conn.status().unwrap_or_default();
                    let place = status.song.unwrap_or_default().pos;
                    let (pos, _) = status.time.unwrap_or_default();
                    let pos = Duration::from_secs(pos.as_secs().wrapping_sub(2));
                    app.conn.conn.seek(place, pos)?;
                    app.conn.update_status();
                }
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
                    app.conn.update_status();
                }
            }

            // Play previous song
            KeyCode::Char('<') => {
                if !app.queue_list.list.is_empty() {
                    app.conn.conn.prev()?;
                    app.update_queue();
                    app.conn.update_status();
                }
            }

            // Volume controls
            KeyCode::Char('=') | KeyCode::Char('+') => {
                app.conn.inc_volume(2);
                app.conn.update_status();
            }

            KeyCode::Char('-') => {
                app.conn.dec_volume(2);
                app.conn.update_status();
            }

            // Toggle Mute
            KeyCode::Char('m') => {
                match app.conn.volume_status {
                    VolumeStatus::Muted(v) => {
                        app.conn.conn.volume(v)?;
                        app.conn.volume_status = VolumeStatus::Unmuted;
                    }
                    VolumeStatus::Unmuted => {
                        let current_volume = app.conn.status.volume;
                        app.conn.conn.volume(0)?;
                        app.conn.volume_status = VolumeStatus::Muted(current_volume);
                    }
                }
                app.conn.update_status();
            }

            // Update MPD database
            KeyCode::Char('U') => {
                app.conn.conn.rescan()?;
                app.should_update_song_list = true;
            }

            // Search for songs
            KeyCode::Char('/') => {
                if app.inputmode == InputMode::Normal {
                    app.inputmode = InputMode::Editing;
                } else {
                    app.inputmode = InputMode::Normal;
                }
            }

            // Add or Remove from Current Playlist
            KeyCode::Char(' ') => {
                app.handle_add_or_remove_from_current_playlist()?;
            }
            _ => {}
        }

        // Tab specific keymaps
        match app.selected_tab {
            SelectedTab::Queue => {
                match key_event.code {
                    // Go Up
                    KeyCode::Char('j') | KeyCode::Down => app.queue_list.next(),

                    // Go down
                    KeyCode::Char('k') | KeyCode::Up => app.queue_list.prev(),

                    // Next directory
                    KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                        app.conn.conn.switch(app.queue_list.index as u32)?;
                        app.conn.update_status();
                    }

                    // Delete highlighted song from the queue
                    KeyCode::Char('d') => {
                        if app.queue_list.index >= app.queue_list.list.len()
                            && app.queue_list.index != 0
                        {
                            app.queue_list.index -= 1;
                        }

                        app.conn.conn.delete(app.queue_list.index as u32)?;

                        if app.queue_list.index >= app.queue_list.list.len().saturating_sub(1)
                            && app.queue_list.index != 0
                        {
                            app.queue_list.index -= 1;
                        }

                        app.conn.update_status();
                        app.update_queue();
                    }

                    // Swap highlighted song with next one
                    KeyCode::Char('J') => {
                        let current: u32 = app.queue_list.index as u32;
                        let next: u32 = if (current + 1) as usize == app.queue_list.list.len() {
                            app.queue_list.index as u32
                        } else {
                            app.queue_list.index += 1;
                            current + 1
                        };
                        app.conn.conn.swap(current, next)?;
                        app.update_queue();
                        app.conn.update_status();
                    }

                    // Swap highlighted song with previous one
                    KeyCode::Char('K') => {
                        let current: u32 = app.queue_list.index as u32;
                        let prev: u32 = if current == 0 {
                            app.queue_list.index as u32
                        } else {
                            app.queue_list.index -= 1;
                            current - 1
                        };
                        app.conn.conn.swap(current, prev)?;
                        app.update_queue();
                        app.conn.update_status();
                    }

                    // go to top of list
                    KeyCode::Char('g') => app.queue_list.index = 0,

                    // go to bottom of list
                    KeyCode::Char('G') => app.queue_list.index = app.queue_list.list.len() - 1,

                    _ => {}
                }
            }

            SelectedTab::DirectoryBrowser => {
                match key_event.code {
                    // Go Up
                    KeyCode::Char('j') | KeyCode::Down => app.browser.next(),

                    // Go down
                    KeyCode::Char('k') | KeyCode::Up => app.browser.prev(),

                    // Next directory
                    KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                        // app.update_queue();
                        app.handle_enter()?;
                        app.conn.update_status();
                    }

                    // head back to previous directory
                    KeyCode::Char('h') | KeyCode::Left => {
                        app.browser.handle_go_back(&mut app.conn)?
                    }

                    // go to top of list
                    KeyCode::Char('g') => app.browser.selected = 0,

                    // go to bottom of list
                    KeyCode::Char('G') => app.browser.selected = app.browser.filetree.len() - 1,

                    _ => {}
                }
            }

            SelectedTab::Playlists => {
                match key_event.code {
                    // Go Up
                    KeyCode::Char('j') | KeyCode::Down => app.pl_list.next(),

                    // Go down
                    KeyCode::Char('k') | KeyCode::Up => app.pl_list.prev(),

                    // go to top of list
                    KeyCode::Char('g') => app.pl_list.index = 0,

                    // go to bottom of list
                    KeyCode::Char('G') => app.pl_list.index = app.pl_list.list.len() - 1,

                    // Playlist Rename
                    KeyCode::Char('R') => {
                        app.inputmode = InputMode::PlaylistRename;
                    }

                    // add to current playlist
                    KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right | KeyCode::Char(' ') => {
                        // app.update_queue();
                        if !app.pl_list.list.is_empty() {
                            app.conn
                                .load_playlist(app.pl_list.list.get(app.pl_list.index).unwrap())?;
                            app.conn.update_status();
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

pub fn handle_mouse_events(mouse_event: MouseEvent, app: &mut App) -> AppResult<()> {
    match mouse_event.kind {
        MouseEventKind::ScrollUp => app.handle_scroll_up(),
        MouseEventKind::ScrollDown => app.handle_scroll_down(),
        MouseEventKind::Down(button) => {
            let (x, y) = (mouse_event.column, mouse_event.row);
            match button {
                crossterm::event::MouseButton::Left => app.handle_mouse_left_click(x, y)?,
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}
