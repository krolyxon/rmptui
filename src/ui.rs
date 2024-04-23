use crate::{app::App, connection::Connection};
use ratatui::{prelude::*, widgets::*};

/// Renders the user interface widgets
pub fn render(app: &mut App, frame: &mut Frame) {
    // This is where you add new widgets.
    // See the following resources:
    // - https://docs.rs/ratatui/latest/ratatui/widgets/index.html
    // - https://github.com/ratatui-org/ratatui/tree/master/examples

    // List of songs
    let mut state = ListState::default();
    let size = Rect::new(100, 0, frame.size().width, frame.size().height - 3);
    let list = List::new(app.conn.songs_filenames.clone())
        .block(Block::default().title("Song List").borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    state.select(Some(app.list.index));
    frame.render_stateful_widget(list, size, &mut state);

    // Play Queue
    let size = Rect::new(0, 0, 100, frame.size().height - 25);
    let list = List::new(app.play_deque.clone())
        .block(Block::default().title("Play Queue").borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);
    frame.render_widget(list, size);

    // Status
    let size = Rect::new(0, frame.size().height - 3, frame.size().width, 3);
    let status = Paragraph::new(app.conn.conn.status().unwrap().volume.to_string())
        .block(Block::default().title("Status").borders(Borders::ALL));
    frame.render_widget(status, size);
}
