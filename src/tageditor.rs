//! Read and write audio tags with `lofty`.

use std::collections::BTreeMap;
use std::path::Path;

use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;

use crate::AppResult;

pub const FIELDS: &[(&str, &str)] = &[
    ("title", "Title"),
    ("artist", "Artist"),
    ("albumartist", "Album Artist"),
    ("album", "Album"),
    ("date", "Date"),
    ("track", "Track"),
    ("genre", "Genre"),
    ("composer", "Composer"),
    ("performer", "Performer"),
    ("disc", "Disc"),
    ("comment", "Comment"),
];

pub type TagMap = BTreeMap<String, String>;

pub fn read(path: &Path) -> AppResult<TagMap> {
    let tagged_file = Probe::open(path)?.read()?;
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let mut map = TagMap::new();
    for (name, _) in FIELDS {
        let value = tag
            .and_then(|tag| read_field(tag, name))
            .unwrap_or_default();
        map.insert((*name).to_string(), value);
    }
    Ok(map)
}

fn read_field(tag: &lofty::tag::Tag, field: &str) -> Option<String> {
    match field {
        "title" => tag.title().map(|s| s.to_string()),
        "artist" => tag.artist().map(|s| s.to_string()),
        "album" => tag.album().map(|s| s.to_string()),
        "genre" => tag.genre().map(|s| s.to_string()),
        "track" => tag.track().map(|n| n.to_string()),
        "disc" => tag.disk().map(|n| n.to_string()),
        "date" => tag.get_string(ItemKey::Year).map(str::to_string),
        "albumartist" => tag.get_string(ItemKey::AlbumArtist).map(str::to_string),
        "composer" => tag.get_string(ItemKey::Composer).map(str::to_string),
        "performer" => tag.get_string(ItemKey::Performer).map(str::to_string),
        "comment" => tag.get_string(ItemKey::Comment).map(str::to_string),
        _ => None,
    }
}

pub fn write(path: &Path, map: &TagMap) -> AppResult<()> {
    let mut tagged_file = Probe::open(path)?.read()?;

    if tagged_file.primary_tag().is_none() {
        let tag_type = tagged_file.primary_tag_type();
        tagged_file.insert_tag(lofty::tag::Tag::new(tag_type));
    }
    let tag = tagged_file.primary_tag_mut().expect("just inserted");

    for (name, value) in map {
        write_field(tag, name, value);
    }

    tagged_file.save_to_path(path, WriteOptions::default())?;
    Ok(())
}

fn write_field(tag: &mut lofty::tag::Tag, field: &str, value: &str) {
    match field {
        "title" => tag.set_title(value.to_string()),
        "artist" => tag.set_artist(value.to_string()),
        "album" => tag.set_album(value.to_string()),
        "genre" => tag.set_genre(value.to_string()),
        "track" => set_u32(value, |n| tag.set_track(n)),
        "disc" => set_u32(value, |n| tag.set_disk(n)),
        "date" => set_item(tag, ItemKey::Year, value),
        "albumartist" => set_item(tag, ItemKey::AlbumArtist, value),
        "composer" => set_item(tag, ItemKey::Composer, value),
        "performer" => set_item(tag, ItemKey::Performer, value),
        "comment" => set_item(tag, ItemKey::Comment, value),
        _ => {}
    }
}

fn set_u32(value: &str, f: impl FnOnce(u32)) {
    if let Ok(n) = value.trim().parse::<u32>() {
        f(n);
    }
}

fn set_item(tag: &mut lofty::tag::Tag, key: ItemKey, value: &str) {
    if value.trim().is_empty() {
        tag.remove_key(key);
    } else {
        tag.insert_text(key, value.to_string());
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Transform {
    Capitalize,
    Lower,
    Upper,
    Trim,
}

pub fn transform(map: &mut TagMap, op: Transform) {
    for value in map.values_mut() {
        *value = match op {
            Transform::Capitalize => value
                .split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(first) => {
                            first.to_uppercase().collect::<String>()
                                + &chars.as_str().to_lowercase()
                        }
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
            Transform::Lower => value.to_lowercase(),
            Transform::Upper => value.to_uppercase(),
            Transform::Trim => value.trim().to_string(),
        };
    }
}
