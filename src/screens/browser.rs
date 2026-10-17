//! Screen 2: the directory browser. Stored playlists appear at the bottom of
//! the root directory.

use std::collections::{HashMap, HashSet};

use crossterm::event::MouseEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Cell, Row},
};

use super::{Ctx, Screen};
use crate::{
    action::Action,
    mpd::{DirEntry, MpdEvent, Query, Reply, Request, Selection, Subsystem, commands},
    popups::AddToPlaylistPopup,
    ui::{
        self,
        list::{Click, ListView},
    },
    util::format_duration,
};

#[derive(Debug, Default)]
pub struct BrowserScreen {
    path: String,
    entries: Vec<DirEntry>,
    loaded: bool,
    /// Parent directories and the cursor position in each.
    stack: Vec<(String, usize)>,
    /// The cursor position to restore once the directory has loaded.
    restore: Option<usize>,
    list: ListView,
    marks: HashSet<usize>,
    cache: HashMap<String, Vec<DirEntry>>,
}

impl BrowserScreen {
    fn load(&mut self, ctx: &Ctx) {
        if let Some(entries) = self.cache.get(&self.path) {
            self.entries = entries.clone();
            self.loaded = true;
            self.apply_restore();
        } else {
            self.loaded = false;
            ctx.client.query(Query::Directory(self.path.clone()));
        }
    }

    fn apply_restore(&mut self) {
        if let Some(index) = self.restore.take() {
            self.list.select(index, self.entries.len());
        }
    }

    fn open(&mut self, path: String, ctx: &Ctx) {
        let index = self.list.selected(self.entries.len()).unwrap_or(0);
        self.stack
            .push((std::mem::replace(&mut self.path, path), index));
        self.list.reset();
        self.marks.clear();
        self.load(ctx);
    }

    fn back(&mut self, ctx: &Ctx) {
        if let Some((parent, index)) = self.stack.pop() {
            self.path = parent;
            self.list.reset();
            self.marks.clear();
            self.restore = Some(index);
            self.load(ctx);
        }
    }

    fn selected(&self) -> Option<&DirEntry> {
        self.list
            .selected(self.entries.len())
            .map(|i| &self.entries[i])
    }

    /// Marked entries, or the selected one, excluding playlists.
    fn selection(&self) -> Vec<String> {
        let entries: Vec<&DirEntry> = if self.marks.is_empty() {
            self.selected().into_iter().collect()
        } else {
            let mut marks: Vec<_> = self.marks.iter().copied().collect();
            marks.sort_unstable();
            marks.iter().filter_map(|&i| self.entries.get(i)).collect()
        };
        entries
            .into_iter()
            .filter(|e| !matches!(e, DirEntry::Playlist(_)))
            .map(|e| e.path().to_owned())
            .collect()
    }

    fn activate(&mut self, ctx: &mut Ctx) {
        let Some(entry) = self.selected().cloned() else {
            return;
        };
        match entry {
            DirEntry::Directory(path) => self.open(path, ctx),
            DirEntry::Song(song) => match ctx.mpd.queued_id(&song.file) {
                Some(id) => ctx.client.exec(commands::play_id(id)),
                None => ctx.client.request(Request::AddAndPlay(vec![song.file])),
            },
            DirEntry::Playlist(name) => {
                ctx.client.request(Request::LoadAndPlay {
                    name: name.clone(),
                    offset: 0,
                });
                ctx.notify(format!("Loaded playlist “{name}”"));
            }
        }
    }

    fn add_to_queue(&mut self, ctx: &mut Ctx) {
        if !self.marks.is_empty() {
            let uris = self.selection();
            let count = uris.len();
            ctx.client.exec_all(Selection::Uris(uris).add_to_queue());
            ctx.notify(format!("Added {count} item(s) to the queue"));
            self.marks.clear();
            return;
        }
        let Some(entry) = self.selected().cloned() else {
            return;
        };
        match entry {
            DirEntry::Directory(ref path) => {
                ctx.client.exec(commands::add(path));
                ctx.notify(format!("Added “{}” to the queue", entry.name()));
            }
            // Space toggles a song in and out of the queue.
            DirEntry::Song(song) => {
                let ids: Vec<u32> = ctx
                    .mpd
                    .queue
                    .iter()
                    .filter(|q| q.file == song.file)
                    .filter_map(|q| q.id)
                    .collect();
                if ids.is_empty() {
                    ctx.client.exec(commands::add(&song.file));
                } else {
                    ctx.client
                        .exec_all(ids.into_iter().map(commands::delete_id).collect());
                }
            }
            DirEntry::Playlist(name) => {
                ctx.client.exec(commands::load(&name));
                ctx.notify(format!("Added playlist “{name}” to the queue"));
            }
        }
        self.list.navigate(Action::Down, self.entries.len());
    }
}

/// Directories first, then songs in server order, then playlists.
fn sort_entries(entries: &mut [DirEntry]) {
    let rank = |e: &DirEntry| match e {
        DirEntry::Directory(_) => 0,
        DirEntry::Song(_) => 1,
        DirEntry::Playlist(_) => 2,
    };
    entries.sort_by(|a, b| {
        rank(a).cmp(&rank(b)).then_with(|| match (a, b) {
            (DirEntry::Song(_), DirEntry::Song(_)) => std::cmp::Ordering::Equal,
            _ => a.name().to_lowercase().cmp(&b.name().to_lowercase()),
        })
    });
}

impl Screen for BrowserScreen {
    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let theme = ctx.theme();
        let title = Line::from(vec![
            Span::raw(" Browse "),
            Span::styled(format!("/{} ", self.path), theme.dim()),
        ]);
        if self.entries.is_empty() {
            let block = ui::pane(title, true, theme);
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let text = if self.loaded {
                "This folder is empty"
            } else {
                "Loading…"
            };
            ui::draw_placeholder(frame, inner, &[text], theme);
            return;
        }
        let entries = &self.entries;
        let marks = &self.marks;
        self.list.render(
            frame,
            area,
            ui::pane(title, true, theme),
            None,
            &[Constraint::Fill(1), Constraint::Length(8)],
            entries.len(),
            true,
            theme,
            |i| {
                let (label, duration, style) = match &entries[i] {
                    DirEntry::Directory(_) => (
                        format!("▸ {}/", entries[i].name()),
                        String::new(),
                        theme.directory(),
                    ),
                    DirEntry::Song(song) => {
                        let style = if ctx.mpd.is_queued(&song.file) {
                            theme.text().add_modifier(Modifier::BOLD)
                        } else {
                            theme.text()
                        };
                        (
                            format!("  {}", ui::song_label(song)),
                            song.duration.map(format_duration).unwrap_or_default(),
                            style,
                        )
                    }
                    DirEntry::Playlist(name) => {
                        (format!("≡ {name}"), String::new(), theme.accent())
                    }
                };
                let style = if marks.contains(&i) {
                    theme.marked()
                } else {
                    style
                };
                Row::new([
                    Cell::from(label),
                    Cell::from(Span::styled(duration, theme.dim())),
                ])
                .style(style)
            },
        );
    }

    fn handle_action(&mut self, action: Action, ctx: &mut Ctx) -> bool {
        if self.list.navigate(action, self.entries.len()) {
            return true;
        }
        match action {
            Action::Activate | Action::Right => self.activate(ctx),
            Action::Left => self.back(ctx),
            Action::AddToQueue => self.add_to_queue(ctx),
            Action::AddToPlaylist => {
                let uris = self.selection();
                if !uris.is_empty() {
                    let popup = AddToPlaylistPopup::new(Selection::Uris(uris), ctx);
                    ctx.popup(popup);
                    self.marks.clear();
                }
            }
            Action::ToggleMark => {
                if let Some(i) = self.list.selected(self.entries.len()) {
                    if !self.marks.remove(&i) {
                        self.marks.insert(i);
                    }
                    self.list.navigate(Action::Down, self.entries.len());
                }
            }
            Action::ClearMarks => self.marks.clear(),
            _ => return false,
        }
        true
    }

    fn on_mpd(&mut self, event: &MpdEvent, ctx: &mut Ctx) {
        match event {
            MpdEvent::Reply(Reply::Directory { path, entries }) => {
                let mut entries = entries.clone();
                sort_entries(&mut entries);
                if *path == self.path {
                    self.entries = entries.clone();
                    self.loaded = true;
                    self.apply_restore();
                }
                self.cache.insert(path.clone(), entries);
            }
            MpdEvent::Changed(subsystems) => {
                let database = subsystems.contains(&Subsystem::Database);
                let playlists = subsystems.contains(&Subsystem::StoredPlaylist);
                if database || playlists {
                    self.cache.clear();
                    if self.loaded && (database || self.path.is_empty()) {
                        // Keep the cursor where it is after the reload.
                        self.restore = self.list.selected(self.entries.len());
                        ctx.client.query(Query::Directory(self.path.clone()));
                    }
                }
            }
            MpdEvent::Connected { .. } => {
                self.cache.clear();
                if self.loaded {
                    ctx.client.query(Query::Directory(self.path.clone()));
                }
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
        if let Some(Click::Activate(_)) = self.list.mouse(event, self.entries.len()) {
            self.activate(ctx);
        }
    }

    fn search(&mut self, query: &str, _ctx: &Ctx) -> bool {
        // Match what is displayed: names for folders, labels for songs.
        let names: Vec<String> = self
            .entries
            .iter()
            .map(|entry| match entry {
                DirEntry::Song(song) => ui::song_label(song),
                other => other.name().to_owned(),
            })
            .collect();
        self.list
            .search(query, names.iter().map(String::as_str), names.len());
        true
    }
}
