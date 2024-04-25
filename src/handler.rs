use std::time::Duration;

use crate::app::{App, AppResult, SelectedTab};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mpd::{Query, Term};
use ratatui::style::Modifier;
use rust_fuzzy_search;
use simple_dmenu::dmenu;

pub fn handle_key_events(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    match key_event.code {
        KeyCode::Char('q') | KeyCode::Esc => app.quit(),
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if key_event.modifiers == KeyModifiers::CONTROL {
                app.quit();
            }
        }

        KeyCode::Char('j') | KeyCode::Down => match app.selected_tab {
            SelectedTab::SongList => app.song_list.next(),
            SelectedTab::Queue => app.queue_list.next(),
            SelectedTab::Playlists => app.pl_list.next(),
        },

        KeyCode::Char('k') | KeyCode::Up => match app.selected_tab {
            SelectedTab::SongList => app.song_list.prev(),
            SelectedTab::Queue => app.queue_list.prev(),
            SelectedTab::Playlists => app.pl_list.prev(),
        },

        KeyCode::Enter | KeyCode::Char('l') => {
            // app.update_queue();

            match app.selected_tab {
                SelectedTab::SongList => {
                    let song = app.conn.get_song_with_only_filename(
                        app.conn.songs_filenames.get(app.song_list.index).unwrap(),
                    );
                    app.conn.push(&song)?;
                }
                SelectedTab::Queue => {
                    let song = app.conn.get_song_with_only_filename(
                        app.queue_list.list.get(app.queue_list.index).unwrap(),
                    );
                    app.conn.push(&song)?;
                }
                SelectedTab::Playlists => {
                    app.conn
                        .push_playlist(app.pl_list.list.get(app.pl_list.index).unwrap())?;
                }
            }
        }

        // Playback controls
        // Toggle Pause
        KeyCode::Char('p') => {
            app.conn.toggle_pause();
        }

        // Pause
        KeyCode::Char('s') => {
            app.conn.pause();
        }

        // Clear Queue
        KeyCode::Char('x') => {
            app.conn.conn.clear()?;
            // app.update_queue();
        }

        // Dmenu prompt
        KeyCode::Char('D') => {
            app.conn.play_dmenu()?;
        }

        // add to queue
        KeyCode::Char('a') => {
            let song = app.conn.get_song_with_only_filename(
                app.conn.songs_filenames.get(app.song_list.index).unwrap(),
            );
            app.conn.conn.push(&song)?;
        }

        KeyCode::Right => {
            app.conn
                .push_playlist(app.pl_list.list.get(app.pl_list.index).unwrap())?;
        }

        KeyCode::Char('f') => {
            let place = app.conn.conn.status().unwrap().song.unwrap().pos;
            let (pos, _) = app.conn.conn.status().unwrap().time.unwrap();
            let pos = Duration::from_secs(pos.as_secs().wrapping_add(2));
            app.conn.conn.seek(place, pos)?;
        }

        KeyCode::Char('b') => {
            let place = app.conn.conn.status().unwrap().song.unwrap().pos;
            let (pos, _) = app.conn.conn.status().unwrap().time.unwrap();
            let pos = Duration::from_secs(pos.as_secs().wrapping_add(2));
            app.conn.conn.seek(place, pos)?;
        }

        KeyCode::Tab => {
            app.cycle_tabls();
        }

        KeyCode::Char('1') => {
            app.selected_tab = SelectedTab::SongList;
        }

        KeyCode::Char('2') => {
            app.selected_tab = SelectedTab::Queue;
        }

        KeyCode::Char('3') => {
            app.selected_tab = SelectedTab::Playlists;
        }

        KeyCode::Char('n') => {
            app.conn.conn.next()?;
        }

        KeyCode::Char('N') => {
            app.conn.conn.prev()?;
        }

        // Volume controls
        KeyCode::Char('=') => {
            app.conn.inc_volume(2);
        }

        KeyCode::Char('-') => {
            app.conn.dec_volume(2);
        }

        // Delete highlighted song from the queue
        KeyCode::Char('d') => {
            app.conn.conn.delete(app.queue_list.index as u32)?;
            app.update_queue();
        }

        KeyCode::Char('U') => {
            app.conn.conn.update()?;
        }

        KeyCode::Char('L') => {
            let str = dmenu!(prompt "Search: ");
            let list = app
                .conn
                .songs_filenames
                .iter()
                .map(|f| f.as_str())
                .collect::<Vec<&str>>();
            let (filename, _) = rust_fuzzy_search::fuzzy_search_sorted(&str, &list)
                .get(0)
                .unwrap()
                .clone();

            let song = app.conn.get_song_with_only_filename(filename);
            app.conn.push(&song)?;
        }

        _ => {}
    }

    Ok(())
}
