use ratatui::{Frame, layout::Rect};

use super::{Popup, PopupState, frame_popup};
use crate::{
    action::{Context, ScreenId},
    config::KeyCombo,
    mpd::{Selection, commands},
    screens::{Ctx, Effect, ScreenMessage},
    tags::TagField,
    ui::input::{InputEvent, LineInput},
};

/// What the entered text is for.
#[derive(Clone, Debug)]
pub enum InputPurpose {
    NewPlaylist(Selection),
    RenamePlaylist(String),
    SaveQueue,
    TagValue(TagField),
    FileName,
}

pub struct InputPopup {
    title: String,
    input: LineInput,
    purpose: InputPurpose,
}

impl InputPopup {
    pub fn new(title: impl Into<String>, initial: &str, purpose: InputPurpose) -> Self {
        Self {
            title: title.into(),
            input: LineInput::new(initial),
            purpose,
        }
    }

    fn submit(&self, ctx: &mut Ctx) -> PopupState {
        let value = self.input.text().trim().to_owned();
        if value.is_empty() && !matches!(self.purpose, InputPurpose::TagValue(_)) {
            return PopupState::Open;
        }
        match &self.purpose {
            InputPurpose::NewPlaylist(selection) => {
                if ctx.mpd.playlists.iter().any(|p| p.name == value) {
                    ctx.notify(format!("Added to existing playlist “{value}”"));
                } else {
                    ctx.notify(format!("Created playlist “{value}”"));
                }
                ctx.client.exec_all(selection.add_to_playlist(&value));
            }
            InputPurpose::RenamePlaylist(from) => {
                if *from != value {
                    ctx.client.exec(commands::rename(from, &value));
                }
            }
            InputPurpose::SaveQueue => {
                ctx.client.exec(commands::save(&value));
                ctx.notify(format!("Saved the queue as “{value}”"));
            }
            InputPurpose::FileName => ctx.effects.push(Effect::Message(
                ScreenId::TagEditor,
                ScreenMessage::FileName(value),
            )),
            InputPurpose::TagValue(field) => ctx.effects.push(Effect::Message(
                ScreenId::TagEditor,
                ScreenMessage::TagValue {
                    field: *field,
                    value,
                },
            )),
        }
        PopupState::Closed
    }
}

impl Popup for InputPopup {
    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let width = (area.width * 3 / 5).max(40);
        let inner = frame_popup(frame, area, width, 3, &self.title, ctx);
        self.input.render(frame, inner, "", ctx.theme());
    }

    fn handle_key(&mut self, key: KeyCombo, ctx: &mut Ctx) -> PopupState {
        match ctx.config.keymap.resolve(&[Context::Input], key) {
            Some(action) => match self.input.apply(action) {
                InputEvent::Submit => self.submit(ctx),
                InputEvent::Cancel => PopupState::Closed,
                _ => PopupState::Open,
            },
            None => {
                if let Some(ch) = key.as_text() {
                    self.input.insert(ch);
                }
                PopupState::Open
            }
        }
    }
}
