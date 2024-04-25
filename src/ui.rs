use crate::app::{App, SelectedTab};
use ratatui::{
    prelude::*,
    widgets::{block::Title, *},
};

/// Renders the user interface widgets
pub fn render(app: &mut App, frame: &mut Frame) {
    // This is where you add new widgets.
    // See the following resources:
    // - https://docs.rs/ratatui/latest/ratatui/widgets/index.html
    // - https://github.com/ratatui-org/ratatui/tree/master/examples

    // Layout
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(93), Constraint::Percentage(7)])
        .split(frame.size());

    match app.selected_tab {
        SelectedTab::Queue => draw_queue(frame, app, layout[0]),
        SelectedTab::Playlists => draw_playlists(frame, app, layout[0]),
        SelectedTab::DirectoryBrowser => draw_directory_browser(frame, app, layout[0]),
    }

    draw_progress_bar(frame, app, layout[1]);
}

/// Draws the file tree browser
fn draw_directory_browser(frame: &mut Frame, app: &mut App, size: Rect) {
    let mut song_state = ListState::default();
    let total_songs = app.conn.conn.stats().unwrap().songs.to_string();
    let mut list: Vec<String> = vec![];
    for (t, s) in app.browser.filetree.iter() {
        if t == "file" {
            list.push(s.to_string());
        } else {
            list.push(format!("[{}]", *s));
        }
    }
    let list = List::new(list)
        .block(
            Block::default()
                .title(format!("File Browser: {}", app.browser.path.clone()).bold())
                .title(
                    Title::from(format!("Total Songs: {}", total_songs).bold().green())
                        .alignment(Alignment::Center),
                )
                .title(
                    Title::from(format!("Volume: {}%", app.conn.volume).bold().green())
                        .alignment(Alignment::Right),
                )
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true)
        .scroll_padding(20);

    song_state.select(Some(app.browser.selected));
    frame.render_stateful_widget(list, size, &mut song_state);
}

/// draws playing queue
fn draw_queue(frame: &mut Frame, app: &mut App, size: Rect) {
    let mut queue_state = ListState::default();
    let title = Block::default()
        .title(Title::from("Play Queue".green().bold()))
        .title(
            Title::from(format!("Volume: {}%", app.conn.volume).bold().green())
                .alignment(Alignment::Right),
        );
    let list = List::new(app.queue_list.list.clone())
        .block(title.borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    queue_state.select(Some(app.queue_list.index));
    frame.render_stateful_widget(list, size, &mut queue_state);
}

/// draws all playlists
fn draw_playlists(frame: &mut Frame, app: &mut App, size: Rect) {
    let mut state = ListState::default();

    let title = Block::default()
        .title(Title::from("Playlist".green().bold()))
        .title(
            Title::from(format!("Volume: {}%", app.conn.volume).bold().green())
                .alignment(Alignment::Right),
        );

    let list = List::new(app.pl_list.list.clone())
        .block(title.borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    state.select(Some(app.pl_list.index));
    frame.render_stateful_widget(list, size, &mut state);
}

/// Draws Progress Bar
fn draw_progress_bar(frame: &mut Frame, app: &mut App, size: Rect) {
    let song = app
        .conn
        .now_playing()
        .unwrap()
        .unwrap_or_else(|| "No Title Found".to_string());

    let state = &app.conn.state;
    // let (elapsed, total) = app.conn.conn.status().unwrap().time.unwrap_or_default();

    let title = Block::default()
        .title(Title::from(format!("{}: ", state).red().bold()))
        .title(Title::from(song.green().bold()))
        .title(
            Title::from(
                format!(
                    "{}/{}",
                    app.conn.elapsed.as_secs(),
                    app.conn.total_duration.as_secs()
                )
                .cyan()
                .bold(),
            )
            .alignment(Alignment::Right),
        )
        .borders(Borders::ALL);

    // .title(Title::from(app.conn.conn.status().unwrap_or_default().volume.to_string().yellow())).title_alignment(Alignment::Right);
    let progress_bar = LineGauge::default()
        .block(title.borders(Borders::ALL))
        .gauge_style(
            Style::default()
                .fg(Color::LightBlue)
                .bg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        )
        .line_set(symbols::line::THICK)
        .ratio(app.conn.get_progress_ratio());

    frame.render_widget(progress_bar, size);
}
