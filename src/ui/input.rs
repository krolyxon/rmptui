//! A single-line text editor.

use ratatui::{
    Frame,
    layout::{Position, Rect},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::{action::Action, config::Theme};

#[derive(Clone, Debug, Default)]
pub struct LineInput {
    text: String,
    /// Cursor position as a byte offset on a char boundary.
    cursor: usize,
}

/// What an edit did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Changed,
    Moved,
    Submit,
    Cancel,
    Ignored,
}

impl LineInput {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            cursor: text.len(),
            text,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn insert(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    fn previous_boundary(&self) -> usize {
        self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next_boundary(&self) -> usize {
        self.text[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |c| self.cursor + c.len_utf8())
    }

    /// Applies an input action.
    pub fn apply(&mut self, action: Action) -> InputEvent {
        match action {
            Action::Submit => InputEvent::Submit,
            Action::Cancel => InputEvent::Cancel,
            Action::Backspace if self.cursor > 0 => {
                let start = self.previous_boundary();
                self.text.drain(start..self.cursor);
                self.cursor = start;
                InputEvent::Changed
            }
            Action::DeleteChar if self.cursor < self.text.len() => {
                let end = self.next_boundary();
                self.text.drain(self.cursor..end);
                InputEvent::Changed
            }
            Action::DeleteWord if self.cursor > 0 => {
                let before = &self.text[..self.cursor];
                let trimmed = before.trim_end();
                let start = trimmed.rfind(char::is_whitespace).map_or(0, |i| {
                    i + trimmed[i..].chars().next().map_or(1, char::len_utf8)
                });
                self.text.drain(start..self.cursor);
                self.cursor = start;
                InputEvent::Changed
            }
            Action::CursorLeft => {
                self.cursor = self.previous_boundary();
                InputEvent::Moved
            }
            Action::CursorRight => {
                self.cursor = self.next_boundary();
                InputEvent::Moved
            }
            Action::CursorHome => {
                self.cursor = 0;
                InputEvent::Moved
            }
            Action::CursorEnd => {
                self.cursor = self.text.len();
                InputEvent::Moved
            }
            _ => InputEvent::Ignored,
        }
    }

    /// Draws `prefix` followed by the text and places the terminal cursor.
    pub fn render(&self, frame: &mut Frame, area: Rect, prefix: &str, theme: &Theme) {
        let prefix_width = prefix.width();
        let available = usize::from(area.width).saturating_sub(prefix_width + 1);
        let before_cursor = self.text[..self.cursor].width();
        // Scroll horizontally so that the cursor stays visible.
        let skip = before_cursor.saturating_sub(available);
        let mut visible = String::new();
        let mut width = 0;
        for ch in self.text.chars() {
            let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if width >= skip {
                visible.push(ch);
            }
            width += w;
        }
        let line = Line::from(vec![
            Span::styled(prefix.to_owned(), theme.accent()),
            Span::styled(visible, theme.text()),
        ]);
        frame.render_widget(line, area);
        let x = area.x + (prefix_width + before_cursor - skip) as u16;
        frame.set_cursor_position(Position::new(x.min(area.right().saturating_sub(1)), area.y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_unicode_text() {
        let mut input = LineInput::new("héllo");
        assert_eq!(input.apply(Action::Backspace), InputEvent::Changed);
        assert_eq!(input.text(), "héll");
        input.apply(Action::CursorHome);
        input.apply(Action::CursorRight);
        input.apply(Action::DeleteChar);
        assert_eq!(input.text(), "hll");
        input.insert('é');
        assert_eq!(input.text(), "héll");
        assert_eq!(input.apply(Action::Submit), InputEvent::Submit);
    }

    #[test]
    fn deletes_words() {
        let mut input = LineInput::new("one two  ");
        input.apply(Action::DeleteWord);
        assert_eq!(input.text(), "one ");
        input.apply(Action::DeleteWord);
        assert_eq!(input.text(), "");
        assert_eq!(input.apply(Action::Backspace), InputEvent::Ignored);
    }
}
