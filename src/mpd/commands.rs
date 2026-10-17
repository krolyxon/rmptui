//! Constructors for the commands rmptui sends.

use super::protocol::Command;

fn bool_arg(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

pub fn play() -> Command {
    Command::new("play")
}

pub fn play_pos(pos: u32) -> Command {
    Command::new("play").arg(pos.to_string())
}

pub fn play_id(id: u32) -> Command {
    Command::new("playid").arg(id.to_string())
}

pub fn pause(paused: bool) -> Command {
    Command::new("pause").arg(bool_arg(paused))
}

pub fn stop() -> Command {
    Command::new("stop")
}

pub fn next() -> Command {
    Command::new("next")
}

pub fn previous() -> Command {
    Command::new("previous")
}

/// Seeks relative to the current position.
pub fn seek_relative(seconds: i64) -> Command {
    Command::new("seekcur").arg(format!("{seconds:+}"))
}

pub fn seek_to(seconds: f64) -> Command {
    Command::new("seekcur").arg(format!("{seconds:.3}"))
}

/// Changes the volume relative to the current level. The server applies the
/// change atomically, so fast repeated key presses are never lost.
pub fn change_volume(delta: i32) -> Command {
    Command::new("volume").arg(format!("{delta:+}"))
}

pub fn set_volume(volume: u8) -> Command {
    Command::new("setvol").arg(volume.min(100).to_string())
}

pub fn repeat(on: bool) -> Command {
    Command::new("repeat").arg(bool_arg(on))
}

pub fn random(on: bool) -> Command {
    Command::new("random").arg(bool_arg(on))
}

pub fn single(on: bool) -> Command {
    Command::new("single").arg(bool_arg(on))
}

pub fn consume(on: bool) -> Command {
    Command::new("consume").arg(bool_arg(on))
}

pub fn update(path: Option<&str>) -> Command {
    let command = Command::new("update");
    match path {
        Some(path) if !path.is_empty() => command.arg(path),
        _ => command,
    }
}

pub fn clear() -> Command {
    Command::new("clear")
}

pub fn shuffle() -> Command {
    Command::new("shuffle")
}

/// Adds a song or (recursively) a directory to the queue.
pub fn add(uri: &str) -> Command {
    Command::new("add").arg(uri)
}

pub fn add_id(uri: &str) -> Command {
    Command::new("addid").arg(uri)
}

pub fn delete_id(id: u32) -> Command {
    Command::new("deleteid").arg(id.to_string())
}

pub fn move_id(id: u32, to: u32) -> Command {
    Command::new("moveid")
        .arg(id.to_string())
        .arg(to.to_string())
}

pub fn find_add(filter: &str) -> Command {
    Command::new("findadd").arg(filter)
}

pub fn load(playlist: &str) -> Command {
    Command::new("load").arg(playlist)
}

pub fn save(playlist: &str) -> Command {
    Command::new("save").arg(playlist)
}

pub fn rename(from: &str, to: &str) -> Command {
    Command::new("rename").arg(from).arg(to)
}

pub fn remove_playlist(playlist: &str) -> Command {
    Command::new("rm").arg(playlist)
}

/// Adds a song or directory to a stored playlist, creating it if needed.
pub fn playlist_add(playlist: &str, uri: &str) -> Command {
    Command::new("playlistadd").arg(playlist).arg(uri)
}

/// Adds every song matching `filter` to a stored playlist.
pub fn playlist_add_filter(playlist: &str, filter: &str) -> Command {
    Command::new("searchaddpl").arg(playlist).arg(filter)
}

pub fn playlist_delete(playlist: &str, pos: u32) -> Command {
    Command::new("playlistdelete")
        .arg(playlist)
        .arg(pos.to_string())
}

pub fn playlist_move(playlist: &str, from: u32, to: u32) -> Command {
    Command::new("playlistmove")
        .arg(playlist)
        .arg(from.to_string())
        .arg(to.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_relative_arguments() {
        assert_eq!(seek_relative(5).as_str(), r#"seekcur "+5""#);
        assert_eq!(seek_relative(-5).as_str(), r#"seekcur "-5""#);
        assert_eq!(change_volume(-2).as_str(), r#"volume "-2""#);
        assert_eq!(update(None).as_str(), "update");
        assert_eq!(set_volume(140).as_str(), r#"setvol "100""#);
    }
}
