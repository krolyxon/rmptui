use crate::app::{App, AppResult, SelectedTab};
use crate::browser::FileExtension;
use crossterm::event::{KeyCode, KeyEvent};
use std::path::Path;

pub fn hande_pl_append_keys(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    match key_event.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.playlist_popup = false;
        }

        KeyCode::Char('j') | KeyCode::Down => app.append_list.next(),
        KeyCode::Char('k') | KeyCode::Up => app.append_list.prev(),

        KeyCode::Enter => {
            let pl_index = app.append_list.index;
            let pl_name = &app.append_list.list.get(pl_index).unwrap();

            let s_index: usize;
            match app.selected_tab {
                SelectedTab::Queue => {
                    s_index = app.queue_list.index;
                    let short_path = &app.queue_list.list.get(s_index).unwrap().file;

                    if let Some(full_path) = app.conn.get_full_path(short_path) {
                        let song = app.conn.get_song_with_only_filename(&full_path);

                        if *pl_name == "Current Playlist" {
                            app.conn.conn.push(&song)?;
                            app.update_queue();
                        } else {
                            app.conn.add_to_playlist(pl_name, &song)?;
                        }
                    }
                }

                SelectedTab::DirectoryBrowser => {
                    let (t, f) = app.browser.filetree.get(app.browser.selected).unwrap();
                    if t == "file" {
                        let short_path = f;
                        if let Some(full_path) = app.conn.get_full_path(short_path) {
                            let song = app.conn.get_song_with_only_filename(&full_path);

                            if *pl_name == "Current Playlist" {
                                app.conn.conn.push(&song)?;
                                app.update_queue();
                            } else {
                                app.conn.add_to_playlist(pl_name, &song)?;
                            }
                        }
                    } else if t == "directory" {
                        for (t, f) in app.conn.conn.listfiles(f)?.iter() {
                            // dir_vec.push((t, f));
                            if t == "file"
                                && Path::new(&f).has_extension(&[
                                    "mp3", "ogg", "flac", "m4a", "wav", "aac", "opus", "ape",
                                    "wma", "mpc", "aiff", "dff", "mp2", "mka",
                                ])
                            {
                                let full_path = app.conn.get_full_path(f).unwrap_or_default();
                                let song = app.conn.get_song_with_only_filename(&full_path);
                                if *pl_name == "Current Playlist" {
                                    app.conn.conn.push(&song)?;
                                } else {
                                    app.conn.add_to_playlist(pl_name, &song)?;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }

            // hide the playlist popup
            app.playlist_popup = false;
            app.append_list.index = 0;
        }
        _ => {}
    }

    Ok(())
}
