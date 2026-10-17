//! Screen 5: editing tags of local files.
//!
//! Folders on the left, their songs in the middle, the tags of the selected
//! (or all marked) songs on the right. Edits stay pending until saved.

use std::collections::{BTreeMap, HashMap, HashSet};

use crossterm::event::MouseEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    text::{Line, Span},
    widgets::{Cell, Row},
};

use super::{Ctx, Effect, Screen, ScreenMessage};
use crate::{
    action::Action,
    mpd::{DirEntry, MpdEvent, Query, Reply, Song, Subsystem},
    popups::{InputPopup, InputPurpose},
    tags::{TagField, TagWrite},
    ui::{
        self,
        list::{Click, ListView},
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Pane {
    #[default]
    Folders,
    Songs,
    Fields,
}

#[derive(Debug, Default)]
pub struct TagEditorScreen {
    focus: Pane,
    path: String,
    stack: Vec<(String, usize)>,
    restore: Option<usize>,
    loaded: bool,
    folders: Vec<String>,
    songs: Vec<Song>,
    folder_list: ListView,
    song_list: ListView,
    field_list: ListView,
    /// Marked songs by index.
    marks: HashSet<usize>,
    /// Pending edits by file.
    pending: HashMap<String, BTreeMap<TagField, String>>,
}

impl TagEditorScreen {
    fn has_parent(&self) -> bool {
        !self.path.is_empty()
    }

    /// Rows of the folder pane: `..` (if not at the root), then folders.
    fn folder_rows(&self) -> usize {
        self.folders.len() + usize::from(self.has_parent())
    }

    fn load(&mut self, ctx: &Ctx) {
        self.loaded = false;
        ctx.client.query(Query::Directory(self.path.clone()));
    }

    fn open_selected_folder(&mut self, ctx: &Ctx) {
        let Some(index) = self.folder_list.selected(self.folder_rows()) else {
            return;
        };
        if self.has_parent() && index == 0 {
            self.back(ctx);
            return;
        }
        let folder = self.folders[index - usize::from(self.has_parent())].clone();
        self.stack
            .push((std::mem::replace(&mut self.path, folder), index));
        self.reset_lists();
        self.load(ctx);
    }

    fn back(&mut self, ctx: &Ctx) {
        if let Some((parent, index)) = self.stack.pop() {
            self.path = parent;
            self.reset_lists();
            self.restore = Some(index);
            self.load(ctx);
        }
    }

    fn reset_lists(&mut self) {
        self.folder_list.reset();
        self.song_list.reset();
        self.marks.clear();
    }

    /// Indices of the songs an edit applies to: marked ones, or the selected.
    fn targets(&self) -> Vec<usize> {
        if self.marks.is_empty() {
            self.song_list
                .selected(self.songs.len())
                .into_iter()
                .collect()
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
            .and_then(|changes| changes.get(&field))
            .map(String::as_str)
            .unwrap_or_else(|| song.tag(field.mpd_tag()).unwrap_or_default())
    }

    /// The common value of `field` across the targets, or `None` if they differ.
    fn shared_value(&self, field: TagField) -> Option<String> {
        let mut values = self
            .targets()
            .into_iter()
            .map(|i| self.value(&self.songs[i], field));
        let first = values.next()?;
        values.all(|v| v == first).then(|| first.to_owned())
    }

    fn edit_field(&mut self, ctx: &mut Ctx) {
        if self.targets().is_empty() {
            return;
        }
        let Some(index) = self.field_list.selected(TagField::ALL.len()) else {
            return;
        };
        let field = TagField::ALL[index];
        let count = self.targets().len();
        let title = if count > 1 {
            format!("{} ({count} songs)", field.label())
        } else {
            field.label().to_owned()
        };
        let initial = self.shared_value(field).unwrap_or_default();
        ctx.popup(InputPopup::new(
            title,
            &initial,
            InputPurpose::TagValue(field),
        ));
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
        let writes = self
            .pending
            .drain()
            .map(|(uri, changes)| TagWrite {
                path: music_dir.join(&uri),
                uri,
                changes,
            })
            .collect();
        ctx.effects.push(Effect::WriteTags(writes));
    }
}

impl Screen for TagEditorScreen {
    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let theme = ctx.theme();
        if ctx.mpd.music_dir.is_none() {
            let block = ui::pane(" Tag editor ", true, theme);
            let inner = block.inner(area);
            frame.render_widget(block, area);
            ui::draw_placeholder(
                frame,
                inner,
                &[
                    "The tag editor needs access to the music files.",
                    "",
                    "Set `music_directory` in the [mpd] section of the config,",
                    "or connect to MPD through its local Unix socket.",
                ],
                theme,
            );
            return;
        }
        let [folders_area, songs_area, fields_area] = Layout::horizontal([
            Constraint::Percentage(25),
            Constraint::Percentage(40),
            Constraint::Percentage(35),
        ])
        .areas(area);

        let parent = self.has_parent();
        let folders = &self.folders;
        let title = Line::from(vec![
            Span::raw(" Folders "),
            Span::styled(format!("/{} ", self.path), theme.dim()),
        ]);
        self.folder_list.render(
            frame,
            folders_area,
            ui::pane(title, self.focus == Pane::Folders, theme),
            None,
            &[Constraint::Fill(1)],
            folders.len() + usize::from(parent),
            self.focus == Pane::Folders,
            theme,
            |i| {
                let name = if parent && i == 0 {
                    "..".to_owned()
                } else {
                    let path = &folders[i - usize::from(parent)];
                    format!("▸ {}/", path.rsplit('/').next().unwrap_or(path))
                };
                Row::new([Cell::from(name)]).style(theme.directory())
            },
        );

        let songs = &self.songs;
        let pending = &self.pending;
        let marks = &self.marks;
        let title = if pending.is_empty() {
            Line::from(" Songs ")
        } else {
            Line::from(vec![
                Span::raw(" Songs "),
                Span::styled(format!("{} unsaved ", pending.len()), theme.marked()),
            ])
        };
        let block = ui::pane(title, self.focus == Pane::Songs, theme);
        if songs.is_empty() {
            let inner = block.inner(songs_area);
            frame.render_widget(block, songs_area);
            let text = if self.loaded {
                "No songs in this folder"
            } else {
                "Loading…"
            };
            ui::draw_placeholder(frame, inner, &[text], theme);
        } else {
            self.song_list.render(
                frame,
                songs_area,
                block,
                None,
                &[Constraint::Length(1), Constraint::Fill(1)],
                songs.len(),
                self.focus == Pane::Songs,
                theme,
                |i| {
                    let song = &songs[i];
                    let changed = if pending.contains_key(&song.file) {
                        "*"
                    } else {
                        " "
                    };
                    let style = if marks.contains(&i) {
                        theme.marked()
                    } else {
                        theme.text()
                    };
                    Row::new([
                        Cell::from(Span::styled(changed, theme.marked())),
                        Cell::from(song.file_name().to_owned()),
                    ])
                    .style(style)
                },
            );
        }

        let targets = self.targets();
        let title = match targets.len() {
            0 => " Tags ".to_owned(),
            1 => " Tags ".to_owned(),
            n => format!(" Tags · {n} songs "),
        };
        let values: Vec<(TagField, Option<String>)> = TagField::ALL
            .iter()
            .map(|&f| (f, self.shared_value(f)))
            .collect();
        let has_targets = !targets.is_empty();
        self.field_list.render(
            frame,
            fields_area,
            ui::pane(title, self.focus == Pane::Fields, theme),
            None,
            &[Constraint::Length(13), Constraint::Fill(1)],
            values.len(),
            self.focus == Pane::Fields,
            theme,
            |i| {
                let (field, value) = &values[i];
                let value = match value {
                    _ if !has_targets => Span::raw(""),
                    Some(v) => Span::styled(v.clone(), theme.text()),
                    None => Span::styled("<various>", theme.dim()),
                };
                Row::new([
                    Cell::from(Span::styled(field.label(), theme.dim())),
                    Cell::from(value),
                ])
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
            Pane::Folders => {
                if self.folder_list.navigate(action, self.folder_rows()) {
                    return true;
                }
                match action {
                    Action::Activate => self.open_selected_folder(ctx),
                    Action::Left => self.back(ctx),
                    Action::Right if !self.songs.is_empty() => self.focus = Pane::Songs,
                    _ => return false,
                }
            }
            Pane::Songs => {
                if self.song_list.navigate(action, self.songs.len()) {
                    return true;
                }
                match action {
                    Action::Left => self.focus = Pane::Folders,
                    Action::Right | Action::Activate => self.focus = Pane::Fields,
                    Action::ToggleMark | Action::AddToQueue => {
                        if let Some(i) = self.song_list.selected(self.songs.len()) {
                            if !self.marks.remove(&i) {
                                self.marks.insert(i);
                            }
                            self.song_list.navigate(Action::Down, self.songs.len());
                        }
                    }
                    Action::ClearMarks => self.marks.clear(),
                    _ => return false,
                }
            }
            Pane::Fields => {
                if self.field_list.navigate(action, TagField::ALL.len()) {
                    return true;
                }
                match action {
                    Action::Left => self.focus = Pane::Songs,
                    Action::Activate | Action::Right => self.edit_field(ctx),
                    _ => return false,
                }
            }
        }
        true
    }

    fn on_message(&mut self, message: ScreenMessage, _ctx: &mut Ctx) {
        let ScreenMessage::TagValue { field, value } = message;
        for index in self.targets() {
            let song = &self.songs[index];
            let original = song.tag(field.mpd_tag()).unwrap_or_default();
            let changes = self.pending.entry(song.file.clone()).or_default();
            if value == original {
                changes.remove(&field);
            } else {
                changes.insert(field, value.clone());
            }
            if changes.is_empty() {
                self.pending.remove(&song.file);
            }
        }
    }

    fn on_mpd(&mut self, event: &MpdEvent, ctx: &mut Ctx) {
        match event {
            MpdEvent::Reply(Reply::Directory { path, entries }) if *path == self.path => {
                let mut folders = Vec::new();
                let mut songs = Vec::new();
                for entry in entries {
                    match entry {
                        DirEntry::Directory(dir) => folders.push(dir.clone()),
                        DirEntry::Song(song) => songs.push(song.clone()),
                        DirEntry::Playlist(_) => {}
                    }
                }
                folders.sort_by_cached_key(|f| f.to_lowercase());
                self.folders = folders;
                self.songs = songs;
                self.loaded = true;
                if let Some(index) = self.restore.take() {
                    self.folder_list.select(index, self.folder_rows());
                }
            }
            MpdEvent::Changed(subsystems)
                if subsystems.contains(&Subsystem::Database) && self.loaded =>
            {
                // Show freshly written tags.
                ctx.client.query(Query::Directory(self.path.clone()));
            }
            _ => {}
        }
    }

    fn on_enter(&mut self, ctx: &mut Ctx) {
        if !self.loaded {
            self.load(ctx);
        }
    }

    fn on_mouse(&mut self, event: MouseEvent, ctx: &mut Ctx) {
        let position = Position::new(event.column, event.row);
        if self.folder_list.contains(position) {
            self.focus = Pane::Folders;
            if let Some(Click::Activate(_)) = self.folder_list.mouse(event, self.folder_rows()) {
                self.open_selected_folder(ctx);
            }
        } else if self.song_list.contains(position) {
            self.focus = Pane::Songs;
            self.song_list.mouse(event, self.songs.len());
        } else if self.field_list.contains(position) {
            self.focus = Pane::Fields;
            if let Some(Click::Activate(_)) = self.field_list.mouse(event, TagField::ALL.len()) {
                self.edit_field(ctx);
            }
        }
    }

    fn search(&mut self, query: &str, _ctx: &Ctx) -> bool {
        match self.focus {
            Pane::Folders => {
                let mut names: Vec<&str> = Vec::new();
                if self.has_parent() {
                    names.push("..");
                }
                names.extend(
                    self.folders
                        .iter()
                        .map(|f| f.rsplit('/').next().unwrap_or(f)),
                );
                let len = names.len();
                self.folder_list.search(query, names.into_iter(), len);
            }
            Pane::Songs => {
                let len = self.songs.len();
                self.song_list
                    .search(query, self.songs.iter().map(Song::file_name), len);
            }
            Pane::Fields => return false,
        }
        true
    }
}
