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
            app.list.next();
        }

        KeyCode::Char('k') => {
            app.list.prev();
        }

        KeyCode::Enter | KeyCode::Char('l') => {
            let song = app.conn.get_song_with_only_filename(app.conn.songs_filenames.get(app.list.index).unwrap());
            app.conn.push(&song).unwrap();
            app.update_queue();
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
            app.update_queue();
        }
        _ => {}
    }

    Ok(())
}
