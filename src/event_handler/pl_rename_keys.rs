use crate::{
    app::{App, AppResult},
    ui::InputMode,
};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_pl_rename_keys(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    match key_event.code {
        KeyCode::Esc => {
            app.pl_newname_input.clear();
            app.reset_cursor();
            app.inputmode = InputMode::Normal;
        }
        KeyCode::Char(to_insert) => {
            app.enter_char(to_insert);
        }
        KeyCode::Enter => {
            app.conn.conn.pl_rename(
                app.pl_list.get_item_at_current_index(),
                &app.pl_newname_input,
            )?;
            app.pl_list.list = App::get_playlist(&mut app.conn.conn)?;
            app.pl_newname_input.clear();
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
