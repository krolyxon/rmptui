//! Small helpers shared across modules.

use std::{env, path::PathBuf, time::Duration};

/// Replaces a leading `~` with the home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            env::var_os("HOME").map(PathBuf::from).map_or_else(
                || PathBuf::from(path),
                |home| home.join(rest.trim_start_matches('/')),
            )
        }
        _ => PathBuf::from(path),
    }
}

/// `$XDG_CACHE_HOME/rmptui`, falling back to `~/.cache/rmptui`.
pub fn cache_dir() -> PathBuf {
    env::var_os("XDG_CACHE_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| expand_tilde("~/.cache"))
        .join("rmptui")
}

/// Formats a duration as `m:ss` or `h:mm:ss`.
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Spells out a duration like ncmpcpp: `1 hour, 3 minutes, 56 seconds`.
pub fn format_length(duration: Duration) -> String {
    let secs = duration.as_secs();
    let parts = [
        (secs / 86_400, "day"),
        (secs / 3600 % 24, "hour"),
        (secs / 60 % 60, "minute"),
        (secs % 60, "second"),
    ];
    let words: Vec<String> = parts
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, unit)| format!("{n} {unit}{}", if *n == 1 { "" } else { "s" }))
        .collect();
    if words.is_empty() {
        "0 seconds".to_owned()
    } else {
        words.join(", ")
    }
}

/// `1 item` / `3 items`.
pub fn plural(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_lengths() {
        assert_eq!(
            format_length(Duration::from_secs(236)),
            "3 minutes, 56 seconds"
        );
        assert_eq!(format_length(Duration::from_secs(3601)), "1 hour, 1 second");
        assert_eq!(format_length(Duration::ZERO), "0 seconds");
        assert_eq!(plural(1, "item"), "1 item");
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(Duration::from_secs(5)), "0:05");
        assert_eq!(format_duration(Duration::from_secs(605)), "10:05");
        assert_eq!(format_duration(Duration::from_secs(3725)), "1:02:05");
    }

    #[test]
    fn expands_tilde() {
        let home = env::var("HOME").unwrap();
        assert_eq!(
            expand_tilde("~/music"),
            PathBuf::from(format!("{home}/music"))
        );
        assert_eq!(expand_tilde("/abs"), PathBuf::from("/abs"));
        assert_eq!(expand_tilde("~user/x"), PathBuf::from("~user/x"));
    }
}
