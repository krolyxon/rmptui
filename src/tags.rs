//! Reading and writing tags of local files for the tag editor.

use std::{collections::BTreeMap, path::PathBuf};

use lofty::{config::WriteOptions, prelude::*, tag::Tag};

/// The tags the editor can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TagField {
    Title,
    Artist,
    AlbumArtist,
    Album,
    Track,
    Disc,
    Date,
    Genre,
    Comment,
}

impl TagField {
    pub const ALL: [Self; 9] = [
        Self::Title,
        Self::Artist,
        Self::AlbumArtist,
        Self::Album,
        Self::Track,
        Self::Disc,
        Self::Date,
        Self::Genre,
        Self::Comment,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::AlbumArtist => "Album Artist",
            Self::Album => "Album",
            Self::Track => "Track",
            Self::Disc => "Disc",
            Self::Date => "Date",
            Self::Genre => "Genre",
            Self::Comment => "Comment",
        }
    }

    /// The tag name MPD reports for this field.
    pub fn mpd_tag(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::AlbumArtist => "AlbumArtist",
            Self::Album => "Album",
            Self::Track => "Track",
            Self::Disc => "Disc",
            Self::Date => "Date",
            Self::Genre => "Genre",
            Self::Comment => "Comment",
        }
    }

    fn item_key(self) -> ItemKey {
        match self {
            Self::Title => ItemKey::TrackTitle,
            Self::Artist => ItemKey::TrackArtist,
            Self::AlbumArtist => ItemKey::AlbumArtist,
            Self::Album => ItemKey::AlbumTitle,
            Self::Track => ItemKey::TrackNumber,
            Self::Disc => ItemKey::DiscNumber,
            Self::Date => ItemKey::RecordingDate,
            Self::Genre => ItemKey::Genre,
            Self::Comment => ItemKey::Comment,
        }
    }
}

/// Pending changes for one file. An empty value removes the tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagWrite {
    pub path: PathBuf,
    /// Library-relative path, for reporting and rescanning.
    pub uri: String,
    pub changes: BTreeMap<TagField, String>,
}

/// Writes `changes` into the file, creating its primary tag if needed.
pub fn write(write: &TagWrite) -> Result<(), String> {
    let describe = |error: &dyn std::fmt::Display| format!("{}: {error}", write.uri);
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
        .map_err(|e| describe(&e))
}
