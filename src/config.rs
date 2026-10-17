use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::PathBuf,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

use crate::AppResult;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub mpd: MpdConfig,
    pub playback: PlaybackConfig,
    pub keybindings: Keybindings,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct MpdConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub music_directory: String,
}

impl Default for MpdConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6600,
            password: String::new(),
            music_directory: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct PlaybackConfig {
    pub seek_seconds: u64,
    pub volume_step: u8,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            seek_seconds: 2,
            volume_step: 2,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Keybindings {
    #[serde(flatten)]
    bindings: BTreeMap<String, Vec<String>>,
}

impl Default for Keybindings {
    fn default() -> Self {
        let defaults: &[(&str, &[&str])] = &[
            ("quit", &["q", "Ctrl+c"]),
            ("clear_queue", &["c", "C"]),
            ("toggle_pause", &["p"]),
            ("pause", &["s"]),
            ("toggle_repeat", &["r"]),
            ("toggle_random", &["z"]),
            ("open_playlist_menu", &["a"]),
            ("seek_forward", &["f"]),
            ("seek_backward", &["b"]),
            ("cycle_tabs", &["Tab"]),
            ("select_queue", &["1"]),
            ("select_browser", &["2"]),
            ("select_playlists", &["3"]),
            ("select_tags", &["4"]),
            ("next_track", &[">"]),
            ("previous_track", &["<"]),
            ("volume_up", &["=", "+"]),
            ("volume_down", &["-"]),
            ("toggle_mute", &["m"]),
            ("update_database", &["U"]),
            ("search", &["/"]),
            ("toggle_selected_item", &["Space"]),
            ("search_next", &["n"]),
            ("search_previous", &["N"]),
            ("move_down", &["j", "Down"]),
            ("move_up", &["k", "Up"]),
            ("activate", &["Enter", "l", "Right"]),
            ("go_back", &["h", "Left"]),
            ("go_to_top", &["g"]),
            ("go_to_bottom", &["G"]),
            ("queue_delete", &["d"]),
            ("queue_move_down", &["J"]),
            ("queue_move_up", &["K"]),
            ("playlist_rename", &["R"]),
            ("playlist_focus_contents", &["Right", "l"]),
            ("playlist_focus_list", &["Left", "h"]),
            ("playlist_delete_song", &["d"]),
            ("playlist_move_song_down", &["J"]),
            ("playlist_move_song_up", &["K"]),
            ("playlist_delete", &["D"]),
            ("playlist_load", &["Space", "Enter"]),
            ("popup_close", &["q", "Esc"]),
            ("popup_select", &["Enter"]),
            ("input_cancel", &["Esc"]),
            ("input_backspace", &["Backspace"]),
            ("input_cursor_left", &["Left"]),
            ("input_cursor_right", &["Right"]),
            ("input_submit", &["Enter"]),
            ("open_in_tag_editor", &["T"]),
            ("tag_edit", &["Enter", "l"]),
            ("tag_save", &["s"]),
            ("tag_reset", &["r"]),
            ("tag_transform_capitalize", &["c"]),
            ("tag_transform_lower", &["L"]),
            ("tag_transform_upper", &["U"]),
            ("tag_transform_trim", &["t"]),
        ];
        Self {
            bindings: defaults
                .iter()
                .map(|(action, keys)| {
                    (
                        (*action).to_string(),
                        keys.iter().map(|key| (*key).to_string()).collect(),
                    )
                })
                .collect(),
        }
    }
}

impl Keybindings {
    pub fn matches(&self, action: &str, key: KeyEvent) -> bool {
        self.bindings.get(action).is_some_and(|keys| {
            keys.iter()
                .any(|spec| parse_key_spec(spec).is_ok_and(|binding| key_matches(binding, key)))
        })
    }

    fn validate(&self) -> AppResult<()> {
        let defaults = Self::default();
        for (action, keys) in &self.bindings {
            if !defaults.bindings.contains_key(action) {
                return Err(invalid_config(&format!(
                    "keybindings contains unknown action `{action}`"
                )));
            }
            for key in keys {
                parse_key_spec(key).map_err(|error| {
                    invalid_config(&format!(
                        "invalid keybinding `{key}` for `{action}`: {error}"
                    ))
                })?;
            }
        }
        Ok(())
    }

    fn apply_missing_defaults(&mut self) {
        let defaults = Self::default().bindings;
        for (action, keys) in &defaults {
            if self
                .bindings
                .get(action)
                .is_some_and(|current| current == &vec!["Right".to_string()])
                && action == "playlist_focus_contents"
            {
                self.bindings.insert(action.clone(), keys.clone());
            }
            if self
                .bindings
                .get(action)
                .is_some_and(|current| current == &vec!["Left".to_string()])
                && action == "playlist_focus_list"
            {
                self.bindings.insert(action.clone(), keys.clone());
            }
        }
        for (action, keys) in defaults {
            self.bindings.entry(action).or_insert(keys);
        }
    }
}

impl Config {
    pub fn load_or_create() -> AppResult<(Self, PathBuf)> {
        let path = config_path()?;
        let config = Self::load_or_create_at(&path)?;
        Ok((config, path))
    }

    fn load_or_create_at(path: &std::path::Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let default_contents = default_config_text()?;
                match create_config_file(path, default_contents.as_bytes()) {
                    Ok(()) => default_contents,
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        fs::read_to_string(path)?
                    }
                    Err(error) => return Err(Box::new(error)),
                }
            }
            Err(error) => return Err(Box::new(error)),
        };

        let mut config: Self = toml::from_str(&contents)?;
        config.keybindings.apply_missing_defaults();
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> AppResult<()> {
        if self.mpd.host.trim().is_empty() {
            return Err(invalid_config("mpd.host cannot be empty"));
        }
        if self.mpd.port == 0 {
            return Err(invalid_config("mpd.port must be between 1 and 65535"));
        }
        if self.playback.seek_seconds == 0 {
            return Err(invalid_config(
                "playback.seek_seconds must be greater than zero",
            ));
        }
        if !(1..=100).contains(&self.playback.volume_step) {
            return Err(invalid_config(
                "playback.volume_step must be between 1 and 100",
            ));
        }
        self.keybindings.validate()
    }
}

fn parse_key_spec(spec: &str) -> Result<KeyEvent, String> {
    if spec == "+" {
        return Ok(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::empty()));
    }
    let mut modifiers = KeyModifiers::empty();
    let mut base = spec;
    while !base.starts_with("Char(") {
        let Some((modifier, rest)) = base.split_once('+') else {
            break;
        };
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "alt" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            _ => return Err(format!("unknown modifier `{modifier}`")),
        }
        base = rest;
    }
    if base.is_empty() {
        return Err("missing key".into());
    }
    let lowered_base = base.to_ascii_lowercase();
    let code = if let Some(number) = lowered_base
        .strip_prefix('f')
        .and_then(|number| number.parse::<u8>().ok())
        .filter(|number| (1..=24).contains(number))
    {
        KeyCode::F(number)
    } else {
        match lowered_base.as_str() {
            "esc" | "escape" => KeyCode::Esc,
            "enter" | "return" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "backspace" => KeyCode::Backspace,
            "delete" | "del" => KeyCode::Delete,
            "insert" => KeyCode::Insert,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "space" => KeyCode::Char(' '),
            _ if lowered_base.starts_with("char(") && base.ends_with(')') => {
                let inner = &base[5..base.len() - 1];
                let mut chars = inner.chars();
                let character = chars
                    .next()
                    .ok_or_else(|| "empty Char(...) key".to_string())?;
                if chars.next().is_some() {
                    return Err("Char(...) must contain exactly one character".into());
                }
                KeyCode::Char(character)
            }
            _ => {
                let mut chars = base.chars();
                let character = chars.next().ok_or_else(|| "missing key".to_string())?;
                if chars.next().is_some() {
                    return Err(format!("unknown key `{base}`"));
                }
                KeyCode::Char(character)
            }
        }
    };
    Ok(KeyEvent::new(code, modifiers))
}

fn key_matches(binding: KeyEvent, event: KeyEvent) -> bool {
    if binding.code != event.code {
        return false;
    }
    let mut event_modifiers = event.modifiers;
    if !binding.modifiers.contains(KeyModifiers::SHIFT) {
        event_modifiers.remove(KeyModifiers::SHIFT);
    }
    binding.modifiers == event_modifiers
}

pub fn config_path() -> io::Result<PathBuf> {
    let base = env::var_os("CONFIG_HOME")
        .or_else(|| env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "CONFIG_HOME, XDG_CONFIG_HOME, and HOME are all unset",
            )
        })?;
    if base.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "configuration home environment variable cannot be empty",
        ));
    }
    Ok(base.join("rmptui").join("config.toml"))
}

fn default_config_text() -> AppResult<String> {
    let serialized = toml::to_string_pretty(&Config::default())?;
    let mut documented = String::from(
        "# rmptui configuration. Values below are the built-in defaults.\n\
         # Save changes and restart rmptui for them to take effect.\n\
         #\n\
         # Config file location: $CONFIG_HOME/rmptui/config.toml, falling back to\n\
         # $XDG_CONFIG_HOME/rmptui/config.toml, then $HOME/.config/rmptui/config.toml.\n\
         #\n\
         # Environment overrides:\n\
         #   MPD_HOST overrides [mpd].host. It may include a port (host:port),\n\
         #   or credentials as password@host. A port embedded in MPD_HOST wins\n\
         #   over MPD_PORT.\n\
         #   MPD_PORT overrides [mpd].port when MPD_HOST has no embedded port.\n\
         #\n\
         # Key names may be characters or named keys (Enter, Esc, Tab, Space,\n\
         # arrows, Backspace, F1-F24). Prefix modifiers with Ctrl+, Alt+, or\n\
         # Shift+. Use arrays for aliases and [] to disable an action.\n\
         # The password is stored as plain text; this file is created with\n\
         # owner-only permissions on Unix.\n\n",
    );

    let mut section = "";
    for line in serialized.lines() {
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            let description = match section {
                "mpd" => "# MPD server connection settings.",
                "playback" => "# Playback adjustment amounts.",
                "keybindings" => "# Action keys, matched only in relevant UI contexts.",
                _ => "# Configuration options.",
            };
            documented.push_str(description);
            documented.push('\n');
            documented.push_str(line);
            documented.push('\n');
            continue;
        }

        if let Some((key, _)) = line.split_once(" = ") {
            let description = option_description(section, key);
            documented.push_str("# ");
            documented.push_str(description);
            documented.push('\n');
        }
        documented.push_str(line);
        documented.push('\n');
    }

    Ok(documented)
}

fn option_description(section: &str, key: &str) -> &'static str {
    match (section, key) {
        ("mpd", "host") => "MPD hostname or address. MPD_HOST overrides this value.",
        ("mpd", "port") => "MPD TCP port. MPD_PORT overrides it unless MPD_HOST includes a port.",
        ("mpd", "password") => {
            "Optional MPD password; leave empty when the server has no password."
        }
        ("mpd", "music_directory") => {
            "Absolute path to your MPD music directory (needed for tag editing)."
        }
        ("playback", "seek_seconds") => "Seconds moved by seek_forward and seek_backward.",
        ("playback", "volume_step") => {
            "Volume percentage changed by volume_up and volume_down (1-100)."
        }
        ("keybindings", "quit") => "Exit rmptui.",
        ("keybindings", "clear_queue") => "Remove every song from the current play queue.",
        ("keybindings", "toggle_pause") => "Toggle playback between paused and playing.",
        ("keybindings", "pause") => "Pause playback.",
        ("keybindings", "toggle_repeat") => "Toggle repeat mode.",
        ("keybindings", "toggle_random") => "Toggle random playback mode.",
        ("keybindings", "open_playlist_menu") => {
            "Open the menu for adding the selection to a playlist."
        }
        ("keybindings", "seek_forward") => "Seek forward by playback.seek_seconds.",
        ("keybindings", "seek_backward") => "Seek backward by playback.seek_seconds.",
        ("keybindings", "cycle_tabs") => "Cycle through Queue, Browser, Playlists, and Tags views.",
        ("keybindings", "select_queue") => "Open the Queue view.",
        ("keybindings", "select_browser") => "Open the Browser view.",
        ("keybindings", "select_playlists") => "Open the Playlists view.",
        ("keybindings", "select_tags") => "Open the Tag editor view.",
        ("keybindings", "next_track") => "Start the next track in the queue.",
        ("keybindings", "previous_track") => "Start the previous track in the queue.",
        ("keybindings", "volume_up") => "Increase volume by playback.volume_step.",
        ("keybindings", "volume_down") => "Decrease volume by playback.volume_step.",
        ("keybindings", "toggle_mute") => "Toggle mute.",
        ("keybindings", "update_database") => "Ask MPD to rescan its music database.",
        ("keybindings", "search") => "Start fuzzy search in the current list.",
        ("keybindings", "toggle_selected_item") => {
            "Toggle the selected song in the queue or browser."
        }
        ("keybindings", "search_next") => "Select the next browser fuzzy-search match.",
        ("keybindings", "search_previous") => "Select the previous browser fuzzy-search match.",
        ("keybindings", "move_down") => "Move selection down in the focused list.",
        ("keybindings", "move_up") => "Move selection up in the focused list.",
        ("keybindings", "activate") => {
            "Activate the focused item (for example, play a song or enter a directory)."
        }
        ("keybindings", "go_back") => "Go to the parent directory in the Browser view.",
        ("keybindings", "go_to_top") => "Select the first item in the focused list.",
        ("keybindings", "go_to_bottom") => "Select the last item in the focused list.",
        ("keybindings", "queue_delete") => "Remove the selected song from the current queue.",
        ("keybindings", "queue_move_down") => "Move the selected queue song down one position.",
        ("keybindings", "queue_move_up") => "Move the selected queue song up one position.",
        ("keybindings", "playlist_rename") => "Rename the selected saved playlist.",
        ("keybindings", "playlist_focus_contents") => {
            "Focus the selected playlist's song contents."
        }
        ("keybindings", "playlist_focus_list") => "Return focus to the saved-playlist list.",
        ("keybindings", "playlist_delete_song") => {
            "Delete the selected song from the saved playlist."
        }
        ("keybindings", "playlist_move_song_down") => {
            "Move the selected saved-playlist song down one position."
        }
        ("keybindings", "playlist_move_song_up") => {
            "Move the selected saved-playlist song up one position."
        }
        ("keybindings", "playlist_delete") => "Delete the selected saved playlist.",
        ("keybindings", "playlist_load") => {
            "Append the selected saved playlist to the queue and start playback."
        }
        ("keybindings", "popup_close") => "Close the add-to-playlist menu.",
        ("keybindings", "popup_select") => {
            "Choose the selected destination in the add-to-playlist menu."
        }
        ("keybindings", "input_cancel") => "Cancel the active search or playlist-name input.",
        ("keybindings", "input_backspace") => "Delete the character before the input cursor.",
        ("keybindings", "input_cursor_left") => "Move the input cursor left.",
        ("keybindings", "input_cursor_right") => "Move the input cursor right.",
        ("keybindings", "input_submit") => "Submit the active search or playlist-name input.",
        ("keybindings", "open_in_tag_editor") => {
            "Open the selected browser song in the Tag editor."
        }
        ("keybindings", "tag_edit") => "Enter edit mode on the selected tag field.",
        ("keybindings", "tag_save") => "Save tag changes to the file.",
        ("keybindings", "tag_reset") => "Reset tags to last saved values.",
        ("keybindings", "tag_transform_capitalize") => "Capitalize each word in all tag fields.",
        ("keybindings", "tag_transform_lower") => "Lowercase all tag fields.",
        ("keybindings", "tag_transform_upper") => "Uppercase all tag fields.",
        ("keybindings", "tag_transform_trim") => "Trim whitespace from all tag fields.",
        _ => "Key aliases for this action.",
    }
}

fn create_config_file(path: &std::path::Path, contents: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents)
}

fn invalid_config(message: &str) -> Box<dyn Error + Send + Sync> {
    Box::new(io::Error::new(
        io::ErrorKind::InvalidData,
        message.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_round_trips_with_all_sections() {
        let config = Config::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&serialized).unwrap();

        assert_eq!(parsed.mpd.host, "127.0.0.1");
        assert_eq!(parsed.mpd.port, 6600);
        assert_eq!(parsed.playback.seek_seconds, 2);
        assert_eq!(parsed.playback.volume_step, 2);
        assert!(parsed.keybindings.matches(
            "quit",
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty())
        ));
        assert!(parsed.keybindings.matches(
            "quit",
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ));
    }

    #[test]
    fn generated_default_config_is_toml_and_has_helpful_comments() {
        let contents = default_config_text().unwrap();
        let parsed: Config = toml::from_str(&contents).unwrap();
        assert_eq!(parsed.mpd.host, "127.0.0.1");
        assert!(contents.starts_with("# rmptui configuration."));
        assert!(contents.contains("password = \"\""));
        assert!(contents.contains("music_directory = \"\""));
        assert!(contents.contains("[keybindings]"));
        assert!(contents.contains("input_submit = [\"Enter\"]"));
        assert!(contents.contains("select_tags = [\"4\"]"));
        assert!(contents.contains("MPD_HOST overrides [mpd].host"));
        assert!(contents.contains("MPD_PORT overrides"));
        assert!(contents.contains("# Focus the selected playlist's song contents."));
        assert!(contents
            .contains("# Append the selected saved playlist to the queue and start playback."));
        assert!(contents.contains("playlist_focus_contents = ["));
        for action in Keybindings::default().bindings.keys() {
            assert_ne!(
                option_description("keybindings", action),
                "Key aliases for this action.",
                "missing generated-config documentation for `{action}`"
            );
        }
    }

    #[test]
    fn creates_default_config_and_reads_user_changes() {
        let unique = format!(
            "rmptui-config-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let directory = env::temp_dir().join(unique);
        let path = directory.join("rmptui").join("config.toml");

        let created = Config::load_or_create_at(&path).unwrap();
        assert_eq!(created.playback.volume_step, 2);
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("password = \"\""));

        fs::write(
            &path,
            contents.replace("volume_step = 2", "volume_step = 8"),
        )
        .unwrap();
        let loaded = Config::load_or_create_at(&path).unwrap();
        assert_eq!(loaded.playback.volume_step, 8);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn partial_configuration_uses_defaults_for_missing_values() {
        let parsed: Config = toml::from_str(
            r#"
                [mpd]
                host = "music.local"
                port = 6601
            "#,
        )
        .unwrap();

        assert_eq!(parsed.mpd.host, "music.local");
        assert_eq!(parsed.mpd.port, 6601);
        assert_eq!(parsed.playback.seek_seconds, 2);
        assert_eq!(
            parsed.keybindings.bindings.len(),
            Keybindings::default().bindings.len()
        );
    }

    #[test]
    fn keybinding_parser_supports_aliases_modifiers_and_punctuation() {
        let bindings = Keybindings::default();
        assert!(bindings.matches(
            "volume_up",
            KeyEvent::new(KeyCode::Char('+'), KeyModifiers::empty())
        ));
        assert!(bindings.matches(
            "move_down",
            KeyEvent::new(KeyCode::Down, KeyModifiers::empty())
        ));
        assert!(bindings.matches(
            "quit",
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ));
        assert!(bindings.matches(
            "search_previous",
            KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT)
        ));
        assert!(parse_key_spec("Ctrl+Char(+)").is_ok());
        assert_eq!(parse_key_spec("F12").unwrap().code, KeyCode::F(12));
    }

    #[test]
    fn partial_keybinding_configuration_keeps_other_defaults() {
        let mut parsed: Config = toml::from_str(
            r#"
                [keybindings]
                quit = ["x"]
            "#,
        )
        .unwrap();
        parsed.keybindings.apply_missing_defaults();

        assert!(parsed.keybindings.matches(
            "quit",
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty())
        ));
        assert!(parsed.keybindings.matches(
            "move_down",
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty())
        ));
    }

    #[test]
    fn playlist_focus_defaults_migrate_with_existing_default_config() {
        let mut parsed: Config = toml::from_str(
            r#"
                [keybindings]
                playlist_focus_contents = ["Right"]
                playlist_focus_list = ["Left"]
            "#,
        )
        .unwrap();
        parsed.keybindings.apply_missing_defaults();

        assert_eq!(
            parsed.keybindings.bindings["playlist_focus_contents"],
            vec!["Right", "l"]
        );
        assert_eq!(
            parsed.keybindings.bindings["playlist_focus_list"],
            vec!["Left", "h"]
        );
        assert_eq!(
            parsed.keybindings.bindings["playlist_load"],
            vec!["Space", "Enter"]
        );
    }

    #[test]
    fn invalid_keybinding_actions_and_keys_are_rejected() {
        let mut bindings = Keybindings::default();
        bindings
            .bindings
            .insert("unknown_action".to_string(), vec!["x".to_string()]);
        assert!(bindings.validate().is_err());

        let mut bindings = Keybindings::default();
        bindings
            .bindings
            .insert("quit".to_string(), vec!["not-a-key".to_string()]);
        assert!(bindings.validate().is_err());
    }
}
