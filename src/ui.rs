use std::time::Duration;

use crate::{
    app::{App, PlaylistFocus, SelectedTab},
    browser::BrowserEntry,
    connection::{PlaybackState, Song, VolumeStatus},
};
use ratatui::{prelude::*, widgets::*};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum InputMode {
    Editing,
    Normal,
    PlaylistRename,
    NewPlaylist,
}

fn song_row(song: &Song, playing: bool, queued: bool) -> Row<'static> {
    let mut artist = song.artist.clone().unwrap_or_default();
    let track = tag_value(song, "Track");
    let title = song.title.clone().unwrap_or_else(|| song.file.clone());
    let album = tag_value(song, "Album");
    let time = App::format_time(song.duration.unwrap_or(Duration::ZERO));

    if playing {
        artist.insert_str(0, "> ");
    }
    let row = Row::new(vec![
        Cell::from(artist.cyan()),
        Cell::from(if playing {
            format!(" {track}").green()
        } else {
            track.green()
        }),
        Cell::from(if playing { format!(" {title}") } else { title }),
        Cell::from(album.cyan()),
        Cell::from(time.magenta()),
    ]);
    if queued {
        row.bold()
    } else {
        row
    }
}

fn tag_value(song: &Song, name: &str) -> String {
    song.tags
        .iter()
        .find(|(tag, _)| tag.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
        .unwrap_or_default()
}

/// Renders the user interface widgets
pub fn render(app: &mut App, frame: &mut Frame) {
    // Layout
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(93), Constraint::Min(3)])
        .split(frame.area());

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
        InputMode::NewPlaylist => {
            draw_new_playlist(frame, app, layout[1]);
        }
    }

    if app.playlist_popup {
        draw_add_to_playlist(frame, app, layout[0]);
    }
}

/// Draws the directory
fn draw_directory_browser(frame: &mut Frame, app: &mut App, size: Rect) {
    let total_songs = app.conn.stats.songs.to_string();

    let rows = app.browser.entries.iter().map(|entry| match entry {
        BrowserEntry::Song(song) => {
            let queued =
                app.queue_list.list.iter().any(|queued| {
                    queued.file.rsplit('/').next().unwrap_or_default() == entry.name()
                });
            song_row(song, false, queued)
        }
        BrowserEntry::Directory(name) => directory_row(name),
    });

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
            .title(format!("Song Browser: {}", app.browser.path).bold())
            .title(
                Line::from(format!("Total Songs: {}", total_songs).green())
                    .alignment(Alignment::Center),
            )
            .title(match app.conn.volume_status {
                VolumeStatus::Unmuted => {
                    Line::from(format!("Volume: {}%", app.conn.status.volume).green())
                        .alignment(Alignment::Right)
                }
                VolumeStatus::Muted(_v) => Line::from("Muted".red()).alignment(Alignment::Right),
            })
            .borders(Borders::ALL),
    )
    .row_highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Cyan)
            .bg(Color::Black),
    )
    .header(header)
    .flex(layout::Flex::Legacy);

    app.browser_state.select(Some(app.browser.selected));
    frame.render_stateful_widget(table, size, &mut app.browser_state);
}

/// draws playing queue
fn draw_queue(frame: &mut Frame, app: &mut App, size: Rect) {
    let rows = app.queue_list.list.iter().enumerate().map(|(i, song)| {
        let playing = app
            .conn
            .current_song
            .place
            .is_some_and(|position| i == position.pos as usize);
        song_row(song, playing, false)
    });

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
            .title("Play Queue".green().bold())
            .title(format!("({} items)", app.queue_list.list.len()).bold())
            .title(match app.conn.volume_status {
                VolumeStatus::Unmuted => {
                    Line::from(format!("Volume: {}%", app.conn.status.volume).green())
                        .alignment(Alignment::Right)
                }
                VolumeStatus::Muted(_v) => Line::from("Muted".red()).alignment(Alignment::Right),
            })
            .borders(Borders::ALL),
    )
    .row_highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Cyan)
            .bg(Color::Black),
    )
    .header(header)
    .flex(layout::Flex::Legacy);

    app.queue_state.select(Some(app.queue_list.index));
    frame.render_stateful_widget(table, size, &mut app.queue_state);
}

// Draw search bar
fn draw_search_bar(frame: &mut Frame, app: &mut App, size: Rect) {
    // Make the cursor visible and ask ratatui to put it at the specified coordinates after
    // rendering
    #[allow(clippy::cast_possible_truncation)]
    frame.set_cursor_position((
        // Draw the cursor at the current position in the input field.
        // This position is can be controlled via the left and right arrow key
        size.x + app.search_cursor_pos as u16 + 2,
        // Move one line down, from the border to the input line
        size.y + 1,
    ));

    let input = Paragraph::new("/".to_string() + &app.search_input)
        .style(Style::default())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search: Enter to keep, n/N next/previous ".bold().green()),
        );
    frame.render_widget(input, size);
}

/// Draws Progress Bar
fn draw_progress_bar(frame: &mut Frame, app: &mut App, size: Rect) {
    let song = app.conn.now_playing();

    // Get the current playing state
    let mut state = String::new();
    if !app.queue_list.list.is_empty() {
        state.push_str(match app.conn.status.state {
            PlaybackState::Playing => "Playing",
            PlaybackState::Paused => "Paused",
            PlaybackState::Stopped => "Stopped",
        });
        state.push(':');
    }

    // Get the current modes
    let mut modes_bottom: String = String::new();
    // we do this to check if at least one mode is enabled so we can push "[]"
    if app.conn.status.repeat | app.conn.status.random {
        modes_bottom.push('r');
    }

    if !modes_bottom.is_empty() {
        modes_bottom.clear();
        modes_bottom.push('[');
        if app.conn.status.repeat {
            modes_bottom.push('r');
        }
        if app.conn.status.random {
            modes_bottom.push('z');
        }
        modes_bottom.push(']');
    };

    // get the duration
    let duration = if !app.conn.status.total.is_zero() {
        format!(
            "[{}/{}]",
            App::format_time(app.conn.current_elapsed()),
            App::format_time(app.conn.status.total)
        )
    } else {
        "".to_string()
    };

    // Define the title
    let title = Block::default()
        .title(state.red().bold())
        .title(song.green())
        .title(Line::from(duration.cyan().bold()).alignment(Alignment::Right))
        .title_bottom(modes_bottom)
        .borders(Borders::ALL);

    let progress_bar = LineGauge::default()
        .block(title.borders(Borders::ALL))
        .filled_style(
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .unfilled_style(
            Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .filled_symbol(symbols::line::THICK_HORIZONTAL)
        .unfilled_symbol(symbols::line::THICK_HORIZONTAL)
        .ratio(app.conn.get_progress_ratio());

    frame.render_widget(progress_bar, size);
}

fn draw_playlist_viewer(frame: &mut Frame, app: &mut App, area: Rect) {
    let layouts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Draw list of playlists
    let list_focused = app.playlist_focus == PlaylistFocus::List;
    let title = Block::default()
        .title("Playlists".green().bold())
        .borders(Borders::ALL)
        .border_style(if list_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        });
    let list = List::new(app.pl_list.list.iter().map(String::as_str))
        .block(title)
        .highlight_style(Style::new().fg(Color::Cyan).bg(Color::Black).add_modifier(
            if list_focused {
                Modifier::BOLD | Modifier::REVERSED
            } else {
                Modifier::BOLD
            },
        ))
        .repeat_highlight_symbol(true);
    app.playlists_state.select(Some(app.pl_list.index));
    frame.render_stateful_widget(list, layouts[0], &mut app.playlists_state);

    // Playlist viewer

    // Handle if there are no playlists in the mpd database
    if app.pl_list.list.is_empty() {
        let title = Block::default()
            .title("No Playlists Found".red().bold())
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL);
        frame.render_widget(Clear, area); //this clears out the background
        frame.render_widget(title, area);
        return;
    }

    let songs = &app.playlist_contents;
    let rows = songs.iter().map(|song| {
        let title = song.title.as_deref().unwrap_or_default().cyan();
        let artist = song.artist.as_deref().unwrap_or(&song.file);
        let time = App::format_time(song.duration.unwrap_or(Duration::ZERO));

        let row = Row::new(vec![
            Cell::from(artist),
            Cell::from(title),
            Cell::from(time.to_string().green()),
        ]);
        row
    });

    let title = Block::default()
        .title(format!("Content: ({} items)", songs.len()).bold())
        .borders(Borders::ALL)
        .border_style(if app.playlist_focus == PlaylistFocus::Songs {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        });
    let table = Table::new(
        rows,
        vec![
            Constraint::Percentage(40),
            Constraint::Percentage(50),
            Constraint::Percentage(10),
        ],
    )
    .block(title)
    .row_highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Cyan)
            .bg(Color::Black),
    )
    .flex(layout::Flex::SpaceBetween);
    app.playlist_songs_state
        .select(Some(app.playlist_song_index));
    frame.render_stateful_widget(table, layouts[1], &mut app.playlist_songs_state);
}

fn draw_add_to_playlist(frame: &mut Frame, app: &mut App, area: Rect) {
    let area = centered_rect(40, 50, area);
    let mut state = ListState::default();
    let title = Block::default()
        .title("Add Selected Item to: ")
        .title(Line::from("<Esc> to Cancel".green().bold()).alignment(Alignment::Right));
    let list = List::new(app.append_list.list.iter().map(String::as_str))
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
    frame.set_cursor_position((
        // Draw the cursor at the current position in the input field.
        // This position is can be controlled via the left and right arrow key
        area.x + app.pl_cursor_pos as u16 + 2,
        // Move one line down, from the border to the input line
        area.y + 1,
    ));

    let input = Paragraph::new("/".to_string() + &app.pl_newname_input)
        .style(Style::default())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Enter New Name: ".bold().green()),
        );
    frame.render_widget(input, area);
}

fn draw_new_playlist(frame: &mut Frame, app: &mut App, area: Rect) {
    #[allow(clippy::cast_possible_truncation)]
    frame.set_cursor_position((
        // Draw the cursor at the current position in the input field.
        // This position is can be controlled via the left and right arrow key
        area.x + app.pl_new_pl_cursor_pos as u16 + 2,
        // Move one line down, from the border to the input line
        area.y + 1,
    ));

    let input = Paragraph::new("/".to_string() + &app.pl_new_pl_input)
        .style(Style::default())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Enter New Playlist's Name: ".bold().green()),
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

fn directory_row(name: &str) -> Row<'static> {
    Row::new(vec![Cell::from(format!("[{name}]")).column_span(5)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_names_use_the_full_browser_row_width() {
        let name = "mr-robot-vol1-long-name";
        let area = Rect::new(0, 0, 80, 1);
        let mut buffer = Buffer::empty(area);
        let table = Table::new(
            [directory_row(name)],
            [
                Constraint::Length(6),
                Constraint::Length(2),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(6),
            ],
        );
        Widget::render(table, area, &mut buffer);

        let rendered = (0..area.width)
            .map(|x| buffer[(x, 0)].symbol())
            .collect::<String>();
        assert!(rendered.contains(&format!("[{name}]")));
    }
}
