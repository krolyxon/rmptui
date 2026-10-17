//! The ncmpcpp-style frame shared by all screens, plus common widgets.
//!
//! ```text
//! Title of the screen                                        Volume: 100%
//! ──────────────────────────────────────────────────────────────────[rz]─
//! (screen)
//! ━━━━━━━━━━━━━━━━━━━━━
//! Playing: "Title" by Artist in Album (Year)        (320 kbps) [1:41/3:56]
//! ```

pub mod input;
pub mod list;

use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    config::{Column, ColumnField, Theme},
    mpd::{PlayState, Song},
    state::MpdState,
    util::format_duration,
};

/// The rows of the frame around the active screen.
pub struct FrameLayout {
    pub title: Rect,
    pub separator: Rect,
    pub body: Rect,
    pub progress: Rect,
    pub status: Rect,
}

impl FrameLayout {
    pub fn new(area: Rect) -> Self {
        let [title, separator, body, progress, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);
        Self {
            title,
            separator,
            body,
            progress,
            status,
        }
    }
}

/// Draws the title row and the line below it.
pub fn draw_title(
    frame: &mut Frame,
    layout: &FrameLayout,
    title: Line,
    mpd: &MpdState,
    theme: &Theme,
) {
    let volume = match mpd.status.volume {
        _ if !mpd.connected => "Not connected".to_owned(),
        Some(0) => "Volume: muted".to_owned(),
        Some(v) => format!("Volume: {v}%"),
        None => "Volume: n/a".to_owned(),
    };
    let [left, right] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(volume.width() as u16),
    ])
    .areas(layout.title);
    frame.render_widget(title.style(theme.header()), left);
    frame.render_widget(Span::styled(volume, theme.dim()), right);

    // Active modes sit on the line, ncmpcpp style: ───[rz]─
    let status = &mpd.status;
    let flags: String = [
        (status.repeat, 'r'),
        (status.random, 'z'),
        (status.single.is_on(), 's'),
        (status.consume.is_on(), 'c'),
        (status.updating_db, 'U'),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, c)| *c)
    .collect();
    draw_hline(frame, layout.separator, theme);
    if !flags.is_empty() {
        let text = format!("[{flags}]");
        let width = text.width() as u16;
        let x = layout.separator.right().saturating_sub(width + 1);
        let area = Rect::new(x, layout.separator.y, width, 1);
        frame.render_widget(
            Span::styled(text, theme.text().add_modifier(Modifier::BOLD)),
            area,
        );
    }
}

/// A horizontal `─` line across `area`.
pub fn draw_hline(frame: &mut Frame, area: Rect, theme: &Theme) {
    frame.render_widget(
        Span::styled(
            symbols::line::HORIZONTAL.repeat(usize::from(area.width)),
            theme.border(),
        ),
        area,
    );
}

/// Splits `area` into columns separated by `│` lines and draws the lines.
pub fn split_columns(
    frame: &mut Frame,
    area: Rect,
    constraints: &[Constraint],
    theme: &Theme,
) -> Vec<Rect> {
    let mut with_separators = Vec::with_capacity(constraints.len() * 2);
    for (i, constraint) in constraints.iter().enumerate() {
        if i > 0 {
            with_separators.push(Constraint::Length(1));
        }
        with_separators.push(*constraint);
    }
    let areas = Layout::horizontal(with_separators).split(area);
    let mut columns = Vec::with_capacity(constraints.len());
    for (i, rect) in areas.iter().enumerate() {
        if i % 2 == 1 {
            let line: Vec<Line> = (0..rect.height)
                .map(|_| Line::styled(symbols::line::VERTICAL, theme.border()))
                .collect();
            frame.render_widget(Paragraph::new(line), *rect);
        } else {
            columns.push(*rect);
        }
    }
    columns
}

/// Column widths of a song table.
pub fn song_widths(columns: &[Column]) -> Vec<Constraint> {
    columns
        .iter()
        .map(|c| match c.field {
            ColumnField::Duration | ColumnField::Track | ColumnField::Disc => {
                Constraint::Length(c.width)
            }
            _ => Constraint::Fill(c.width),
        })
        .collect()
}

/// The header row of a song table.
pub fn song_header(columns: &[Column]) -> Row<'static> {
    Row::new(columns.iter().map(|c| {
        let title = c.field.title();
        match c.field {
            ColumnField::Duration => Cell::from(Line::from(title).right_aligned()),
            _ => Cell::from(title),
        }
    }))
}

/// One song as a table row. The playing song is prefixed with `> `.
pub fn song_row(song: &Song, columns: &[Column], playing: bool, theme: &Theme) -> Row<'static> {
    let cells = columns.iter().enumerate().map(|(i, c)| {
        let mut text = song_field(song, c.field);
        if playing && i == 0 {
            text.insert_str(0, "> ");
        }
        let style = match c.field {
            ColumnField::Artist | ColumnField::AlbumArtist => Style::new().fg(theme.artist),
            ColumnField::Track | ColumnField::Disc => Style::new().fg(theme.track),
            ColumnField::Album => Style::new().fg(theme.album),
            ColumnField::Duration => Style::new().fg(theme.duration),
            _ => theme.text(),
        };
        match c.field {
            ColumnField::Duration => Cell::from(Line::styled(text, style).right_aligned()),
            // Track numbers are padded like ncmpcpp: ` 1` → `01`.
            ColumnField::Track | ColumnField::Disc if !text.is_empty() => {
                Cell::from(Span::styled(format!("{text:0>2}"), style))
            }
            _ => Cell::from(Span::styled(text, style)),
        }
    });
    let row = Row::new(cells);
    if playing {
        row.style(theme.playing())
    } else {
        row
    }
}

/// The text of one song column.
pub fn song_field(song: &Song, field: ColumnField) -> String {
    let text = match field {
        ColumnField::Artist => song.artist(),
        ColumnField::AlbumArtist => song.album_artist(),
        ColumnField::Title => Some(song.display_title()),
        ColumnField::Album => song.album(),
        ColumnField::Track => song.track(),
        ColumnField::Disc => song.tag("Disc"),
        ColumnField::Date => song.tag("Date"),
        ColumnField::Genre => song.tag("Genre"),
        ColumnField::Duration => return song.duration.map(format_duration).unwrap_or_default(),
        ColumnField::File => Some(song.file.as_str()),
    };
    clean(text.unwrap_or_default())
}

/// Removes zero-width characters (variation selectors, joiners) for display.
/// Terminals and ratatui disagree on their width, which leaves stray
/// characters on screen.
pub fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\u{FE00}'..='\u{FE0F}' | '\u{200B}'..='\u{200D}' | '\u{2060}'))
        .collect()
}

/// `Artist - Title`, or just the title.
pub fn song_label(song: &Song) -> String {
    clean(&match song.artist() {
        Some(artist) => format!("{artist} - {}", song.display_title()),
        None => song.display_title().to_owned(),
    })
}

/// The text a song is searched by.
pub fn song_search_text(song: &Song) -> String {
    format!(
        "{} {} {}",
        song.artist().unwrap_or_default(),
        song.display_title(),
        song.album().unwrap_or_default()
    )
}

/// Draws the thin progress bar.
pub fn draw_progress(frame: &mut Frame, area: Rect, mpd: &MpdState, theme: &Theme) {
    let total = mpd.status.duration.unwrap_or_default();
    let width = usize::from(area.width);
    let filled = if total.is_zero() || mpd.status.state == PlayState::Stop {
        0
    } else {
        ((mpd.elapsed().as_secs_f64() / total.as_secs_f64()) * width as f64).round() as usize
    }
    .min(width);
    frame.render_widget(
        Span::styled(
            symbols::line::THICK_HORIZONTAL.repeat(filled),
            Style::new().fg(theme.progress_filled),
        ),
        area,
    );
}

/// Maps a click on the progress bar to a position in the song.
pub fn progress_seek_target(area: Rect, x: u16, mpd: &MpdState) -> Option<Duration> {
    let total = mpd.status.duration?;
    if area.width == 0 || x < area.x || x >= area.right() {
        return None;
    }
    Some(total.mul_f64(f64::from(x - area.x) / f64::from(area.width)))
}

/// `Playing: "Title" by Artist in Album (Year)` and `(320 kbps) [1:41/3:56]`.
pub fn draw_now_playing(frame: &mut Frame, area: Rect, mpd: &MpdState, theme: &Theme) {
    let status = &mpd.status;
    let (Some(song), PlayState::Play | PlayState::Pause) = (&mpd.current, status.state) else {
        return;
    };
    let mut right = String::new();
    if let Some(bitrate) = status.bitrate {
        right.push_str(&format!("({bitrate} kbps) "));
    }
    right.push_str(&format!(
        "[{}/{}]",
        format_duration(mpd.elapsed()),
        format_duration(status.duration.unwrap_or_default())
    ));
    let [left_area, right_area] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(right.width() as u16),
    ])
    .areas(area);

    let state = if status.state == PlayState::Play {
        "Playing: "
    } else {
        "Paused: "
    };
    let bold = theme.text().add_modifier(Modifier::BOLD);
    let mut spans = vec![
        Span::styled(state, theme.dim()),
        Span::styled(format!("\"{}\"", clean(song.display_title())), bold),
    ];
    if let Some(artist) = song.artist() {
        spans.push(Span::styled(" by ", theme.dim()));
        spans.push(Span::styled(clean(artist), theme.text()));
    }
    if let Some(album) = song.album() {
        spans.push(Span::styled(" in ", theme.dim()));
        let album = match song.tag("Date") {
            Some(date) => format!("{album} ({})", date.chars().take(4).collect::<String>()),
            None => album.to_owned(),
        };
        spans.push(Span::styled(clean(&album), theme.text()));
    }
    frame.render_widget(Line::from(spans), left_area);
    frame.render_widget(Span::styled(right, theme.text()), right_area);
}

/// A one-line message, styled as an error if needed.
pub fn draw_message(frame: &mut Frame, area: Rect, text: &str, error: bool, theme: &Theme) {
    let style = if error { theme.error() } else { theme.text() };
    frame.render_widget(Span::styled(text.to_owned(), style), area);
}

/// A centered, dimmed placeholder for empty panes.
pub fn draw_placeholder(frame: &mut Frame, area: Rect, lines: &[&str], theme: &Theme) {
    let top = area.height.saturating_sub(lines.len() as u16) / 2;
    let text: Vec<Line> = std::iter::repeat_n(Line::default(), usize::from(top))
        .chain(lines.iter().map(|l| Line::styled(*l, theme.dim())))
        .collect();
    frame.render_widget(Paragraph::new(text).centered(), area);
}
