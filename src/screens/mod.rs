//! The screens. Each is a self-contained component that renders itself,
//! interprets actions, and reacts to server events.

mod browser;
mod library;
mod lyrics;
mod playlists;
mod queue;
mod tag_editor;
mod visualizer;

use std::sync::Arc;

use crossterm::event::MouseEvent;
use ratatui::{Frame, layout::Rect, text::Line};
use tokio::sync::watch;

pub use browser::BrowserScreen;
pub use library::LibraryScreen;
pub use lyrics::{LyricsScreen, draw_lyrics};
pub use playlists::PlaylistsScreen;
pub use queue::QueueScreen;
pub use tag_editor::TagEditorScreen;
pub use visualizer::VisualizerScreen;

use crate::{
    action::{Action, ScreenId},
    config::{Config, Theme},
    lyrics::LyricsState,
    mpd::{MpdEvent, MpdHandle},
    popups::Popup,
    state::MpdState,
    tags::{TagField, TagWrite},
    visualizer::AudioFrame,
};

/// What a screen or popup may read, plus a queue of effects for the app.
pub struct Ctx<'a> {
    pub config: &'a Config,
    pub mpd: &'a MpdState,
    pub client: &'a MpdHandle,
    pub lyrics: &'a LyricsState,
    pub effects: Vec<Effect>,
}

impl<'a> Ctx<'a> {
    pub fn new(
        config: &'a Config,
        mpd: &'a MpdState,
        client: &'a MpdHandle,
        lyrics: &'a LyricsState,
    ) -> Self {
        Self {
            config,
            mpd,
            client,
            lyrics,
            effects: Vec::new(),
        }
    }

    pub fn theme(&self) -> &'a Theme {
        &self.config.theme
    }

    pub fn notify(&mut self, text: impl Into<String>) {
        self.effects.push(Effect::Notify {
            text: text.into(),
            error: false,
        });
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.effects.push(Effect::Notify {
            text: text.into(),
            error: true,
        });
    }

    pub fn popup(&mut self, popup: impl Popup + 'static) {
        self.effects.push(Effect::Popup(Box::new(popup)));
    }
}

/// Requests from screens and popups that only the app can fulfil.
pub enum Effect {
    Notify { text: String, error: bool },
    Popup(Box<dyn Popup>),
    Message(ScreenId, ScreenMessage),
    WriteTags(Vec<TagWrite>),
    ReloadLyrics,
}

/// Data a popup hands back to a screen.
#[derive(Clone, Debug)]
pub enum ScreenMessage {
    TagValue { field: TagField, value: String },
    FileName(String),
}

pub trait Screen {
    /// The text of the title row.
    fn title(&self, ctx: &Ctx) -> Line<'static>;

    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx);

    /// Returns whether the action was handled.
    fn handle_action(&mut self, action: Action, ctx: &mut Ctx) -> bool;

    /// Called for every server event, whether or not the screen is visible.
    fn on_mpd(&mut self, _event: &MpdEvent, _ctx: &mut Ctx) {}

    /// Called when the screen becomes visible.
    fn on_enter(&mut self, _ctx: &mut Ctx) {}

    fn on_message(&mut self, _message: ScreenMessage, _ctx: &mut Ctx) {}

    fn on_mouse(&mut self, _event: MouseEvent, _ctx: &mut Ctx) {}

    /// Searches the focused list. Returns `false` if there is nothing to
    /// search on this screen.
    fn search(&mut self, _query: &str, _ctx: &Ctx) -> bool {
        false
    }
}

/// All screens, owned by the app.
pub struct Screens {
    pub queue: QueueScreen,
    pub browser: BrowserScreen,
    pub playlists: PlaylistsScreen,
    pub library: LibraryScreen,
    pub tag_editor: TagEditorScreen,
    pub visualizer: VisualizerScreen,
    pub lyrics: LyricsScreen,
}

impl Screens {
    pub fn new(config: &Config, spectrum: watch::Receiver<Arc<AudioFrame>>) -> Self {
        Self {
            queue: QueueScreen::new(config),
            browser: BrowserScreen::default(),
            playlists: PlaylistsScreen::default(),
            library: LibraryScreen::default(),
            tag_editor: TagEditorScreen::default(),
            visualizer: VisualizerScreen::new(config, spectrum),
            lyrics: LyricsScreen::default(),
        }
    }

    pub fn get_mut(&mut self, id: ScreenId) -> &mut dyn Screen {
        match id {
            ScreenId::Queue => &mut self.queue,
            ScreenId::Browser => &mut self.browser,
            ScreenId::Playlists => &mut self.playlists,
            ScreenId::Library => &mut self.library,
            ScreenId::TagEditor => &mut self.tag_editor,
            ScreenId::Visualizer => &mut self.visualizer,
            ScreenId::Lyrics => &mut self.lyrics,
        }
    }

    pub fn for_each(&mut self, mut f: impl FnMut(&mut dyn Screen)) {
        for &id in ScreenId::ALL {
            f(self.get_mut(id));
        }
    }
}
