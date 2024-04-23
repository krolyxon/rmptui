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
            let song = app.conn.get_song_with_only_filename(app.conn.songs_filenames.get(app.song_list.index).unwrap());
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

        KeyCode::Down=> {
            app.pl_list.next();
        }

        KeyCode::Up=> {
            app.pl_list.prev();
        }


        KeyCode::Right => {
            app.conn.push_playlist(app.pl_list.list.get(app.pl_list.index).unwrap())?;
        }

        KeyCode::Char('f')=> {
            // let place = app.conn.conn.status().unwrap().duration;
            let (pos, _) = app.conn.conn.status().unwrap().time.unwrap();
            let pos: i64 = (pos.as_secs() + 2).try_into().unwrap();
            app.conn.conn.seek(2, pos )?;
        }

        KeyCode::Char('b')=> {
            // let place = app.conn.conn.status().unwrap().duration;
            let (pos, _) = app.conn.conn.status().unwrap().time.unwrap();
            let pos: i64 = (pos.as_secs() - 2).try_into().unwrap();
            app.conn.conn.seek(2, pos )?;
        }
        _ => {}
    }

    Ok(())
}
