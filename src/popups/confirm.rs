use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use super::{Popup, PopupState, frame_popup};
use crate::{
    action::{Action, Context},
    config::KeyCombo,
    mpd::Command,
    screens::Ctx,
};

/// Asks before running destructive commands.
pub struct ConfirmPopup {
    message: String,
    commands: Vec<Command>,
}

impl ConfirmPopup {
    pub fn new(message: impl Into<String>, commands: Vec<Command>) -> Self {
        Self {
            message: message.into(),
            commands,
        }
    }
}

impl Popup for ConfirmPopup {
    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let inner = frame_popup(frame, area, 50, 5, "Confirm", ctx);
        let theme = ctx.theme();
        let text = vec![
            Line::from(self.message.as_str()),
            Line::default(),
            Line::from(vec![
                Span::styled("[y]", theme.header()),
                Span::raw("es  "),
                Span::styled("[n]", theme.header()),
                Span::raw("o"),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            inner,
        );
    }

    fn handle_key(&mut self, key: KeyCombo, ctx: &mut Ctx) -> PopupState {
        let confirmed = matches!(key.code, KeyCode::Char('y' | 'Y'))
            || ctx.config.keymap.resolve(&[Context::Navigation], key) == Some(Action::Activate);
        if confirmed {
            ctx.client.exec_all(std::mem::take(&mut self.commands));
            return PopupState::Closed;
        }
        let declined = matches!(key.code, KeyCode::Char('n' | 'N'))
            || ctx.config.keymap.resolve(&[Context::Popup], key) == Some(Action::Close);
        if declined {
            PopupState::Closed
        } else {
            PopupState::Open
        }
    }
}
