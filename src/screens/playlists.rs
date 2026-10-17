//! Screen 3: stored playlists and their contents.

use crossterm::event::MouseEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Position, Rect},
    text::Line,
    widgets::{Cell, Row},
};

use super::{Ctx, Screen};
use crate::{
    action::Action,
    mpd::{MpdEvent, Query, Reply, Request, Selection, Song, Subsystem, commands},
    popups::{AddToPlaylistPopup, ConfirmPopup, InputPopup, InputPurpose},
    ui::{
        self,
        list::{Click, Header, ListView},
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Focus {
    #[default]
    Playlists,
    Songs,
}

#[derive(Debug, Default)]
pub struct PlaylistsScreen {
    focus: Focus,
    playlists: ListView,
    songs: ListView,
    /// Contents of `songs_of`.
    contents: Vec<Song>,
    songs_of: Option<String>,
}

impl PlaylistsScreen {
    fn selected_name<'a>(&self, ctx: &Ctx<'a>) -> Option<&'a str> {
        let playlists = &ctx.mpd.playlists;
        self.playlists
            .selected(playlists.len())
            .map(|i| playlists[i].name.as_str())
    }

    /// Requests the contents of the selected playlist if they are not shown.
    fn sync_contents(&mut self, ctx: &Ctx) {
        let name = self.selected_name(ctx);
        if name != self.songs_of.as_deref() {
            self.songs_of = name.map(str::to_owned);
            self.contents.clear();
            self.songs.reset();
            if let Some(name) = name {
                ctx.client.query(Query::PlaylistSongs(name.to_owned()));
            }
        }
    }

    fn selected_song(&self) -> Option<(usize, &Song)> {
        self.songs
            .selected(self.contents.len())
            .map(|i| (i, &self.contents[i]))
    }

    fn move_song(&mut self, down: bool, ctx: &Ctx) {
        let (Some(name), Some((index, _))) = (self.songs_of.as_deref(), self.selected_song())
        else {
            return;
        };
        let target = if down {
            index + 1
        } else {
            index.wrapping_sub(1)
        };
        if target < self.contents.len() {
            ctx.client
                .exec(commands::playlist_move(name, index as u32, target as u32));
            self.songs.select(target, self.contents.len());
        }
    }

    fn playlist_action(&mut self, action: Action, ctx: &mut Ctx) -> bool {
        let Some(name) = self.selected_name(ctx).map(str::to_owned) else {
            return false;
        };
        match action {
            Action::DeletePlaylist => ctx.popup(ConfirmPopup::new(
                format!("Delete the playlist “{name}”?"),
                vec![commands::remove_playlist(&name)],
            )),
            Action::RenamePlaylist => ctx.popup(InputPopup::new(
                "Rename playlist",
                &name,
                InputPurpose::RenamePlaylist(name.clone()),
            )),
            _ => return false,
        }
        true
    }
}

impl Screen for PlaylistsScreen {
    fn title(&self, _ctx: &Ctx) -> Line<'static> {
        Line::from("Playlist editor")
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let theme = ctx.theme();
        let [left, right] = <[Rect; 2]>::try_from(ui::split_columns(
            frame,
            area,
            &[Constraint::Percentage(25), Constraint::Percentage(75)],
            theme,
        ))
        .expect("two columns");

        let playlists = &ctx.mpd.playlists;
        let list_focused = self.focus == Focus::Playlists;
        self.playlists.render(
            frame,
            left,
            Some(Header::Title(Line::from("Playlists"))),
            &[Constraint::Fill(1)],
            playlists.len(),
            list_focused,
            theme,
            |i| Row::new([Cell::from(ui::clean(&playlists[i].name))]),
        );

        let contents = &self.contents;
        let columns = &ctx.config.ui.song_columns;
        self.songs.render(
            frame,
            right,
            Some(Header::Columns(ui::song_header(columns))),
            &ui::song_widths(columns),
            contents.len(),
            !list_focused,
            theme,
            |i| {
                let song = &contents[i];
                let playing = ctx
                    .mpd
                    .current
                    .as_ref()
                    .is_some_and(|c| c.file == song.file);
                ui::song_row(song, columns, playing, theme)
            },
        );
    }

    fn handle_action(&mut self, action: Action, ctx: &mut Ctx) -> bool {
        if self.playlist_action(action, ctx) {
            return true;
        }
        match self.focus {
            Focus::Playlists => {
                if self.playlists.navigate(action, ctx.mpd.playlists.len()) {
                    self.sync_contents(ctx);
                    return true;
                }
                let Some(name) = self.selected_name(ctx).map(str::to_owned) else {
                    return false;
                };
                match action {
                    Action::Activate => {
                        ctx.client.request(Request::LoadAndPlay {
                            name: name.clone(),
                            offset: 0,
                        });
                        ctx.notify(format!("Loaded playlist “{name}”"));
                    }
                    Action::AddToQueue => {
                        ctx.client.exec(commands::load(&name));
                        ctx.notify(format!("Added playlist “{name}” to the queue"));
                    }
                    Action::Right if !self.contents.is_empty() => self.focus = Focus::Songs,
                    Action::AddToPlaylist if !self.contents.is_empty() => {
                        let files = self.contents.iter().map(|s| s.file.clone()).collect();
                        let popup = AddToPlaylistPopup::new(Selection::Uris(files), ctx);
                        ctx.popup(popup);
                    }
                    _ => return false,
                }
            }
            Focus::Songs => {
                if self.songs.navigate(action, self.contents.len()) {
                    return true;
                }
                let Some(name) = self.songs_of.clone() else {
                    return false;
                };
                match action {
                    Action::Left => self.focus = Focus::Playlists,
                    Action::Activate | Action::Right => {
                        if let Some((index, _)) = self.selected_song() {
                            ctx.client.request(Request::LoadAndPlay {
                                name,
                                offset: index as u32,
                            });
                        }
                    }
                    Action::AddToQueue => {
                        if let Some((_, song)) = self.selected_song() {
                            ctx.client.exec(commands::add(&song.file));
                            ctx.notify(format!("Added “{}” to the queue", song.display_title()));
                        }
                        self.songs.navigate(Action::Down, self.contents.len());
                    }
                    Action::Delete => {
                        if let Some((index, _)) = self.selected_song() {
                            ctx.client
                                .exec(commands::playlist_delete(&name, index as u32));
                        }
                    }
                    Action::MoveItemUp => self.move_song(false, ctx),
                    Action::MoveItemDown => self.move_song(true, ctx),
                    Action::AddToPlaylist => {
                        if let Some((_, song)) = self.selected_song() {
                            let selection = Selection::Uris(vec![song.file.clone()]);
                            let popup = AddToPlaylistPopup::new(selection, ctx);
                            ctx.popup(popup);
                        }
                    }
                    _ => return false,
                }
            }
        }
        true
    }

    fn on_mpd(&mut self, event: &MpdEvent, ctx: &mut Ctx) {
        match event {
            MpdEvent::Reply(Reply::PlaylistSongs { name, songs })
                if Some(name) == self.songs_of.as_ref() =>
            {
                self.contents.clone_from(songs);
                if self.contents.is_empty() {
                    self.focus = Focus::Playlists;
                }
            }
            MpdEvent::Reply(Reply::Playlists(_)) => {
                // Follow the selected playlist when names change (e.g. a rename).
                self.sync_contents(ctx);
            }
            MpdEvent::Changed(subsystems) if subsystems.contains(&Subsystem::StoredPlaylist) => {
                if let Some(name) = &self.songs_of {
                    ctx.client.query(Query::PlaylistSongs(name.clone()));
                }
            }
            _ => {}
        }
    }

    fn on_enter(&mut self, ctx: &mut Ctx) {
        self.sync_contents(ctx);
    }

    fn on_mouse(&mut self, event: MouseEvent, ctx: &mut Ctx) {
        let position = Position::new(event.column, event.row);
        if self.playlists.contains(position) {
            self.focus = Focus::Playlists;
            let click = self.playlists.mouse(event, ctx.mpd.playlists.len());
            self.sync_contents(ctx);
            if let Some(Click::Activate(_)) = click {
                self.handle_action(Action::Activate, ctx);
            }
        } else if self.songs.contains(position) {
            self.focus = Focus::Songs;
            if let Some(Click::Activate(_)) = self.songs.mouse(event, self.contents.len()) {
                self.handle_action(Action::Activate, ctx);
            }
        }
    }

    fn search(&mut self, query: &str, ctx: &Ctx) -> bool {
        match self.focus {
            Focus::Playlists => {
                let playlists = &ctx.mpd.playlists;
                self.playlists.search(
                    query,
                    playlists.iter().map(|p| p.name.as_str()),
                    playlists.len(),
                );
                self.sync_contents(ctx);
            }
            Focus::Songs => {
                let texts: Vec<String> = self.contents.iter().map(ui::song_search_text).collect();
                self.songs
                    .search(query, texts.iter().map(String::as_str), texts.len());
            }
        }
        true
    }
}
