use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Row},
};

use super::{InputPopup, InputPurpose, Popup, PopupState, frame_popup};
use crate::{
    action::{Action, Context},
    config::KeyCombo,
    mpd::Selection,
    screens::Ctx,
    ui::list::ListView,
};

enum Choice {
    Queue,
    NewPlaylist,
    Playlist(String),
}

/// Chooses where to add a selection: the queue, a new or an existing playlist.
pub struct AddToPlaylistPopup {
    selection: Selection,
    choices: Vec<Choice>,
    list: ListView,
}

impl AddToPlaylistPopup {
    pub fn new(selection: Selection, ctx: &Ctx) -> Self {
        let mut choices = vec![Choice::Queue, Choice::NewPlaylist];
        choices.extend(
            ctx.mpd
                .playlists
                .iter()
                .map(|p| Choice::Playlist(p.name.clone())),
        );
        Self {
            selection,
            choices,
            list: ListView::default(),
        }
    }

    fn choose(&mut self, ctx: &mut Ctx) -> PopupState {
        let Some(choice) = self
            .list
            .selected(self.choices.len())
            .and_then(|i| self.choices.get(i))
        else {
            return PopupState::Open;
        };
        match choice {
            Choice::Queue => {
                ctx.client.exec_all(self.selection.add_to_queue());
                ctx.notify("Added to the queue");
            }
            Choice::NewPlaylist => {
                ctx.popup(InputPopup::new(
                    "New playlist",
                    "",
                    InputPurpose::NewPlaylist(self.selection.clone()),
                ));
            }
            Choice::Playlist(name) => {
                ctx.client.exec_all(self.selection.add_to_playlist(name));
                ctx.notify(format!("Added to playlist “{name}”"));
            }
        }
        PopupState::Closed
    }
}

impl Popup for AddToPlaylistPopup {
    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let height = self.choices.len().min(15) as u16 + 2;
        let inner = frame_popup(frame, area, 44, height, "Add to", ctx);
        let theme = ctx.theme();
        let choices = &self.choices;
        self.list.render(
            frame,
            inner,
            Block::new(),
            None,
            &[Constraint::Fill(1)],
            choices.len(),
            true,
            theme,
            |i| {
                Row::new([match &choices[i] {
                    Choice::Queue => Span::styled("Current queue", theme.accent()),
                    Choice::NewPlaylist => Span::styled("New playlist…", theme.accent()),
                    Choice::Playlist(name) => Span::styled(name.clone(), theme.text()),
                }])
            },
        );
    }

    fn handle_key(&mut self, key: KeyCombo, ctx: &mut Ctx) -> PopupState {
        match ctx
            .config
            .keymap
            .resolve(&[Context::Popup, Context::Navigation], key)
        {
            Some(Action::Close) => PopupState::Closed,
            Some(Action::Activate | Action::Right) => self.choose(ctx),
            Some(action) => {
                self.list.navigate(action, self.choices.len());
                PopupState::Open
            }
            None => PopupState::Open,
        }
    }
}
