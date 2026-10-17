//! Screen 7: lyrics of the current song. Also drawn as a pane on the queue.

use crossterm::event::{MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{Ctx, Effect, Screen};
use crate::{
    action::Action,
    lyrics::{Lyrics, LyricsStatus},
    ui,
};

#[derive(Debug, Default)]
pub struct LyricsScreen {
    /// Scroll offset for plain (unsynced) lyrics.
    scroll: usize,
    height: usize,
    file: Option<String>,
}

/// Draws the lyrics pane without manual scrolling.
pub fn draw_lyrics(frame: &mut Frame, area: Rect, ctx: &Ctx, focused: bool) {
    render(frame, area, ctx, focused, 0);
}

fn render(frame: &mut Frame, area: Rect, ctx: &Ctx, focused: bool, scroll: usize) -> usize {
    let theme = ctx.theme();
    let title = match &ctx.mpd.current {
        Some(song) => format!(" Lyrics · {} ", ui::song_label(song)),
        None => " Lyrics ".to_owned(),
    };
    let block = ui::pane(title, focused, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let height = usize::from(inner.height);

    let lyrics = match (&ctx.mpd.current, &ctx.lyrics.status) {
        (None, _) => {
            ui::draw_placeholder(frame, inner, &["Nothing is playing"], theme);
            return height;
        }
        (Some(_), LyricsStatus::Found(lyrics)) => lyrics,
        (Some(_), LyricsStatus::Idle | LyricsStatus::Loading) => {
            ui::draw_placeholder(frame, inner, &["Looking for lyrics…"], theme);
            return height;
        }
        (Some(_), LyricsStatus::NotFound) => {
            ui::draw_placeholder(frame, inner, &["No lyrics found"], theme);
            return height;
        }
        (Some(_), LyricsStatus::Failed(error)) => {
            ui::draw_placeholder(frame, inner, &["Could not fetch lyrics", error], theme);
            return height;
        }
    };

    let current = lyrics.current_line(ctx.mpd.elapsed());
    let top = match (lyrics, current) {
        // Keep the current line in the middle.
        (Lyrics::Synced(_), Some(line)) => line.saturating_sub(height / 2),
        (Lyrics::Synced(_), None) => 0,
        (Lyrics::Plain(_), _) => scroll.min(lyrics.len().saturating_sub(1)),
    };
    let lines: Vec<Line> = (top..lyrics.len().min(top + height))
        .map(|i| {
            let text = lyrics.line(i);
            let style = match (lyrics, current) {
                (Lyrics::Synced(_), Some(c)) if i == c => {
                    theme.playing().add_modifier(Modifier::BOLD)
                }
                (Lyrics::Synced(_), Some(c)) if i < c => theme.dim(),
                (Lyrics::Synced(_), None) => theme.dim(),
                _ => theme.text(),
            };
            Line::from(Span::styled(text.to_owned(), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
    height
}

impl Screen for LyricsScreen {
    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        // Plain lyrics start from the top for every song.
        let file = ctx.mpd.current.as_ref().map(|s| s.file.clone());
        if file != self.file {
            self.file = file;
            self.scroll = 0;
        }
        self.height = render(frame, area, ctx, true, self.scroll);
    }

    fn handle_action(&mut self, action: Action, ctx: &mut Ctx) -> bool {
        let len = ctx.lyrics.lyrics().map_or(0, Lyrics::len);
        let page = self.height.max(1);
        let max = len.saturating_sub(1);
        self.scroll = match action {
            Action::Up => self.scroll.saturating_sub(1),
            Action::Down => (self.scroll + 1).min(max),
            Action::PageUp => self.scroll.saturating_sub(page),
            Action::PageDown => (self.scroll + page).min(max),
            Action::HalfPageUp => self.scroll.saturating_sub(page / 2),
            Action::HalfPageDown => (self.scroll + page / 2).min(max),
            Action::Top => 0,
            Action::Bottom => max,
            Action::ReloadLyrics => {
                ctx.effects.push(Effect::ReloadLyrics);
                ctx.notify("Fetching lyrics again…");
                return true;
            }
            _ => return false,
        };
        true
    }

    fn on_mouse(&mut self, event: MouseEvent, ctx: &mut Ctx) {
        let action = match event.kind {
            MouseEventKind::ScrollDown => Action::Down,
            MouseEventKind::ScrollUp => Action::Up,
            _ => return,
        };
        for _ in 0..3 {
            self.handle_action(action, ctx);
        }
    }
}
