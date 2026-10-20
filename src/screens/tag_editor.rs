//! Screen 5: the tag editor, laid out like ncmpcpp's.
//!
//! `Directories` on the left; `Tag types` in the middle (tag fields, file
//! name, letter-case helpers, reset and save); `Tags` on the right shows the
//! selected tag of every song in the highlighted directory. Edits stay pending
//! until saved.

use std::collections::{BTreeMap, HashSet};

use crossterm::event::MouseEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Position, Rect},
    style::Modifier,
    symbols,
    text::Line,
    widgets::{Cell, Row},
};

use super::{Ctx, Effect, Screen, ScreenMessage};
use crate::{
    action::Action,
    mpd::{DirEntry, MpdEvent, Query, Reply, Song, Subsystem},
    popups::{InputPopup, InputPurpose},
    tags::{self, TagField, TagWrite},
    ui::{
        self,
        list::{Click, Header, ListView},
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Pane {
    #[default]
    Directories,
    TagTypes,
    Tags,
}

/// The rows of the `Tag types` column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    Field(TagField),
    Separator,
    FileName,
    Capitalize,
    Lowercase,
    Reset,
    Save,
}

const ITEMS: [Item; 20] = [
    Item::Field(TagField::Title),
    Item::Field(TagField::Artist),
    Item::Field(TagField::AlbumArtist),
    Item::Field(TagField::Album),
    Item::Field(TagField::Date),
    Item::Field(TagField::Track),
    Item::Field(TagField::Genre),
    Item::Field(TagField::Composer),
    Item::Field(TagField::Performer),
    Item::Field(TagField::Disc),
    Item::Field(TagField::Comment),
    Item::Separator,
    Item::FileName,
    Item::Separator,
    Item::Capitalize,
    Item::Lowercase,
    Item::Separator,
    Item::Reset,
    Item::Save,
    Item::Separator,
];

impl Item {
    fn label(self) -> &'static str {
        match self {
            Self::Field(field) => field.label(),
            Self::Separator => "",
            Self::FileName => "Filename",
            Self::Capitalize => "Capitalize First Letters",
            Self::Lowercase => "lower all letters",
            Self::Reset => "Reset",
            Self::Save => "Save",
        }
    }
}

/// Pending changes of one song.
#[derive(Clone, Debug, Default)]
struct Changes {
    tags: BTreeMap<TagField, String>,
    rename: Option<String>,
}

impl Changes {
    fn is_empty(&self) -> bool {
        self.tags.is_empty() && self.rename.is_none()
    }
}

/// What the open input popup edits.
#[derive(Clone, Debug)]
enum Editing {
    /// A tag of these files.
    Field(Vec<String>),
    FileName(String),
}

#[derive(Debug, Default)]
pub struct TagEditorScreen {
    focus: Pane,
    /// The directory whose subdirectories are listed.
    path: String,
    stack: Vec<(String, usize)>,
    restore: Option<usize>,
    loaded: bool,
    folders: Vec<String>,
    /// The directory whose songs are shown in `Tags`.
    shown: Option<String>,
    songs: Vec<Song>,
    dirs: ListView,
    items: ListView,
    tags: ListView,
    marks: HashSet<usize>,
    /// Pending changes by file.
    pending: BTreeMap<String, Changes>,
    editing: Option<Editing>,
}

impl TagEditorScreen {
    fn has_parent(&self) -> bool {
        !self.path.is_empty()
    }

    /// Rows of the directory column: `.`, `..` (unless at the root), folders.
    fn dir_rows(&self) -> usize {
        1 + usize::from(self.has_parent()) + self.folders.len()
    }

    /// The directory a row of the directory column refers to.
    fn dir_at(&self, index: usize) -> Option<String> {
        let parent = usize::from(self.has_parent());
        match index {
            0 => Some(self.path.clone()),
            1 if parent == 1 => Some(
                self.path
                    .rsplit_once('/')
                    .map_or(String::new(), |(parent, _)| parent.to_owned()),
            ),
            _ => self.folders.get(index - 1 - parent).cloned(),
        }
    }

    fn load(&mut self, ctx: &Ctx) {
        self.loaded = false;
        ctx.client.query(Query::Directory(self.path.clone()));
    }

    /// Shows the songs of the highlighted directory.
    fn sync_songs(&mut self, ctx: &Ctx) {
        let target = self
            .dirs
            .selected(self.dir_rows())
            .and_then(|i| self.dir_at(i));
        if target != self.shown {
            self.shown.clone_from(&target);
            self.songs.clear();
            self.tags.reset();
            self.marks.clear();
            if let Some(dir) = target {
                ctx.client.query(Query::Directory(dir));
            }
        }
    }

    fn enter_directory(&mut self, ctx: &Ctx) {
        let Some(index) = self.dirs.selected(self.dir_rows()) else {
            return;
        };
        match index {
            0 => {}
            1 if self.has_parent() => self.back(ctx),
            _ => {
                let Some(dir) = self.dir_at(index) else {
                    return;
                };
                self.stack
                    .push((std::mem::replace(&mut self.path, dir), index));
                self.dirs.reset();
                self.folders.clear();
                self.load(ctx);
                self.sync_songs(ctx);
            }
        }
    }

    fn back(&mut self, ctx: &Ctx) {
        if let Some((parent, index)) = self.stack.pop() {
            self.path = parent;
            self.dirs.reset();
            self.folders.clear();
            self.restore = Some(index);
            self.load(ctx);
        }
    }

    fn selected_item(&self) -> Item {
        ITEMS[self.items.selected(ITEMS.len()).unwrap_or(0)]
    }

    /// Moves within `Tag types`, skipping separators.
    fn navigate_items(&mut self, action: Action) -> bool {
        let before = self.items.selected(ITEMS.len()).unwrap_or(0);
        if !self.items.navigate(action, ITEMS.len()) {
            return false;
        }
        let mut index = self.items.selected(ITEMS.len()).unwrap_or(0);
        while ITEMS[index] == Item::Separator {
            let up = index < before || index == ITEMS.len() - 1;
            index = match (up, index) {
                (true, 0) => break,
                (true, i) => i - 1,
                (false, i) => i + 1,
            };
            self.items.select(index, ITEMS.len());
        }
        true
    }

    /// The songs an action applies to: marked ones, or all shown songs.
    fn targets(&self) -> Vec<usize> {
        if self.marks.is_empty() {
            (0..self.songs.len()).collect()
        } else {
            let mut marks: Vec<_> = self.marks.iter().copied().collect();
            marks.sort_unstable();
            marks
        }
    }

    /// The (possibly pending) value of `field` for a song.
    fn value<'a>(&'a self, song: &'a Song, field: TagField) -> &'a str {
        self.pending
            .get(&song.file)
            .and_then(|changes| changes.tags.get(&field))
            .map(String::as_str)
            .unwrap_or_else(|| song.tag(field.mpd_tag()).unwrap_or_default())
    }

    fn file_name<'a>(&'a self, song: &'a Song) -> &'a str {
        self.pending
            .get(&song.file)
            .and_then(|changes| changes.rename.as_deref())
            .unwrap_or_else(|| song.file_name())
    }

    fn set_value(&mut self, index: usize, field: TagField, value: String) {
        let Some(song) = self.songs.get(index) else {
            return;
        };
        let original = song.tag(field.mpd_tag()).unwrap_or_default();
        let changes = self.pending.entry(song.file.clone()).or_default();
        if value == original {
            changes.tags.remove(&field);
        } else {
            changes.tags.insert(field, value);
        }
        if changes.is_empty() {
            self.pending.remove(&song.file);
        }
    }

    fn set_file_name(&mut self, file: &str, name: String) {
        let Some(song) = self.songs.iter().find(|s| s.file == file) else {
            return;
        };
        let changes = self.pending.entry(song.file.clone()).or_default();
        changes.rename = (name != song.file_name()).then_some(name);
        if changes.is_empty() {
            self.pending.remove(file);
        }
    }

    fn edit_field_for(&mut self, field: TagField, indices: Vec<usize>, ctx: &mut Ctx) {
        if indices.is_empty() {
            return;
        }
        let mut values = indices.iter().map(|&i| self.value(&self.songs[i], field));
        let first = values.next().unwrap_or_default();
        let initial = if values.all(|v| v == first) {
            first.to_owned()
        } else {
            String::new()
        };
        let title = match indices.len() {
            1 => field.label().to_owned(),
            n => format!("{} ({n} songs)", field.label()),
        };
        self.editing = Some(Editing::Field(
            indices
                .iter()
                .map(|&i| self.songs[i].file.clone())
                .collect(),
        ));
        ctx.popup(InputPopup::new(
            title,
            &initial,
            InputPurpose::TagValue(field),
        ));
    }

    fn change_case(&mut self, change: fn(&str) -> String) {
        for index in self.targets() {
            for field in TagField::ALL.into_iter().filter(|f| f.is_text()) {
                let current = self.value(&self.songs[index], field).to_owned();
                let changed = change(&current);
                if changed != current {
                    self.set_value(index, field, changed);
                }
            }
        }
    }

    fn activate_item(&mut self, ctx: &mut Ctx) {
        match self.selected_item() {
            Item::Field(field) => self.edit_field_for(field, self.targets(), ctx),
            Item::FileName => self.focus = Pane::Tags,
            Item::Capitalize => self.change_case(tags::capitalize_words),
            Item::Lowercase => self.change_case(str::to_lowercase),
            Item::Reset => {
                self.pending.clear();
                ctx.notify("Discarded pending tag changes");
            }
            Item::Save => self.save(ctx),
            Item::Separator => {}
        }
    }

    fn activate_song(&mut self, ctx: &mut Ctx) {
        let Some(index) = self.tags.selected(self.songs.len()) else {
            return;
        };
        match self.selected_item() {
            Item::FileName => {
                let song = &self.songs[index];
                let current = self.file_name(song).to_owned();
                self.editing = Some(Editing::FileName(song.file.clone()));
                ctx.popup(InputPopup::new(
                    "Filename",
                    &current,
                    InputPurpose::FileName,
                ));
            }
            Item::Field(field) => self.edit_field_for(field, vec![index], ctx),
            _ => {}
        }
    }

    fn save(&mut self, ctx: &mut Ctx) {
        if self.pending.is_empty() {
            ctx.notify("No pending tag changes");
            return;
        }
        let Some(music_dir) = ctx.mpd.music_dir.clone() else {
            ctx.error("Set mpd.music_directory in the config to write tags");
            return;
        };
        let writes = std::mem::take(&mut self.pending)
            .into_iter()
            .map(|(uri, changes)| TagWrite {
                path: music_dir.join(&uri),
                uri,
                changes: changes.tags,
                rename: changes.rename,
            })
            .collect();
        ctx.effects.push(Effect::WriteTags(writes));
    }
}

impl Screen for TagEditorScreen {
    fn title(&self, _ctx: &Ctx) -> Line<'static> {
        if self.pending.is_empty() {
            Line::from("Tag editor")
        } else {
            Line::from(format!("Tag editor ({} unsaved)", self.pending.len()))
        }
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let theme = ctx.theme();
        if ctx.mpd.music_dir.is_none() {
            ui::draw_placeholder(
                frame,
                area,
                &[
                    "The tag editor needs access to the music files.",
                    "",
                    "Set `music_directory` in the [mpd] section of the rmptui config,",
                    "or in the mpd.conf of a local MPD.",
                ],
                theme,
            );
            return;
        }
        let [dirs_area, items_area, tags_area] = <[Rect; 3]>::try_from(ui::split_columns(
            frame,
            area,
            &[
                Constraint::Fill(1),
                Constraint::Length(25),
                Constraint::Fill(1),
            ],
            theme,
        ))
        .expect("three columns");

        let parent = self.has_parent();
        let folders = &self.folders;
        self.dirs.render(
            frame,
            dirs_area,
            Some(Header::Title(Line::from("Directories"))),
            &[Constraint::Fill(1)],
            self.dir_rows(),
            self.focus == Pane::Directories,
            theme,
            |i| {
                let name = match i {
                    0 => ".".to_owned(),
                    1 if parent => "..".to_owned(),
                    _ => {
                        let path = &folders[i - 1 - usize::from(parent)];
                        ui::clean(path.rsplit('/').next().unwrap_or(path))
                    }
                };
                Row::new([Cell::from(name)]).style(theme.directory())
            },
        );

        let width = usize::from(items_area.width);
        self.items.render(
            frame,
            items_area,
            Some(Header::Title(Line::from("Tag types"))),
            &[Constraint::Fill(1)],
            ITEMS.len(),
            self.focus == Pane::TagTypes,
            theme,
            |i| match ITEMS[i] {
                Item::Separator => Row::new([Cell::from(symbols::line::HORIZONTAL.repeat(width))])
                    .style(theme.border()),
                item => Row::new([Cell::from(item.label())]),
            },
        );

        let item = self.selected_item();
        let rows: Vec<(String, bool)> = self
            .songs
            .iter()
            .map(|song| {
                let text = match item {
                    Item::Field(field) => self.value(song, field).to_owned(),
                    _ => self.file_name(song).to_owned(),
                };
                (text, self.pending.contains_key(&song.file))
            })
            .collect();
        let marks = &self.marks;
        self.tags.render(
            frame,
            tags_area,
            Some(Header::Title(Line::from("Tags"))),
            &[Constraint::Fill(1)],
            rows.len(),
            self.focus == Pane::Tags,
            theme,
            |i| {
                let (text, changed) = &rows[i];
                let mut style = theme.text();
                if *changed {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if marks.contains(&i) {
                    style = style.patch(theme.marked());
                }
                Row::new([Cell::from(ui::clean(text))]).style(style)
            },
        );
    }

    fn handle_action(&mut self, action: Action, ctx: &mut Ctx) -> bool {
        match action {
            Action::SaveTags => {
                self.save(ctx);
                return true;
            }
            Action::RevertTags => {
                self.pending.clear();
                ctx.notify("Discarded pending tag changes");
                return true;
            }
            _ => {}
        }
        match self.focus {
            Pane::Directories => {
                if self.dirs.navigate(action, self.dir_rows()) {
                    self.sync_songs(ctx);
                    return true;
                }
                match action {
                    Action::Activate => self.enter_directory(ctx),
                    Action::Left => {
                        self.back(ctx);
                        self.sync_songs(ctx);
                    }
                    Action::Right => self.focus = Pane::TagTypes,
                    _ => return false,
                }
            }
            Pane::TagTypes => {
                if self.navigate_items(action) {
                    return true;
                }
                match action {
                    Action::Left => self.focus = Pane::Directories,
                    Action::Right if !self.songs.is_empty() => self.focus = Pane::Tags,
                    Action::Activate => self.activate_item(ctx),
                    _ => return false,
                }
            }
            Pane::Tags => {
                if self.tags.navigate(action, self.songs.len()) {
                    return true;
                }
                match action {
                    Action::Left => self.focus = Pane::TagTypes,
                    Action::Activate | Action::Right => self.activate_song(ctx),
                    Action::ToggleMark | Action::AddToQueue => {
                        if let Some(i) = self.tags.selected(self.songs.len()) {
                            if !self.marks.remove(&i) {
                                self.marks.insert(i);
                            }
                            self.tags.navigate(Action::Down, self.songs.len());
                        }
                    }
                    Action::ClearMarks => self.marks.clear(),
                    _ => return false,
                }
            }
        }
        true
    }

    fn on_message(&mut self, message: ScreenMessage, _ctx: &mut Ctx) {
        match (message, self.editing.take()) {
            (ScreenMessage::TagValue { field, value }, Some(Editing::Field(files))) => {
                for file in files {
                    if let Some(index) = self.songs.iter().position(|s| s.file == file) {
                        self.set_value(index, field, value.clone());
                    }
                }
            }
            (ScreenMessage::FileName(name), Some(Editing::FileName(file))) => {
                self.set_file_name(&file, name);
            }
            _ => {}
        }
    }

    fn on_mpd(&mut self, event: &MpdEvent, ctx: &mut Ctx) {
        match event {
            MpdEvent::Reply(Reply::Directory { path, entries }) => {
                if *path == self.path {
                    let mut folders: Vec<String> = entries
                        .iter()
                        .filter_map(|e| match e {
                            DirEntry::Directory(dir) => Some(dir.clone()),
                            _ => None,
                        })
                        .collect();
                    folders.sort_by_cached_key(|f| f.to_lowercase());
                    self.folders = folders;
                    self.loaded = true;
                    if let Some(index) = self.restore.take() {
                        self.dirs.select(index, self.dir_rows());
                    }
                    self.sync_songs(ctx);
                }
                if Some(path) == self.shown.as_ref() {
                    self.songs = entries
                        .iter()
                        .filter_map(|e| match e {
                            DirEntry::Song(song) => Some(song.clone()),
                            _ => None,
                        })
                        .collect();
                }
            }
            MpdEvent::Changed(subsystems)
                if subsystems.contains(&Subsystem::Database) && self.loaded =>
            {
                // Show freshly written tags and renamed files.
                ctx.client.query(Query::Directory(self.path.clone()));
                if let Some(shown) = self.shown.as_ref().filter(|s| **s != self.path) {
                    ctx.client.query(Query::Directory(shown.clone()));
                }
            }
            _ => {}
        }
    }

    fn on_enter(&mut self, ctx: &mut Ctx) {
        if !self.loaded {
            self.load(ctx);
            self.shown = None;
            self.sync_songs(ctx);
        }
    }

    fn on_mouse(&mut self, event: MouseEvent, ctx: &mut Ctx) {
        let position = Position::new(event.column, event.row);
        if self.dirs.contains(position) {
            self.focus = Pane::Directories;
            let click = self.dirs.mouse(event, self.dir_rows());
            self.sync_songs(ctx);
            if let Some(Click::Activate(_)) = click {
                self.enter_directory(ctx);
            }
        } else if self.items.contains(position) {
            self.focus = Pane::TagTypes;
            if let Some(Click::Activate(_)) = self.items.mouse(event, ITEMS.len()) {
                self.activate_item(ctx);
            }
        } else if self.tags.contains(position) {
            self.focus = Pane::Tags;
            if let Some(Click::Activate(_)) = self.tags.mouse(event, self.songs.len()) {
                self.activate_song(ctx);
            }
        }
    }

    fn search(&mut self, query: &str, ctx: &Ctx) -> bool {
        match self.focus {
            Pane::Directories => {
                let mut names = vec![".".to_owned()];
                if self.has_parent() {
                    names.push("..".to_owned());
                }
                names.extend(
                    self.folders
                        .iter()
                        .map(|f| f.rsplit('/').next().unwrap_or(f).to_owned()),
                );
                self.dirs
                    .search(query, names.iter().map(String::as_str), names.len());
                self.sync_songs(ctx);
            }
            Pane::Tags => {
                let len = self.songs.len();
                self.tags
                    .search(query, self.songs.iter().map(Song::file_name), len);
            }
            Pane::TagTypes => return false,
        }
        true
    }
}
