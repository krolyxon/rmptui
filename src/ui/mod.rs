//! Widgets and the parts of the frame shared by all screens.

pub mod input;
pub mod list;

use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    action::ScreenId,
    config::{ColumnField, Theme},
    mpd::{PlayState, Song},
    state::MpdState,
    util::format_duration,
};

/// A bordered pane; focused panes get the accent color.
pub fn pane<'a>(title: impl Into<Line<'a>>, focused: bool, theme: &Theme) -> Block<'a> {
    let style = if focused {
        theme.accent()
    } else {
        theme.border()
    };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(style)
        .title(title.into().style(style))
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
    text.unwrap_or_default().to_owned()
}

/// `Artist - Title`, or just the title.
pub fn song_label(song: &Song) -> String {
    match song.artist() {
        Some(artist) => format!("{artist} - {}", song.display_title()),
        None => song.display_title().to_owned(),
    }
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

/// The rows of the frame around the active screen.
pub struct FrameLayout {
    pub header: Rect,
    pub tabs: Rect,
    pub body: Rect,
    pub progress: Rect,
    pub status: Rect,
}

impl FrameLayout {
    pub fn new(area: Rect) -> Self {
        let [header, tabs, body, progress, status] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);
        Self {
            header,
            tabs,
            body,
            progress,
            status,
        }
    }
}

pub fn draw_header(frame: &mut Frame, area: Rect, mpd: &MpdState, theme: &Theme) {
    let [top, bottom] = Layout::vertical([Constraint::Length(1); 2]).areas(area);
    let status = &mpd.status;

    let (icon, title, details) = match (&mpd.current, mpd.connected) {
        (_, false) => ("✕", "Not connected".to_owned(), String::new()),
        (Some(song), true) => {
            let icon = match status.state {
                PlayState::Play => "▶",
                PlayState::Pause => "⏸",
                PlayState::Stop => "■",
            };
            let mut details = song.artist().unwrap_or("Unknown artist").to_owned();
            if let Some(album) = song.album() {
                details.push_str(" — ");
                details.push_str(album);
                if let Some(date) = song.tag("Date") {
                    details.push_str(&format!(" ({date})"));
                }
            }
            (icon, song.display_title().to_owned(), details)
        }
        (None, true) => ("■", "Stopped".to_owned(), String::new()),
    };

    let flag = |on: bool, letter: &'static str| {
        if on {
            Span::styled(letter, theme.accent().add_modifier(Modifier::BOLD))
        } else {
            Span::styled(letter, theme.dim())
        }
    };
    let volume = status
        .volume
        .map_or_else(|| "vol n/a".to_owned(), |v| format!("vol {v:>3}%"));
    let right = Line::from(vec![
        Span::styled(volume, theme.text()),
        Span::styled("  [", theme.dim()),
        flag(status.repeat, "r"),
        flag(status.random, "z"),
        flag(status.single.is_on(), "s"),
        flag(status.consume.is_on(), "c"),
        Span::styled("]", theme.dim()),
    ]);
    let mut audio = Vec::new();
    if status.updating_db {
        audio.push("updating database".to_owned());
    }
    if let Some(bitrate) = status.bitrate {
        audio.push(format!("{bitrate} kbps"));
    }
    if let Some(format) = status.audio.as_deref() {
        audio.push(format_audio(format));
    }
    let right_width = right.width() as u16;

    let [title_area, flags_area] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(right_width + 1)]).areas(top);
    frame.render_widget(
        Line::from(vec![
            Span::styled(format!(" {icon} "), theme.accent()),
            Span::styled(title, theme.text().add_modifier(Modifier::BOLD)),
        ]),
        title_area,
    );
    frame.render_widget(right.alignment(Alignment::Right), flags_area);

    let audio = audio.join(" · ");
    let [details_area, audio_area] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(audio.width() as u16 + 1),
    ])
    .areas(bottom);
    frame.render_widget(
        Line::from(Span::styled(format!("   {details}"), theme.dim())),
        details_area,
    );
    frame.render_widget(
        Line::from(Span::styled(audio, theme.dim())).alignment(Alignment::Right),
        audio_area,
    );
}

/// `44100:16:2` → `44.1 kHz · 16 bit · stereo`.
fn format_audio(format: &str) -> String {
    let mut parts = format.split(':');
    let (Some(rate), Some(bits), Some(channels)) = (parts.next(), parts.next(), parts.next())
    else {
        return format.to_owned();
    };
    let rate = rate
        .parse::<f64>()
        .map_or_else(|_| rate.to_owned(), |r| format!("{} kHz", r / 1000.0));
    let bits = match bits {
        "f" => "float".to_owned(),
        "dsd" => "DSD".to_owned(),
        bits => format!("{bits} bit"),
    };
    let channels = match channels {
        "1" => "mono".to_owned(),
        "2" => "stereo".to_owned(),
        n => format!("{n} ch"),
    };
    format!("{rate} · {bits} · {channels}")
}

/// Draws the tab bar and returns each tab's horizontal span for mouse hits.
pub fn draw_tabs(
    frame: &mut Frame,
    area: Rect,
    active: ScreenId,
    theme: &Theme,
) -> Vec<(u16, u16, ScreenId)> {
    let mut spans = vec![Span::raw(" ")];
    let mut hits = Vec::new();
    let mut x = area.x + 1;
    for (i, &screen) in ScreenId::ALL.iter().enumerate() {
        let label = format!(" {} {} ", i + 1, screen.title());
        let width = label.width() as u16;
        let style = if screen == active {
            theme.selection().add_modifier(Modifier::BOLD)
        } else {
            theme.dim()
        };
        hits.push((x, x + width, screen));
        x += width + 1;
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    frame.render_widget(Line::from(spans), area);
    hits
}

/// Draws `elapsed ━━━━──── total`.
pub fn draw_progress(frame: &mut Frame, area: Rect, mpd: &MpdState, theme: &Theme) {
    let elapsed = mpd.elapsed();
    let total = mpd.status.duration.unwrap_or_default();
    let left = format!(" {} ", format_duration(elapsed));
    let right = format!(" {} ", format_duration(total));
    let [left_area, bar, right_area] = progress_layout(area, &left, &right);
    frame.render_widget(Span::styled(left, theme.dim()), left_area);
    frame.render_widget(Span::styled(right, theme.dim()), right_area);

    let width = usize::from(bar.width);
    let filled = if total.is_zero() || mpd.status.state == PlayState::Stop {
        0
    } else {
        ((elapsed.as_secs_f64() / total.as_secs_f64()) * width as f64).round() as usize
    }
    .min(width);
    frame.render_widget(
        Line::from(vec![
            Span::styled("━".repeat(filled), theme.accent()),
            Span::styled("─".repeat(width - filled), theme.border()),
        ]),
        bar,
    );
}

fn progress_layout(area: Rect, left: &str, right: &str) -> [Rect; 3] {
    Layout::horizontal([
        Constraint::Length(left.width() as u16),
        Constraint::Fill(1),
        Constraint::Length(right.width() as u16),
    ])
    .areas(area)
}

/// Maps a click on the progress bar to a position in the song.
pub fn progress_seek_target(area: Rect, x: u16, mpd: &MpdState) -> Option<Duration> {
    let total = mpd.status.duration?;
    let left = format!(" {} ", format_duration(mpd.elapsed()));
    let right = format!(" {} ", format_duration(total));
    let [_, bar, _] = progress_layout(area, &left, &right);
    if bar.width == 0 || x < bar.x || x >= bar.right() {
        return None;
    }
    let fraction = f64::from(x - bar.x) / f64::from(bar.width);
    Some(total.mul_f64(fraction))
}

/// A one-line message, styled as an error if needed.
pub fn draw_message(frame: &mut Frame, area: Rect, text: &str, error: bool, theme: &Theme) {
    let style = if error { theme.error() } else { theme.text() };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(format!(" {text}"), style))),
        area,
    );
}

/// Key hints shown in the status line when nothing else is.
pub fn draw_hint(frame: &mut Frame, area: Rect, text: &str, theme: &Theme) {
    frame.render_widget(
        Line::from(Span::styled(format!(" {text}"), theme.dim())).alignment(Alignment::Left),
        area,
    );
}

/// A centered, dimmed placeholder for empty panes.
pub fn draw_placeholder(frame: &mut Frame, area: Rect, lines: &[&str], theme: &Theme) {
    let top = area.height.saturating_sub(lines.len() as u16) / 2;
    let text: Vec<Line> = std::iter::repeat_n(Line::default(), usize::from(top))
        .chain(
            lines
                .iter()
                .map(|l| Line::from(Span::styled(*l, theme.dim()))),
        )
        .collect();
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::new()),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_audio() {
        assert_eq!(format_audio("44100:16:2"), "44.1 kHz · 16 bit · stereo");
        assert_eq!(format_audio("96000:f:1"), "96 kHz · float · mono");
        assert_eq!(format_audio("weird"), "weird");
    }
}
