use crate::app::App;
use ratatui::{prelude::*, widgets::*};

/// Renders the user interface widgets
pub fn render(app: &mut App, frame: &mut Frame) {
    // This is where you add new widgets.
    // See the following resources:
    // - https://docs.rs/ratatui/latest/ratatui/widgets/index.html
    // - https://github.com/ratatui-org/ratatui/tree/master/examples

    // List of songs
    let mut song_state = ListState::default();
    let size = Rect::new(100, 0, frame.size().width, frame.size().height - 3);
    let list = List::new(app.conn.songs_filenames.clone())
        .block(Block::default().title("Song List").borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    song_state.select(Some(app.song_list.index));
    frame.render_stateful_widget(list, size, &mut song_state);

    // Play Queue
    let mut queue_state = ListState::default();
    let size = Rect::new(0, 0, 100, frame.size().height - 25);
    let list = List::new(app.play_deque.clone())
        .block(Block::default().title("Play Queue").borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    app.update_queue();
    frame.render_stateful_widget(list, size, &mut queue_state);

    // Status
    // let size = Rect::new(0, frame.size().height - 3, frame.size().width, 3);
    // let song = app
    //     .conn
    //     .now_playing()
    //     .unwrap_or_else(|| "No Title Found".to_string());
    //
    // let (elapsed, total) = app.conn.conn.status().unwrap().time.unwrap();
    //
    // let mut lines = vec![];
    // lines.push(Line::from(vec![
    //     Span::styled("Current: ", Style::default().fg(Color::Red)),
    //     Span::styled(song, Style::default().fg(Color::Yellow)),
    //     Span::styled(format!("[{}/{}]", elapsed.as_secs(), total.as_secs()), Style::default().fg(Color::Yellow)),
    // ]));
    // let status = Paragraph::new(Text::from(lines))
    //     .block(Block::default().title("Status").borders(Borders::ALL));
    // frame.render_widget(status, size);

    // Playlists
    let mut state = ListState::default();
    let size = Rect::new(0, 25, 100, frame.size().height - 25 - 3);
    let list = List::new(app.pl_list.list.clone())
        .block(Block::default().title("Playlists").borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    state.select(Some(app.pl_list.index));
    frame.render_stateful_widget(list, size, &mut state);
}
