use crate::app::{App, AppResult};
use ratatui::{prelude::*, widgets::*};

/// Renders the user interface widgets
pub fn render(app: &mut App, frame: &mut Frame) {
    // This is where you add new widgets.
    // See the following resources:
    // - https://docs.rs/ratatui/latest/ratatui/widgets/index.html
    // - https://github.com/ratatui-org/ratatui/tree/master/examples

    // Layout
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(93), Constraint::Percentage(7)])
        .split(frame.size());

    let outer_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_layout[0]);

    let inner_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer_layout[1]);

    draw_song_list(frame, app, outer_layout[0]);
    draw_queue(frame, app, inner_layout[0]);
    draw_playlists(frame, app, inner_layout[1]);

    // Status
    // let song = app
    //     .conn
    //     .now_playing().unwrap()
    //     .unwrap_or_else(|| "No Title Found".to_string());
    //
    // let (elapsed, total) = app.conn.conn.status().unwrap().time.unwrap();
    //
    // let mut lines = vec![];
    // lines.push(Line::from(vec![
    //     Span::styled("Current: ", Style::default().fg(Color::Red)),
    //     Span::styled(song, Style::default().fg(Color::Yellow)),
    //     Span::styled(
    //         format!("[{}/{}]", elapsed.as_secs(), total.as_secs()),
    //         Style::default().fg(Color::Yellow),
    //     ),
    // ]));
    // let status = Paragraph::new(Text::from(lines))
    //     .block(Block::default().title("Status").borders(Borders::ALL));
    // frame.render_widget(status, main_layout[1]);
}

/// draws list of songs
fn draw_song_list(frame: &mut Frame, app: &mut App, size: Rect) {
    let mut song_state = ListState::default();
    let list = List::new(app.conn.songs_filenames.clone())
        .block(Block::default().title("Song List").borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    song_state.select(Some(app.song_list.index));
    frame.render_stateful_widget(list, size, &mut song_state);
}

/// draws playing queue
fn draw_queue(frame: &mut Frame, app: &mut App, size: Rect) {
    let mut queue_state = ListState::default();
    let list = List::new(app.play_deque.clone())
        .block(Block::default().title("Play Queue").borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    app.update_queue();
    frame.render_stateful_widget(list, size, &mut queue_state);
}

/// draws all playlists
fn draw_playlists(frame: &mut Frame, app: &mut App, size: Rect) {
    let mut state = ListState::default();
    let list = List::new(app.pl_list.list.clone())
        .block(Block::default().title("Playlists").borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    state.select(Some(app.pl_list.index));
    frame.render_stateful_widget(list, size, &mut state);
}
