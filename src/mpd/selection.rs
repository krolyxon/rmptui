use super::{Command, commands};

/// A set of songs chosen in the UI, to be added to the queue or a playlist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Songs or directories (added recursively).
    Uris(Vec<String>),
    /// Every song matching a filter expression.
    Filter(String),
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Uris(uris) => uris.is_empty(),
            Self::Filter(_) => false,
        }
    }

    pub fn add_to_queue(&self) -> Vec<Command> {
        match self {
            Self::Uris(uris) => uris.iter().map(|uri| commands::add(uri)).collect(),
            Self::Filter(filter) => vec![commands::find_add(filter)],
        }
    }

    pub fn add_to_playlist(&self, playlist: &str) -> Vec<Command> {
        match self {
            Self::Uris(uris) => uris
                .iter()
                .map(|uri| commands::playlist_add(playlist, uri))
                .collect(),
            Self::Filter(filter) => vec![commands::playlist_add_filter(playlist, filter)],
        }
    }
}
