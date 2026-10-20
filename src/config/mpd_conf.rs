//! Reading `music_directory` from the local MPD configuration.
//!
//! MPD only reveals its music directory over a Unix socket, so for a local
//! server reached over TCP rmptui reads it from MPD's own config file.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::{mpd::Address, util::expand_tilde};

/// The music directory of a local MPD, if `address` is local and an MPD
/// config file names one.
pub fn local_music_directory(address: &Address) -> Option<PathBuf> {
    if !address.is_local() {
        return None;
    }
    candidates()
        .iter()
        .find_map(|path| fs::read_to_string(path).ok())
        .and_then(|contents| parse_music_directory(&contents))
}

/// MPD's config file locations, in MPD's lookup order.
fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()) {
        paths.push(Path::new(&dir).join("mpd/mpd.conf"));
    }
    paths.extend(
        [
            "~/.config/mpd/mpd.conf",
            "~/.mpdconf",
            "~/.mpd/mpd.conf",
            "/etc/mpd.conf",
        ]
        .map(expand_tilde),
    );
    paths
}

fn parse_music_directory(contents: &str) -> Option<PathBuf> {
    contents.lines().find_map(|line| {
        let line = line.trim();
        let value = line.strip_prefix("music_directory")?;
        if !value.starts_with(char::is_whitespace) {
            return None;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.split_once('"'))
            .map_or(value, |(inner, _)| inner);
        (!value.is_empty()).then(|| expand_tilde(value))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_music_directory() {
        let conf = "# music_directory \"/commented\"\n\
                    music_directory_x \"/no\"\n\
                    music_directory\t\t\"~/music\"   # trailing comment\n";
        let home = env::var("HOME").unwrap();
        assert_eq!(
            parse_music_directory(conf),
            Some(PathBuf::from(format!("{home}/music")))
        );
        assert_eq!(
            parse_music_directory("music_directory /srv/music"),
            Some(PathBuf::from("/srv/music"))
        );
        assert_eq!(parse_music_directory("port \"6600\""), None);
    }

    #[test]
    fn ignores_remote_servers() {
        assert_eq!(
            local_music_directory(&Address::Tcp("192.168.1.20:6600".into())),
            None
        );
    }
}
