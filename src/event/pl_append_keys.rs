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
            // name of highlighted playlist in append list
            let pl_name = &app.append_list.get_item_at_current_index();

            match app.selected_tab {
                SelectedTab::Queue => {
                    let short_path = &app.queue_list.get_item_at_current_index().file;

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

                SelectedTab::Playlists => {
                    let playlist_name = app.pl_list.get_item_at_current_index();
                    if *pl_name == "Current Playlist" {
                        app.conn.load_playlist(playlist_name)?;
                        app.update_queue();
                    } else {
                        let songs = app.conn.conn.playlist(playlist_name)?;
                        for song in songs {
                            // We ignore the Err() since there could be songs in playlists, which do not exist in the db anymore.
                            // So instead of panicking, we just ignore if the song does not exists
                            app.conn
                                .add_to_playlist(*pl_name, &song)
                                .unwrap_or_else(|_| {});
                        }
                    }
                }
            }

            // hide the playlist popup
            app.playlist_popup = false;
            app.append_list.index = 0;
        }
        _ => {}
    }

    Ok(())
}
