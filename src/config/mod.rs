//! Configuration: loading, defaults, validation and the MPD address.
//!
//! `default.toml` is the single source of truth for defaults. A user file is
//! merged over it table by table, so partial files keep every default they
//! leave out.

mod keys;
mod theme;

use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    net::Ipv6Addr,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;

pub use keys::{Bindings, KeyCombo, Keymap};
pub use theme::Theme;

use crate::{action::ScreenId, mpd::Address, util::expand_tilde};

pub const DEFAULT_CONFIG: &str = include_str!("default.toml");

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot access {}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("invalid configuration in {}: {message}", path.display())]
    Parse { path: PathBuf, message: String },
    #[error("invalid configuration: {0}")]
    Invalid(String),
    #[error(
        "{} uses the rmptui 0.1 format, which 0.2 no longer reads.\n\
         Move it aside (e.g. to config.toml.old) and start rmptui again to \
         generate a documented 0.2 configuration, then copy your settings over.",
        .0.display()
    )]
    Legacy(PathBuf),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MpdConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub music_directory: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaybackConfig {
    pub seek_seconds: u32,
    pub volume_step: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnField {
    Artist,
    AlbumArtist,
    Title,
    Album,
    Track,
    Disc,
    Date,
    Genre,
    Duration,
    File,
}

impl ColumnField {
    pub fn title(self) -> &'static str {
        match self {
            Self::Artist => "Artist",
            Self::AlbumArtist => "Album Artist",
            Self::Title => "Title",
            Self::Album => "Album",
            Self::Track => "#",
            Self::Disc => "Disc",
            Self::Date => "Date",
            Self::Genre => "Genre",
            Self::Duration => "Time",
            Self::File => "File",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub field: ColumnField,
    pub width: u16,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiConfig {
    pub startup_screen: ScreenId,
    pub mouse: bool,
    pub lyrics_pane: bool,
    pub lyrics_pane_width: u16,
    pub message_timeout: u64,
    pub queue_columns: Vec<Column>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LyricsConfig {
    pub online: bool,
    pub cache: bool,
    pub lrclib_url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualizerConfig {
    pub fifo: PathBuf,
    pub sample_rate: u32,
    pub channels: u16,
    pub framerate: u32,
    pub bar_width: u16,
    pub bar_gap: u16,
    pub min_frequency: f32,
    pub max_frequency: f32,
    pub falloff: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    mpd: MpdConfig,
    playback: PlaybackConfig,
    ui: UiConfig,
    theme: Theme,
    lyrics: LyricsConfig,
    visualizer: VisualizerConfig,
    keys: Bindings,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub mpd: MpdConfig,
    pub playback: PlaybackConfig,
    pub ui: UiConfig,
    pub theme: Theme,
    pub lyrics: LyricsConfig,
    pub visualizer: VisualizerConfig,
    pub keymap: Keymap,
}

impl Default for Config {
    fn default() -> Self {
        Self::from_toml(DEFAULT_CONFIG).expect("the built-in default configuration is valid")
    }
}

impl Config {
    /// Loads the config at `path`, or at the default location. A missing
    /// file at the default location is created from the documented defaults.
    pub fn load(path: Option<&Path>) -> Result<(Self, PathBuf), ConfigError> {
        let (path, create) = match path {
            Some(path) => (path.to_path_buf(), false),
            None => (default_path(), true),
        };
        let io_error = |source| ConfigError::Io {
            path: path.clone(),
            source,
        };
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
                write_default(&path).map_err(io_error)?;
                DEFAULT_CONFIG.to_owned()
            }
            Err(error) => return Err(io_error(error)),
        };
        let user: toml::Table = toml::from_str(&contents).map_err(|error| ConfigError::Parse {
            path: path.clone(),
            message: error.to_string(),
        })?;
        if user.contains_key("keybindings") {
            return Err(ConfigError::Legacy(path));
        }
        let config = Self::from_table(user).map_err(|error| match error {
            ConfigError::Invalid(message) => ConfigError::Parse {
                path: path.clone(),
                message,
            },
            other => other,
        })?;
        Ok((config, path))
    }

    /// Parses a (partial) config, filling in defaults.
    pub fn from_toml(contents: &str) -> Result<Self, ConfigError> {
        let user: toml::Table =
            toml::from_str(contents).map_err(|error| ConfigError::Invalid(error.to_string()))?;
        Self::from_table(user)
    }

    fn from_table(user: toml::Table) -> Result<Self, ConfigError> {
        let mut merged: toml::Table =
            toml::from_str(DEFAULT_CONFIG).expect("the built-in default configuration is TOML");
        merge(&mut merged, user);
        let raw: RawConfig = toml::Value::Table(merged)
            .try_into()
            .map_err(|error: toml::de::Error| ConfigError::Invalid(error.to_string()))?;
        let config = Self {
            keymap: Keymap::new(raw.keys).map_err(ConfigError::Invalid)?,
            mpd: raw.mpd,
            playback: raw.playback,
            ui: raw.ui,
            theme: raw.theme,
            lyrics: raw.lyrics,
            visualizer: raw.visualizer,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let check = |ok: bool, message: &str| {
            if ok {
                Ok(())
            } else {
                Err(ConfigError::Invalid(message.to_owned()))
            }
        };
        check(!self.mpd.host.trim().is_empty(), "mpd.host cannot be empty")?;
        check(self.mpd.port != 0, "mpd.port must be between 1 and 65535")?;
        check(
            self.playback.seek_seconds > 0,
            "playback.seek_seconds must be greater than zero",
        )?;
        check(
            (1..=100).contains(&self.playback.volume_step),
            "playback.volume_step must be between 1 and 100",
        )?;
        check(
            (10..=90).contains(&self.ui.lyrics_pane_width),
            "ui.lyrics_pane_width must be between 10 and 90",
        )?;
        check(
            !self.ui.queue_columns.is_empty(),
            "ui.queue_columns cannot be empty",
        )?;
        let vis = &self.visualizer;
        check(
            vis.sample_rate > 0,
            "visualizer.sample_rate must be greater than zero",
        )?;
        check(
            matches!(vis.channels, 1 | 2),
            "visualizer.channels must be 1 or 2",
        )?;
        check(
            (1..=240).contains(&vis.framerate),
            "visualizer.framerate must be between 1 and 240",
        )?;
        check(
            vis.bar_width > 0,
            "visualizer.bar_width must be greater than zero",
        )?;
        check(
            vis.min_frequency > 0.0 && vis.min_frequency < vis.max_frequency,
            "visualizer.min_frequency must be positive and below max_frequency",
        )?;
        check(
            vis.falloff > 0.0,
            "visualizer.falloff must be greater than zero",
        )?;
        check(
            !self.theme.visualizer.is_empty(),
            "theme.visualizer needs at least one color",
        )?;
        Ok(())
    }

    /// The music directory from the config, if set.
    pub fn music_directory(&self) -> Option<PathBuf> {
        let dir = self.mpd.music_directory.trim();
        (!dir.is_empty()).then(|| expand_tilde(dir))
    }

    pub fn message_timeout(&self) -> Duration {
        Duration::from_secs(self.ui.message_timeout)
    }

    /// The MPD address and password, honoring `MPD_HOST` and `MPD_PORT`.
    pub fn mpd_connection(&self) -> Result<(Address, Option<String>), ConfigError> {
        resolve_connection(
            &self.mpd,
            env::var("MPD_HOST").ok().filter(|h| !h.is_empty()),
            env::var("MPD_PORT").ok().filter(|p| !p.is_empty()),
        )
    }
}

/// Recursively merges `user` into `base`. Tables merge; other values replace.
fn merge(base: &mut toml::Table, user: toml::Table) {
    for (key, value) in user {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(base)), toml::Value::Table(user)) => merge(base, user),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

/// `$CONFIG_HOME/rmptui/config.toml`, falling back to `$XDG_CONFIG_HOME`
/// and then `~/.config`.
pub fn default_path() -> PathBuf {
    env::var_os("CONFIG_HOME")
        .or_else(|| env::var_os("XDG_CONFIG_HOME"))
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| expand_tilde("~/.config"))
        .join("rmptui")
        .join("config.toml")
}

fn write_default(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // The file may hold the MPD password.
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => file.write_all(DEFAULT_CONFIG.as_bytes()),
        // Another instance created it in the meantime.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

fn resolve_connection(
    mpd: &MpdConfig,
    env_host: Option<String>,
    env_port: Option<String>,
) -> Result<(Address, Option<String>), ConfigError> {
    let configured_password = (!mpd.password.is_empty()).then(|| mpd.password.clone());
    let (host, password) = match env_host {
        Some(host) => match host.split_once('@') {
            Some((password, host)) => (host.to_owned(), Some(password.to_owned())),
            None => (host, configured_password),
        },
        None => (mpd.host.clone(), configured_password),
    };
    if host.is_empty() {
        return Err(ConfigError::Invalid("MPD host cannot be empty".into()));
    }
    if host.starts_with('/') || host.starts_with('~') {
        return Ok((Address::Unix(expand_tilde(&host)), password));
    }
    let port = match env_port {
        Some(port) => port
            .parse::<u16>()
            .ok()
            .filter(|&p| p != 0)
            .ok_or_else(|| ConfigError::Invalid(format!("invalid MPD_PORT value `{port}`")))?,
        None => mpd.port,
    };
    Ok((Address::Tcp(tcp_address(&host, port)), password))
}

/// Formats `host` and `port` for connecting, keeping an explicit port in
/// `host` and bracketing IPv6 addresses.
fn tcp_address(host: &str, port: u16) -> String {
    if host.starts_with('[') && host.contains("]:") {
        host.to_owned()
    } else if let Some(ipv6) = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .filter(|h| h.parse::<Ipv6Addr>().is_ok())
    {
        format!("[{ipv6}]:{port}")
    } else if host.parse::<Ipv6Addr>().is_ok() {
        format!("[{host}]:{port}")
    } else if host
        .rsplit_once(':')
        .is_some_and(|(_, suffix)| suffix.parse::<u16>().is_ok())
    {
        host.to_owned()
    } else {
        format!("{host}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, Context};

    #[test]
    fn default_config_is_valid_and_binds_every_action() {
        let config = Config::default();
        for &action in Action::ALL {
            let bound = action
                .contexts()
                .iter()
                .any(|&context| !config.keymap.keys_for(context, action).is_empty());
            assert!(bound, "action `{action}` has no default binding");
        }
        assert_eq!(config.mpd.port, 6600);
        assert_eq!(config.ui.startup_screen, ScreenId::Queue);
    }

    #[test]
    fn partial_config_keeps_defaults() {
        let config = Config::from_toml(
            r#"
            [playback]
            volume_step = 10
            [keys.global]
            quit = ["Q"]
            "#,
        )
        .unwrap();
        assert_eq!(config.playback.volume_step, 10);
        assert_eq!(config.playback.seek_seconds, 5);
        let quit = config.keymap.keys_for(Context::Global, Action::Quit);
        assert_eq!(quit, ["Q".parse::<KeyCombo>().unwrap()]);
        // Other global bindings are untouched.
        assert!(
            !config
                .keymap
                .keys_for(Context::Global, Action::TogglePause)
                .is_empty()
        );
    }

    #[test]
    fn rejects_bad_values() {
        for bad in [
            "[mpd]\nport = 0",
            "[playback]\nvolume_step = 0",
            "[mpd]\nhots = \"typo\"",
            "[theme]\naccent = \"notacolor\"",
            "[keys.global]\nquit = [\"Hyper+q\"]",
            "[keys.global]\nnot_an_action = [\"x\"]",
            // `p` is already bound to toggle_pause.
            "[keys.global]\nquit = [\"p\"]",
        ] {
            assert!(Config::from_toml(bad).is_err(), "accepted `{bad}`");
        }
    }

    #[test]
    fn detects_legacy_config() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-tmp")
            .join(format!("config-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("legacy.toml");
        fs::write(&path, "[keybindings]\nquit = [\"q\"]\n").unwrap();
        let result = Config::load(Some(&path));
        fs::remove_dir_all(&dir).unwrap();
        assert!(matches!(result, Err(ConfigError::Legacy(_))));
    }

    fn mpd_config(host: &str, password: &str) -> MpdConfig {
        MpdConfig {
            host: host.into(),
            port: 6600,
            password: password.into(),
            music_directory: String::new(),
        }
    }

    #[test]
    fn resolves_connection() {
        let resolve = |host: &str, env_host: Option<&str>, env_port: Option<&str>| {
            resolve_connection(
                &mpd_config(host, "cfg"),
                env_host.map(Into::into),
                env_port.map(Into::into),
            )
            .unwrap()
        };
        assert_eq!(
            resolve("localhost", None, None),
            (Address::Tcp("localhost:6600".into()), Some("cfg".into()))
        );
        assert_eq!(
            resolve("x", Some("secret@host"), Some("6601")),
            (Address::Tcp("host:6601".into()), Some("secret".into()))
        );
        assert_eq!(
            resolve("/run/mpd/socket", None, None).0,
            Address::Unix("/run/mpd/socket".into())
        );
        assert!(resolve_connection(&mpd_config("x", ""), None, Some("nope".into())).is_err());
    }

    #[test]
    fn formats_tcp_addresses() {
        assert_eq!(tcp_address("localhost", 6600), "localhost:6600");
        assert_eq!(tcp_address("localhost:6601", 6600), "localhost:6601");
        assert_eq!(tcp_address("[::1]", 6600), "[::1]:6600");
        assert_eq!(tcp_address("[::1]:6601", 6600), "[::1]:6601");
        assert_eq!(tcp_address("::1", 6600), "[::1]:6600");
    }
}
