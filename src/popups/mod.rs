//! Modal popups drawn above the screens. The topmost popup receives all
//! keys until it closes.

mod add_to_playlist;
mod confirm;
mod help;
mod input;

use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    text::Line,
    widgets::{Block, BorderType, Clear},
};

pub use add_to_playlist::AddToPlaylistPopup;
pub use confirm::ConfirmPopup;
pub use help::HelpPopup;
pub use input::{InputPopup, InputPurpose};

use crate::{config::KeyCombo, screens::Ctx};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupState {
    Open,
    Closed,
}

pub trait Popup {
    /// Draws the popup; `area` is the whole terminal.
    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx);

    fn handle_key(&mut self, key: KeyCombo, ctx: &mut Ctx) -> PopupState;
}

/// Clears a centered rectangle and draws a titled frame around it,
/// returning the inner area.
fn frame_popup(
    frame: &mut Frame,
    area: Rect,
    width: u16,
    height: u16,
    title: &str,
    ctx: &Ctx,
) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(ctx.theme().border())
        .title(Line::from(format!(" {title} ")).style(ctx.theme().header()));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    inner
}
