//! Events delivered to the application loop by background tasks.

use crate::{lyrics::LookupResult, mpd::MpdEvent};

#[derive(Debug)]
pub enum AppEvent {
    Mpd(MpdEvent),
    Lyrics {
        file: String,
        result: LookupResult,
    },
    /// Tag writes finished: the directories to rescan, or an error.
    TagsWritten(Result<Vec<String>, String>),
}

impl From<MpdEvent> for AppEvent {
    fn from(event: MpdEvent) -> Self {
        Self::Mpd(event)
    }
}
