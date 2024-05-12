use std::time::Duration;

use crate::app::{App, SelectedTab};
use ratatui::{
    prelude::*,
    widgets::{block::Title, *},
};

#[derive(Debug, PartialEq)]
pub enum InputMode {
    Editing,
    Normal,
    PlaylistRename,
}

/// Renders the user interface widgets
pub fn render(app: &mut App, frame: &mut Frame) {
    // Layout
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(93), Constraint::Min(3)])
        .split(frame.size());

    match app.selected_tab {
        SelectedTab::Queue => draw_queue(frame, app, layout[0]),
        SelectedTab::Playlists => draw_playlist_viewer(frame, app, layout[0]),
        SelectedTab::DirectoryBrowser => draw_directory_browser(frame, app, layout[0]),
    }

    match app.inputmode {
        InputMode::Normal => {
            draw_progress_bar(frame, app, layout[1]);
        }
        InputMode::Editing => {
            draw_search_bar(frame, app, layout[1]);
        }
        InputMode::PlaylistRename => {
            draw_rename_playlist(frame, app, layout[1]);
        }
    }

    if app.playlist_popup {
        draw_add_to_playlist(frame, app, layout[0]);
    }
}

/// Draws the directory
fn draw_directory_browser(frame: &mut Frame, app: &mut App, size: Rect) {
    let total_songs = app.conn.stats.songs.to_string();

    let rows = app.browser.filetree.iter().enumerate().map(|(i, (t, s))| {
        if t == "file" {
            let song = app.browser.songs.get(i).unwrap().clone();

            // metadata
            let title = song.clone().title.unwrap_or_else(|| song.clone().file);
            let artist = song.clone().artist.unwrap_or_default().cyan();
            let album = song
                .tags
                .iter()
                .filter(|(x, _)| x == "Album")
                .map(|(_, l)| l.clone())
                .collect::<Vec<String>>()
                .join("");

            let track = song
                .tags
                .iter()
                .filter(|(x, _)| x == "Track")
                .map(|(_, l)| l.clone())
                .collect::<Vec<String>>()
                .join("");

            let time =
                App::format_time(song.clone().duration.unwrap_or_else(|| Duration::new(0, 0)));

            let mut status: bool = false;
            for sn in app.queue_list.list.iter() {
                if sn.file.contains(s) {
                    status = true;
                }
            }

            let row = Row::new(vec![
                Cell::from(artist),
                Cell::from(track.green()),
                Cell::from(title),
                Cell::from(album.cyan()),
                Cell::from(time.to_string().magenta()),
            ]);

            if status {
                row.magenta().bold()
            } else {
                row
            }
        } else {
            let row = Row::new(vec![Cell::from(format!("[{}]", *s))]);
            row
        }
    });

    let mut state = TableState::new();
    let header = ["Artist", "Track", "Title", "Album", "Time"]
        .into_iter()
        .map(Cell::from)
        .collect::<Row>()
        .bold()
        .height(1);
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(34),
            Constraint::Percentage(3),
            Constraint::Min(30),
            Constraint::Percentage(30),
            Constraint::Percentage(3),
        ],
    )
    .block(
        Block::default()
            .title(format!("Song Browser: {}", app.browser.path.clone()).bold())
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
    .highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Cyan)
            .bg(Color::Black),
    )
    .header(header)
    .flex(layout::Flex::Legacy);

    state.select(Some(app.browser.selected));
    frame.render_stateful_widget(table, size, &mut state);
}

/// draws playing queue
fn draw_queue(frame: &mut Frame, app: &mut App, size: Rect) {
    let rows = app.queue_list.list.iter().enumerate().map(|(i, song)| {
        // metadata
        let title = song.clone().title.unwrap_or_else(|| song.clone().file);
        let artist = song.clone().artist.unwrap_or_default().cyan();
        let album = song
            .tags
            .iter()
            .filter(|(x, _)| x == "Album")
            .map(|(_, l)| l.clone())
            .collect::<Vec<String>>()
            .join("");

        let track = song
            .tags
            .iter()
            .filter(|(x, _)| x == "Track")
            .map(|(_, l)| l.clone())
            .collect::<Vec<String>>()
            .join("");

        let time = App::format_time(song.clone().duration.unwrap_or_else(|| Duration::new(0, 0)));

        let row = Row::new(vec![
            Cell::from(artist),
            Cell::from(track.green()),
            Cell::from(title),
            Cell::from(album.cyan()),
            Cell::from(time.to_string().magenta()),
        ]);

        let place = app.conn.current_song.place;
        if let Some(pos) = place {
            if i == pos.pos as usize {
                row.magenta().bold()
            } else {
                row
            }
        } else {
            row
        }
    });

    let mut state = TableState::new();
    let header = ["Artist", "Track", "Title", "Album", "Time"]
        .into_iter()
        .map(Cell::from)
        .collect::<Row>()
        .bold()
        .height(1);
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(34),
            Constraint::Percentage(3),
            Constraint::Min(30),
            Constraint::Percentage(30),
            Constraint::Percentage(3),
        ],
    )
    .block(
        Block::default()
            .title(Title::from("Play Queue".green().bold()))
            .title(Title::from(
                format!("({} items)", app.queue_list.list.len()).bold(),
            ))
            .title(
                Title::from(format!("Volume: {}%", app.conn.volume).bold().green())
                    .alignment(Alignment::Right),
            )
            .borders(Borders::ALL),
    )
    .highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Cyan)
            .bg(Color::Black),
    )
    .header(header)
    .flex(layout::Flex::Legacy);

    state.select(Some(app.queue_list.index));
    frame.render_stateful_widget(table, size, &mut state);
}

// Draw search bar
fn draw_search_bar(frame: &mut Frame, app: &mut App, size: Rect) {
    // Make the cursor visible and ask ratatui to put it at the specified coordinates after
    // rendering
    #[allow(clippy::cast_possible_truncation)]
    frame.set_cursor(
        // Draw the cursor at the current position in the input field.
        // This position is can be controlled via the left and right arrow key
        size.x + app.search_cursor_pos as u16 + 2,
        // Move one line down, from the border to the input line
        size.y + 1,
    );

    let input = Paragraph::new("/".to_string() + &app.search_input)
        .style(Style::default())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search Forward: ".bold().green()),
        );
    frame.render_widget(input, size);
}

/// Draws Progress Bar
fn draw_progress_bar(frame: &mut Frame, app: &mut App, size: Rect) {
    // Get the current playing song
    let song = app
        .conn
        .now_playing()
        .unwrap()
        .unwrap_or_else(|| "No Title Found".to_string());

    // Get the current playing state
    let mut state: String = String::new();
    if !app.queue_list.list.is_empty() {
        state = app.conn.state.clone();
        state.push(':');
    }

    // Get the current modes
    let mut modes_bottom: String = String::new();
    // we do this to check if at least one mode is enabled so we can push "[]"
    if app.conn.repeat | app.conn.random {
        modes_bottom.push('r');
    }

    if !modes_bottom.is_empty() {
        modes_bottom.clear();
        modes_bottom.push('[');
        if app.conn.repeat {
            modes_bottom.push('r');
        }
        if app.conn.random {
            modes_bottom.push('z');
        }
        modes_bottom.push(']');
    };

    // get the duration
    let duration = if app.conn.total_duration.as_secs() != 0 {
        format!(
            "[{}/{}]",
            App::format_time(app.conn.elapsed),
            App::format_time(app.conn.total_duration)
        )
    } else {
        "".to_string()
    };

    // Define the title
    let title = Block::default()
        .title(Title::from(state.red().bold()))
        .title(Title::from(song.green().bold()))
        .title(Title::from(duration.cyan().bold()).alignment(Alignment::Right))
        .title(Title::from(modes_bottom).position(block::Position::Bottom))
        .borders(Borders::ALL);

    let progress_bar = LineGauge::default()
        .block(title.borders(Borders::ALL))
        .gauge_style(
            Style::default()
                .fg(Color::Blue)
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .line_set(symbols::line::THICK)
        .ratio(app.conn.get_progress_ratio());

    frame.render_widget(progress_bar, size);
}

fn draw_playlist_viewer(frame: &mut Frame, app: &mut App, area: Rect) {
    let layouts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Draw list of playlists
    let mut state = ListState::default();
    let title = Block::default().title(Title::from("Playlist".green().bold()));
    let list = List::new(app.pl_list.list.clone())
        .block(title.borders(Borders::ALL))
        .highlight_style(
            Style::new()
                .fg(Color::Cyan)
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::REVERSED),
        )
        .repeat_highlight_symbol(true);
    state.select(Some(app.pl_list.index));
    frame.render_stateful_widget(list, layouts[0], &mut state);

    // Playlist viewer

    // Handle if there are no playlists in the mpd database
    if app.pl_list.list.is_empty() {
        let title = Block::default()
            .title(Title::from("No Playlists Found".red().bold()))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL);
        frame.render_widget(Clear, area); //this clears out the background
        frame.render_widget(title, area);
        return;
    }

    let pl_name = app.pl_list.list.get(app.pl_list.index).unwrap();
    let songs = app.conn.conn.playlist(pl_name).unwrap();
    let rows = songs.iter().map(|song| {
        let title = song.clone().title.unwrap_or_default().cyan();
        let artist = song.clone().artist.unwrap_or_else(|| song.clone().file);
        let time = App::format_time(song.clone().duration.unwrap_or_else(|| Duration::new(0, 0)));

        let row = Row::new(vec![
            Cell::from(artist),
            Cell::from(title),
            Cell::from(time.to_string().green()),
        ]);
        row
    });

    let title = Block::default()
        .title(format!("Content: ({} items)", songs.len()).bold())
        .borders(Borders::ALL);
    let table = Table::new(
        rows,
        vec![
            Constraint::Min(48),
            Constraint::Percentage(48),
            Constraint::Percentage(4),
        ],
    )
    .block(title)
    .highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Cyan)
            .bg(Color::Black),
    )
    .flex(layout::Flex::SpaceBetween);
    frame.render_widget(table, layouts[1]);
}

fn draw_add_to_playlist(frame: &mut Frame, app: &mut App, area: Rect) {
    let area = centered_rect(40, 50, area);
    let mut state = ListState::default();
    let title = Block::default()
        .title(Title::from("Add Selected Item to: "))
        .title(Title::from("<Esc> to Cancel".green().bold()).alignment(Alignment::Right));
    let list = List::new(app.append_list.list.clone())
        .block(title.borders(Borders::ALL))
        .highlight_style(
            Style::new()
                .fg(Color::Cyan)
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::REVERSED),
        )
        .repeat_highlight_symbol(true);

    state.select(Some(app.append_list.index));
    frame.render_widget(Clear, area); //this clears out the background
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_rename_playlist(frame: &mut Frame, app: &mut App, area: Rect) {
    #[allow(clippy::cast_possible_truncation)]
    frame.set_cursor(
        // Draw the cursor at the current position in the input field.
        // This position is can be controlled via the left and right arrow key
        area.x + app.pl_cursor_pos as u16 + 2,
        // Move one line down, from the border to the input line
        area.y + 1,
    );

    let input = Paragraph::new("/".to_string() + &app.pl_newname_input)
        .style(Style::default())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Enter New Name: ".bold().green()),
        );
    frame.render_widget(input, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
