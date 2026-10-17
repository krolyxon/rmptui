//! Reading and writing tags of local files for the tag editor.

use std::{collections::BTreeMap, path::PathBuf};

use lofty::{config::WriteOptions, prelude::*, tag::Tag};

/// The tags the editor can change, in ncmpcpp's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TagField {
    Title,
    Artist,
    AlbumArtist,
    Album,
    Date,
    Track,
    Genre,
    Composer,
    Performer,
    Disc,
    Comment,
}

impl TagField {
    pub const ALL: [Self; 11] = [
        Self::Title,
        Self::Artist,
        Self::AlbumArtist,
        Self::Album,
        Self::Date,
        Self::Track,
        Self::Genre,
        Self::Composer,
        Self::Performer,
        Self::Disc,
        Self::Comment,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::AlbumArtist => "Album Artist",
            Self::Album => "Album",
            Self::Date => "Date",
            Self::Track => "Track",
            Self::Genre => "Genre",
            Self::Composer => "Composer",
            Self::Performer => "Performer",
            Self::Disc => "Disc",
            Self::Comment => "Comment",
        }
    }

    /// The tag name MPD reports for this field.
    pub fn mpd_tag(self) -> &'static str {
        match self {
            Self::AlbumArtist => "AlbumArtist",
            other => other.label(),
        }
    }

    /// Whether letter-case changes make sense for this field.
    pub fn is_text(self) -> bool {
        !matches!(self, Self::Date | Self::Track | Self::Disc)
    }

    fn item_key(self) -> ItemKey {
        match self {
            Self::Title => ItemKey::TrackTitle,
            Self::Artist => ItemKey::TrackArtist,
            Self::AlbumArtist => ItemKey::AlbumArtist,
            Self::Album => ItemKey::AlbumTitle,
            Self::Date => ItemKey::RecordingDate,
            Self::Track => ItemKey::TrackNumber,
            Self::Genre => ItemKey::Genre,
            Self::Composer => ItemKey::Composer,
            Self::Performer => ItemKey::Performer,
            Self::Disc => ItemKey::DiscNumber,
            Self::Comment => ItemKey::Comment,
        }
    }
}

/// Pending changes for one file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagWrite {
    pub path: PathBuf,
    /// Library-relative path, for reporting and rescanning.
    pub uri: String,
    /// New tag values; an empty value removes the tag.
    pub changes: BTreeMap<TagField, String>,
    /// A new file name (without directory), if the file is to be renamed.
    pub rename: Option<String>,
}

/// Writes the tag changes, creating the primary tag if needed, then renames
/// the file if requested.
pub fn write(write: &TagWrite) -> Result<(), String> {
    let describe = |error: &dyn std::fmt::Display| format!("{}: {error}", write.uri);
    if let Some(name) = &write.rename
        && (name.trim().is_empty() || name.contains('/') || matches!(name.trim(), "." | ".."))
    {
        return Err(format!("{}: invalid file name “{name}”", write.uri));
    }
    if !write.changes.is_empty() {
        let mut file = lofty::read_from_path(&write.path).map_err(|e| describe(&e))?;
        if file.primary_tag().is_none() {
            file.insert_tag(Tag::new(file.primary_tag_type()));
        }
        let tag = file
            .primary_tag_mut()
            .ok_or_else(|| format!("{}: cannot create a tag", write.uri))?;
        for (field, value) in &write.changes {
            let value = value.trim();
            if value.is_empty() {
                tag.remove_key(field.item_key());
            } else if !tag.insert_text(field.item_key(), value.to_owned()) {
                return Err(format!(
                    "{}: the {:?} tag cannot hold {}",
                    write.uri,
                    tag.tag_type(),
                    field.label()
                ));
            }
        }
        tag.save_to_path(&write.path, WriteOptions::default())
            .map_err(|e| describe(&e))?;
    }
    if let Some(name) = &write.rename {
        let target = write.path.with_file_name(name.trim());
        if target.exists() {
            return Err(format!("{}: “{}” already exists", write.uri, name.trim()));
        }
        std::fs::rename(&write.path, &target).map_err(|e| describe(&e))?;
    }
    Ok(())
}

/// `du hast (live)` → `Du Hast (Live)`.
pub fn capitalize_words(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut at_word_start = true;
    for ch in value.chars() {
        if at_word_start {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
        at_word_start = ch.is_whitespace() || matches!(ch, '(' | '[' | '-' | '/' | '"');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalizes_first_letters() {
        assert_eq!(
            capitalize_words("du hast (live) - one"),
            "Du Hast (Live) - One"
        );
        assert_eq!(capitalize_words("été wORLD"), "Été WORLD");
    }

    #[test]
    fn rejects_bad_renames() {
        let rename = |name: &str| TagWrite {
            path: "/nonexistent/a.mp3".into(),
            uri: "a.mp3".into(),
            rename: Some(name.into()),
            ..TagWrite::default()
        };
        assert!(
            write(&rename("x/y.mp3"))
                .unwrap_err()
                .contains("invalid file name")
        );
        assert!(
            write(&rename(".."))
                .unwrap_err()
                .contains("invalid file name")
        );
    }
}
