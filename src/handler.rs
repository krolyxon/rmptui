use std::time::Duration;

use crate::app::{App, AppResult};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_key_events(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    match key_event.code {
        KeyCode::Char('q') | KeyCode::Esc => app.quit(),
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if key_event.modifiers == KeyModifiers::CONTROL {
                app.quit();
            }
        }

        KeyCode::Char('j') => {
            app.song_list.next();
        }

        KeyCode::Char('k') => {
            app.song_list.prev();
        }

        KeyCode::Enter | KeyCode::Char('l') => {
            let song = app.conn.get_song_with_only_filename(
                app.conn.songs_filenames.get(app.song_list.index).unwrap(),
            );
            app.conn.push(&song)?;
            // app.update_queue();
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

        // Clearn Queue
        KeyCode::Char('x') => {
            app.conn.conn.clear()?;
            // app.update_queue();
        }

        KeyCode::Char('d') => {
            app.conn.play_dmenu()?;
        }

        KeyCode::Down => {
            if key_event.modifiers == KeyModifiers::SHIFT {
                app.queue_list.next();
            } else {
                app.pl_list.next();
            }
        }

        KeyCode::Up => {
            if key_event.modifiers == KeyModifiers::SHIFT {
                app.queue_list.prev();
            } else {
                app.pl_list.prev();
            }
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
        _ => {}
    }

    Ok(())
}
