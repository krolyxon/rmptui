//! Every user-triggerable action and the key contexts they are bound in.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, de};

macro_rules! named_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident { $($variant:ident = $str:literal),* $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name { $($variant),* }

        impl $name {
            pub const ALL: &[Self] = &[$(Self::$variant),*];

            pub fn name(self) -> &'static str {
                match self { $(Self::$variant => $str),* }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.name())
            }
        }

        impl FromStr for $name {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::ALL
                    .iter()
                    .copied()
                    .find(|v| v.name() == s)
                    .ok_or_else(|| {
                        let names: Vec<_> = Self::ALL.iter().map(|v| v.name()).collect();
                        format!("unknown value `{s}`, expected one of: {}", names.join(", "))
                    })
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let s = String::deserialize(deserializer)?;
                s.parse().map_err(de::Error::custom)
            }
        }
    };
}

named_enum! {
    /// A set of bindings that is active in a certain part of the UI.
    ///
    /// Lookups go from the most specific context (popup or screen) through
    /// `navigation` to `global`, so specific bindings shadow general ones.
    pub enum Context {
        Global = "global",
        Navigation = "navigation",
        Queue = "queue",
        Browser = "browser",
        Playlists = "playlists",
        Library = "library",
        TagEditor = "tag_editor",
        Visualizer = "visualizer",
        Lyrics = "lyrics",
        Popup = "popup",
        Input = "input",
    }
}

named_enum! {
    pub enum Action {
        // Global
        Quit = "quit",
        Help = "help",
        TogglePause = "toggle_pause",
        Pause = "pause",
        Stop = "stop",
        NextTrack = "next_track",
        PreviousTrack = "previous_track",
        SeekForward = "seek_forward",
        SeekBackward = "seek_backward",
        VolumeUp = "volume_up",
        VolumeDown = "volume_down",
        ToggleMute = "toggle_mute",
        ToggleRepeat = "toggle_repeat",
        ToggleRandom = "toggle_random",
        ToggleSingle = "toggle_single",
        ToggleConsume = "toggle_consume",
        UpdateDatabase = "update_database",
        NextScreen = "next_screen",
        PreviousScreen = "previous_screen",
        ScreenQueue = "screen_queue",
        ScreenBrowser = "screen_browser",
        ScreenPlaylists = "screen_playlists",
        ScreenLibrary = "screen_library",
        ScreenTagEditor = "screen_tag_editor",
        ScreenVisualizer = "screen_visualizer",
        ScreenLyrics = "screen_lyrics",
        // Navigation (lists)
        Up = "up",
        Down = "down",
        Top = "top",
        Bottom = "bottom",
        PageUp = "page_up",
        PageDown = "page_down",
        HalfPageUp = "half_page_up",
        HalfPageDown = "half_page_down",
        Left = "left",
        Right = "right",
        Activate = "activate",
        ToggleMark = "toggle_mark",
        ClearMarks = "clear_marks",
        Search = "search",
        SearchNext = "search_next",
        SearchPrevious = "search_previous",
        AddToQueue = "add_to_queue",
        AddToPlaylist = "add_to_playlist",
        // Screen specific
        Delete = "delete",
        MoveItemUp = "move_item_up",
        MoveItemDown = "move_item_down",
        ClearQueue = "clear_queue",
        ShuffleQueue = "shuffle_queue",
        SaveQueue = "save_queue",
        JumpToCurrent = "jump_to_current",
        ToggleLyricsPane = "toggle_lyrics_pane",
        DeletePlaylist = "delete_playlist",
        RenamePlaylist = "rename_playlist",
        ToggleArtistTag = "toggle_artist_tag",
        SaveTags = "save_tags",
        RevertTags = "revert_tags",
        ReloadLyrics = "reload_lyrics",
        // Popups and text input
        Close = "close",
        Submit = "submit",
        Cancel = "cancel",
        Backspace = "backspace",
        DeleteChar = "delete_char",
        DeleteWord = "delete_word",
        CursorLeft = "cursor_left",
        CursorRight = "cursor_right",
        CursorHome = "cursor_home",
        CursorEnd = "cursor_end",
    }
}

named_enum! {
    /// The screens, in tab order.
    pub enum ScreenId {
        Queue = "queue",
        Browser = "browser",
        Playlists = "playlists",
        Library = "library",
        TagEditor = "tag_editor",
        Visualizer = "visualizer",
        Lyrics = "lyrics",
    }
}

impl ScreenId {
    pub fn title(self) -> &'static str {
        match self {
            Self::Queue => "Queue",
            Self::Browser => "Browse",
            Self::Playlists => "Playlists",
            Self::Library => "Library",
            Self::TagEditor => "Tags",
            Self::Visualizer => "Visualizer",
            Self::Lyrics => "Lyrics",
        }
    }

    /// The key context that is active while the screen is focused.
    pub fn context(self) -> Context {
        match self {
            Self::Queue => Context::Queue,
            Self::Browser => Context::Browser,
            Self::Playlists => Context::Playlists,
            Self::Library => Context::Library,
            Self::TagEditor => Context::TagEditor,
            Self::Visualizer => Context::Visualizer,
            Self::Lyrics => Context::Lyrics,
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

impl Action {
    /// The contexts this action may be bound in.
    pub fn contexts(self) -> &'static [Context] {
        use Action::*;
        use Context as C;
        match self {
            Quit | Help | TogglePause | Pause | Stop | NextTrack | PreviousTrack | SeekForward
            | SeekBackward | VolumeUp | VolumeDown | ToggleMute | ToggleRepeat | ToggleRandom
            | ToggleSingle | ToggleConsume | UpdateDatabase | NextScreen | PreviousScreen
            | ScreenQueue | ScreenBrowser | ScreenPlaylists | ScreenLibrary | ScreenTagEditor
            | ScreenVisualizer | ScreenLyrics => &[C::Global],
            Up | Down | Top | Bottom | PageUp | PageDown | HalfPageUp | HalfPageDown | Left
            | Right | Activate | ToggleMark | ClearMarks | Search | SearchNext | SearchPrevious
            | AddToQueue | AddToPlaylist => &[C::Navigation],
            Delete | MoveItemUp | MoveItemDown => &[C::Queue, C::Playlists],
            ClearQueue | ShuffleQueue | SaveQueue | JumpToCurrent | ToggleLyricsPane => &[C::Queue],
            DeletePlaylist | RenamePlaylist => &[C::Playlists],
            ToggleArtistTag => &[C::Library],
            SaveTags | RevertTags => &[C::TagEditor],
            ReloadLyrics => &[C::Lyrics],
            Close => &[C::Popup],
            Submit | Cancel | Backspace | DeleteChar | DeleteWord | CursorLeft | CursorRight
            | CursorHome | CursorEnd => &[C::Input],
        }
    }

    /// A short human-readable description for the help popup.
    pub fn description(self) -> &'static str {
        use Action::*;
        match self {
            Quit => "Quit rmptui",
            Help => "Show key bindings",
            TogglePause => "Toggle play/pause",
            Pause => "Pause",
            Stop => "Stop playback",
            NextTrack => "Play next track",
            PreviousTrack => "Play previous track",
            SeekForward => "Seek forward",
            SeekBackward => "Seek backward",
            VolumeUp => "Increase volume",
            VolumeDown => "Decrease volume",
            ToggleMute => "Toggle mute",
            ToggleRepeat => "Toggle repeat",
            ToggleRandom => "Toggle random",
            ToggleSingle => "Toggle single",
            ToggleConsume => "Toggle consume",
            UpdateDatabase => "Rescan the music database",
            NextScreen => "Next screen",
            PreviousScreen => "Previous screen",
            ScreenQueue => "Show the queue",
            ScreenBrowser => "Show the song browser",
            ScreenPlaylists => "Show the playlist editor",
            ScreenLibrary => "Show the media library",
            ScreenTagEditor => "Show the tag editor",
            ScreenVisualizer => "Show the visualizer",
            ScreenLyrics => "Show the lyrics",
            Up => "Move up",
            Down => "Move down",
            Top => "Go to top",
            Bottom => "Go to bottom",
            PageUp => "Page up",
            PageDown => "Page down",
            HalfPageUp => "Half page up",
            HalfPageDown => "Half page down",
            Left => "Go back / previous column",
            Right => "Open / next column",
            Activate => "Play or open the selected item",
            ToggleMark => "Mark or unmark the item",
            ClearMarks => "Clear all marks",
            Search => "Search the current list",
            SearchNext => "Next search match",
            SearchPrevious => "Previous search match",
            AddToQueue => "Add to (or toggle in) the queue",
            AddToPlaylist => "Add to a playlist",
            Delete => "Remove the item",
            MoveItemUp => "Move the item up",
            MoveItemDown => "Move the item down",
            ClearQueue => "Clear the queue",
            ShuffleQueue => "Shuffle the queue",
            SaveQueue => "Save the queue as a playlist",
            JumpToCurrent => "Jump to the playing song",
            ToggleLyricsPane => "Toggle the lyrics pane",
            DeletePlaylist => "Delete the playlist",
            RenamePlaylist => "Rename the playlist",
            ToggleArtistTag => "Switch between AlbumArtist and Artist",
            SaveTags => "Write pending tag changes",
            RevertTags => "Discard pending tag changes",
            ReloadLyrics => "Fetch lyrics again",
            Close => "Close the popup",
            Submit => "Submit",
            Cancel => "Cancel",
            Backspace => "Delete previous character",
            DeleteChar => "Delete next character",
            DeleteWord => "Delete previous word",
            CursorLeft => "Cursor left",
            CursorRight => "Cursor right",
            CursorHome => "Cursor to start",
            CursorEnd => "Cursor to end",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for action in Action::ALL {
            assert_eq!(action.name().parse::<Action>().unwrap(), *action);
            assert!(!action.contexts().is_empty());
        }
        for context in Context::ALL {
            assert_eq!(context.name().parse::<Context>().unwrap(), *context);
        }
        assert!("nope".parse::<Action>().is_err());
    }
}
