//! A virtualized, searchable list used by every screen.

use std::time::{Duration, Instant};

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use nucleo_matcher::{
    Config as MatcherConfig, Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};
use ratatui::{
    Frame,
    layout::{Constraint, Position, Rect},
    style::{Modifier, Style},
    widgets::{Block, Row, Table, TableState},
};

use crate::{action::Action, config::Theme};

/// Cursor, scroll offset and search state of a list.
///
/// Only the visible rows are built when drawing, so lists with tens of
/// thousands of items stay cheap.
#[derive(Debug, Default)]
pub struct ListView {
    selected: usize,
    offset: usize,
    /// Rows visible in the last draw.
    height: usize,
    /// Where the rows were drawn last, for mouse hit-testing.
    rows_area: Rect,
    last_click: Option<(usize, Instant)>,
    pub search: Search,
}

/// What a mouse event on a list did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// The cursor moved (click or wheel).
    Moved,
    /// A row was double-clicked.
    Activate(usize),
}

impl ListView {
    pub fn selected(&self, len: usize) -> Option<usize> {
        (len > 0).then(|| self.selected.min(len - 1))
    }

    pub fn select(&mut self, index: usize, len: usize) {
        self.selected = index.min(len.saturating_sub(1));
    }

    /// Resets cursor, scroll and search, e.g. when the content is replaced.
    pub fn reset(&mut self) {
        self.selected = 0;
        self.offset = 0;
        self.search.clear();
    }

    /// Handles movement actions. Returns whether the action was one.
    pub fn navigate(&mut self, action: Action, len: usize) -> bool {
        let page = self.height.max(1);
        let current = self.selected.min(len.saturating_sub(1));
        let target = match action {
            Action::Up => current.saturating_sub(1),
            Action::Down => current + 1,
            Action::Top => 0,
            Action::Bottom => len.saturating_sub(1),
            Action::PageUp => current.saturating_sub(page),
            Action::PageDown => current + page,
            Action::HalfPageUp => current.saturating_sub(page / 2),
            Action::HalfPageDown => current + page / 2,
            Action::SearchNext => match self.search.next_after(current) {
                Some(index) => index,
                None => return true,
            },
            Action::SearchPrevious => match self.search.previous_before(current) {
                Some(index) => index,
                None => return true,
            },
            _ => return false,
        };
        self.select(target, len);
        true
    }

    /// Scrolls by `delta` rows (mouse wheel), moving the cursor along.
    pub fn scroll(&mut self, delta: isize, len: usize) {
        let target = self.selected.saturating_add_signed(delta);
        self.select(target, len);
    }

    /// Whether a position lies within the rows drawn last.
    pub fn contains(&self, position: Position) -> bool {
        self.rows_area.contains(position)
    }

    /// Handles wheel scrolling and (double) clicks inside the list.
    pub fn mouse(&mut self, event: MouseEvent, len: usize) -> Option<Click> {
        let position = Position::new(event.column, event.row);
        match event.kind {
            MouseEventKind::ScrollDown => self.scroll(3, len),
            MouseEventKind::ScrollUp => self.scroll(-3, len),
            MouseEventKind::Down(MouseButton::Left) => {
                let index = self.index_at(position, len)?;
                self.select(index, len);
                let double = self.last_click.is_some_and(|(last, at)| {
                    last == index && at.elapsed() < Duration::from_millis(400)
                });
                if double {
                    self.last_click = None;
                    return Some(Click::Activate(index));
                }
                self.last_click = Some((index, Instant::now()));
            }
            _ => return None,
        }
        Some(Click::Moved)
    }

    /// The item under a terminal position from the last draw.
    pub fn index_at(&self, position: Position, len: usize) -> Option<usize> {
        if !self.rows_area.contains(position) {
            return None;
        }
        let index = self.offset + usize::from(position.y - self.rows_area.y);
        (index < len).then_some(index)
    }

    /// Recomputes search matches and jumps to the first one at or after the
    /// cursor.
    pub fn search<'a>(&mut self, query: &str, items: impl Iterator<Item = &'a str>, len: usize) {
        self.search.update(query, items);
        let current = self.selected.min(len.saturating_sub(1));
        if let Some(index) = self.search.first_from(current) {
            self.select(index, len);
        }
    }

    /// Draws rows `offset..offset + height` inside `block`.
    #[allow(clippy::too_many_arguments)]
    pub fn render<'a>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        block: Block<'a>,
        header: Option<Row<'a>>,
        widths: &[Constraint],
        len: usize,
        focused: bool,
        theme: &Theme,
        mut row: impl FnMut(usize) -> Row<'a>,
    ) {
        let inner = block.inner(area);
        let header_height = u16::from(header.is_some());
        self.rows_area = Rect {
            y: inner.y + header_height,
            height: inner.height.saturating_sub(header_height),
            ..inner
        };
        self.height = usize::from(self.rows_area.height);
        self.selected = self.selected.min(len.saturating_sub(1));
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.height > 0 && self.selected >= self.offset + self.height {
            self.offset = self.selected + 1 - self.height;
        }
        self.offset = self.offset.min(len.saturating_sub(self.height));

        let end = (self.offset + self.height).min(len);
        let rows: Vec<Row> = (self.offset..end)
            .map(|index| {
                let r = row(index);
                if self.search.is_match(index) {
                    r.style(Style::new().add_modifier(Modifier::UNDERLINED))
                } else {
                    r
                }
            })
            .collect();
        let highlight = if focused {
            theme.selection()
        } else {
            theme.selection_inactive()
        };
        let mut table = Table::new(rows, widths.to_vec())
            .block(block)
            .column_spacing(1)
            .row_highlight_style(highlight);
        if let Some(header) = header {
            table = table.header(header);
        }
        let mut state = TableState::default();
        if len > 0 {
            state.select(Some(self.selected - self.offset));
        }
        frame.render_stateful_widget(table, area, &mut state);
    }
}

/// Fuzzy search results over a list, kept in list order so that `n`/`N`
/// walk through them like in a pager.
#[derive(Debug, Default)]
pub struct Search {
    query: String,
    matches: Vec<usize>,
}

impl Search {
    pub fn update<'a>(&mut self, query: &str, items: impl Iterator<Item = &'a str>) {
        self.query = query.to_owned();
        self.matches.clear();
        if query.trim().is_empty() {
            return;
        }
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut matcher = Matcher::new(MatcherConfig::DEFAULT);
        let mut buf = Vec::new();
        self.matches
            .extend(items.enumerate().filter_map(|(index, item)| {
                pattern
                    .score(Utf32Str::new(item, &mut buf), &mut matcher)
                    .map(|_| index)
            }));
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.matches.clear();
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    pub fn is_match(&self, index: usize) -> bool {
        self.matches.binary_search(&index).is_ok()
    }

    /// The first match at or after `index`, wrapping around.
    pub fn first_from(&self, index: usize) -> Option<usize> {
        let start = self.matches.partition_point(|&m| m < index);
        self.matches
            .get(start)
            .or_else(|| self.matches.first())
            .copied()
    }

    /// The first match after `index`, wrapping around.
    pub fn next_after(&self, index: usize) -> Option<usize> {
        self.first_from(index.saturating_add(1))
    }

    /// The last match before `index`, wrapping around.
    pub fn previous_before(&self, index: usize) -> Option<usize> {
        let end = self.matches.partition_point(|&m| m < index);
        end.checked_sub(1)
            .and_then(|i| self.matches.get(i))
            .or_else(|| self.matches.last())
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: [&str; 5] = ["Alpha", "beta", "Gamma", "alphabet", "delta"];

    fn search(query: &str) -> Search {
        let mut search = Search::default();
        search.update(query, ITEMS.into_iter());
        search
    }

    #[test]
    fn finds_fuzzy_matches_in_list_order() {
        let s = search("alp");
        assert_eq!(s.matches, [0, 3]);
        assert!(s.is_match(3) && !s.is_match(1));
        assert_eq!(search("").match_count(), 0);
    }

    #[test]
    fn cycles_matches_with_wraparound() {
        let s = search("alp");
        assert_eq!(s.next_after(0), Some(3));
        assert_eq!(s.next_after(3), Some(0));
        assert_eq!(s.first_from(3), Some(3));
        assert_eq!(s.previous_before(3), Some(0));
        assert_eq!(s.previous_before(0), Some(3));
    }

    #[test]
    fn navigates_and_clamps() {
        let mut view = ListView {
            height: 10,
            ..ListView::default()
        };
        assert!(view.navigate(Action::Bottom, 25));
        assert_eq!(view.selected(25), Some(24));
        view.navigate(Action::PageUp, 25);
        assert_eq!(view.selected(25), Some(14));
        view.navigate(Action::HalfPageDown, 25);
        assert_eq!(view.selected(25), Some(19));
        view.navigate(Action::Down, 25);
        view.navigate(Action::PageDown, 25);
        assert_eq!(view.selected(25), Some(24));
        assert_eq!(view.selected(3), Some(2));
        assert_eq!(view.selected(0), None);
        assert!(!view.navigate(Action::Quit, 25));
    }

    #[test]
    fn search_jumps_to_first_match_from_cursor() {
        let mut view = ListView::default();
        view.select(1, ITEMS.len());
        view.search("alp", ITEMS.into_iter(), ITEMS.len());
        assert_eq!(view.selected(ITEMS.len()), Some(3));
        view.navigate(Action::SearchNext, ITEMS.len());
        assert_eq!(view.selected(ITEMS.len()), Some(0));
    }
}
