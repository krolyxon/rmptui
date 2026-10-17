//! Screen 4: the media library — artists, their albums, and album songs.

use crossterm::event::MouseEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::Modifier,
    text::Span,
    widgets::{Cell, Row},
};

use super::{Ctx, Screen};
use crate::{
    action::Action,
    mpd::{
        Album, MpdEvent, Query, Reply, Request, Selection, Song, Subsystem, commands,
        protocol::{filter_and, filter_eq},
    },
    popups::AddToPlaylistPopup,
    ui::{
        self,
        list::{Click, ListView},
    },
    util::format_duration,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum Column {
    #[default]
    Artists,
    Albums,
    Songs,
}

#[derive(Debug, Default)]
pub struct LibraryScreen {
    /// Group by `Artist` instead of `AlbumArtist`.
    use_artist: bool,
    focus: Column,
    artists: Vec<String>,
    artists_loaded: bool,
    artist_list: ListView,
    albums: Vec<Album>,
    albums_of: Option<String>,
    album_list: ListView,
    songs: Vec<Song>,
    songs_filter: Option<String>,
    song_list: ListView,
}

impl LibraryScreen {
    fn tag(&self) -> &'static str {
        if self.use_artist {
            "Artist"
        } else {
            "AlbumArtist"
        }
    }

    fn reload(&mut self, ctx: &Ctx) {
        self.artists_loaded = false;
        self.albums_of = None;
        self.songs_filter = None;
        ctx.client.query(Query::TagValues(self.tag().to_owned()));
    }

    fn selected_artist(&self) -> Option<&str> {
        self.artist_list
            .selected(self.artists.len())
            .map(|i| self.artists[i].as_str())
    }

    fn selected_album(&self) -> Option<&Album> {
        self.album_list
            .selected(self.albums.len())
            .map(|i| &self.albums[i])
    }

    fn artist_filter(&self) -> Option<String> {
        self.selected_artist()
            .map(|artist| filter_eq(self.tag(), artist))
    }

    fn album_filter(&self) -> Option<String> {
        let artist = self.artist_filter()?;
        let album = self.selected_album()?;
        let mut parts = vec![artist, filter_eq("Album", &album.name)];
        if !album.date.is_empty() {
            parts.push(filter_eq("Date", &album.date));
        }
        Some(filter_and(parts))
    }

    /// Requests albums and songs for the current selection when it changed.
    fn sync(&mut self, ctx: &Ctx) {
        let artist = self.selected_artist().map(str::to_owned);
        if artist != self.albums_of {
            self.albums_of.clone_from(&artist);
            self.albums.clear();
            self.album_list.reset();
            self.songs.clear();
            self.songs_filter = None;
            if let Some(artist) = artist {
                ctx.client.query(Query::Albums {
                    tag: self.tag().to_owned(),
                    value: artist,
                });
            }
            return;
        }
        let filter = self.album_filter();
        if filter != self.songs_filter {
            self.songs_filter.clone_from(&filter);
            self.songs.clear();
            self.song_list.reset();
            if let Some(filter) = filter {
                ctx.client.query(Query::Find(filter));
            }
        }
    }

    fn focused_list(&mut self) -> (&mut ListView, usize) {
        match self.focus {
            Column::Artists => (&mut self.artist_list, self.artists.len()),
            Column::Albums => (&mut self.album_list, self.albums.len()),
            Column::Songs => (&mut self.song_list, self.songs.len()),
        }
    }

    fn selected_song(&self) -> Option<&Song> {
        self.song_list
            .selected(self.songs.len())
            .map(|i| &self.songs[i])
    }

    /// The songs the focused column refers to.
    fn selection(&self) -> Option<Selection> {
        match self.focus {
            Column::Artists => self.artist_filter().map(Selection::Filter),
            Column::Albums => self.album_filter().map(Selection::Filter),
            Column::Songs => self
                .selected_song()
                .map(|s| Selection::Uris(vec![s.file.clone()])),
        }
    }

    fn activate(&mut self, ctx: &mut Ctx) {
        match self.focus {
            Column::Artists if !self.albums.is_empty() => self.focus = Column::Albums,
            Column::Artists => {}
            Column::Albums => {
                if let Some(filter) = self.album_filter() {
                    ctx.client.request(Request::FindAddAndPlay(filter));
                }
            }
            Column::Songs => {
                if let Some(song) = self.selected_song() {
                    match ctx.mpd.queued_id(&song.file) {
                        Some(id) => ctx.client.exec(commands::play_id(id)),
                        None => ctx
                            .client
                            .request(Request::AddAndPlay(vec![song.file.clone()])),
                    }
                }
            }
        }
    }
}

fn unknown(value: &str) -> &str {
    if value.is_empty() { "<unknown>" } else { value }
}

impl Screen for LibraryScreen {
    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let theme = ctx.theme();
        let [artists_area, albums_area, songs_area] = Layout::horizontal([
            Constraint::Percentage(28),
            Constraint::Percentage(32),
            Constraint::Percentage(40),
        ])
        .areas(area);

        let artists = &self.artists;
        let title = if self.use_artist {
            " Artists "
        } else {
            " Album Artists "
        };
        let block = ui::pane(title, self.focus == Column::Artists, theme);
        if artists.is_empty() {
            let inner = block.inner(artists_area);
            frame.render_widget(block, artists_area);
            let text = if self.artists_loaded {
                "No artists"
            } else {
                "Loading…"
            };
            ui::draw_placeholder(frame, inner, &[text], theme);
        } else {
            self.artist_list.render(
                frame,
                artists_area,
                block,
                None,
                &[Constraint::Fill(1)],
                artists.len(),
                self.focus == Column::Artists,
                theme,
                |i| Row::new([Cell::from(unknown(&artists[i]).to_owned())]),
            );
        }

        let albums = &self.albums;
        let block = ui::pane(" Albums ", self.focus == Column::Albums, theme);
        if albums.is_empty() {
            frame.render_widget(block, albums_area);
        } else {
            self.album_list.render(
                frame,
                albums_area,
                block,
                None,
                &[Constraint::Length(6), Constraint::Fill(1)],
                albums.len(),
                self.focus == Column::Albums,
                theme,
                |i| {
                    let album = &albums[i];
                    Row::new([
                        Cell::from(Span::styled(
                            album.date.chars().take(4).collect::<String>(),
                            theme.dim(),
                        )),
                        Cell::from(unknown(&album.name).to_owned()),
                    ])
                },
            );
        }

        let songs = &self.songs;
        let block = ui::pane(" Songs ", self.focus == Column::Songs, theme);
        if songs.is_empty() {
            frame.render_widget(block, songs_area);
        } else {
            self.song_list.render(
                frame,
                songs_area,
                block,
                None,
                &[
                    Constraint::Length(3),
                    Constraint::Fill(1),
                    Constraint::Length(7),
                ],
                songs.len(),
                self.focus == Column::Songs,
                theme,
                |i| {
                    let song = &songs[i];
                    let style = if ctx.mpd.is_queued(&song.file) {
                        theme.text().add_modifier(Modifier::BOLD)
                    } else {
                        theme.text()
                    };
                    Row::new([
                        Cell::from(Span::styled(
                            song.track().unwrap_or_default().to_owned(),
                            theme.dim(),
                        )),
                        Cell::from(song.display_title().to_owned()),
                        Cell::from(Span::styled(
                            song.duration.map(format_duration).unwrap_or_default(),
                            theme.dim(),
                        )),
                    ])
                    .style(style)
                },
            );
        }
    }

    fn handle_action(&mut self, action: Action, ctx: &mut Ctx) -> bool {
        let (list, len) = self.focused_list();
        if list.navigate(action, len) {
            self.sync(ctx);
            return true;
        }
        match action {
            Action::Left => {
                self.focus = match self.focus {
                    Column::Songs => Column::Albums,
                    _ => Column::Artists,
                }
            }
            Action::Right => match self.focus {
                Column::Artists if !self.albums.is_empty() => self.focus = Column::Albums,
                Column::Albums if !self.songs.is_empty() => self.focus = Column::Songs,
                _ => {}
            },
            Action::Activate => self.activate(ctx),
            Action::AddToQueue => {
                if let Some(selection) = self.selection() {
                    ctx.client.exec_all(selection.add_to_queue());
                    ctx.notify("Added to the queue");
                    if self.focus == Column::Songs {
                        self.song_list.navigate(Action::Down, self.songs.len());
                    }
                }
            }
            Action::AddToPlaylist => {
                if let Some(selection) = self.selection() {
                    let popup = AddToPlaylistPopup::new(selection, ctx);
                    ctx.popup(popup);
                }
            }
            Action::ToggleArtistTag => {
                self.use_artist = !self.use_artist;
                self.focus = Column::Artists;
                self.artists.clear();
                self.artist_list.reset();
                self.reload(ctx);
            }
            _ => return false,
        }
        true
    }

    fn on_mpd(&mut self, event: &MpdEvent, ctx: &mut Ctx) {
        match event {
            MpdEvent::Reply(Reply::TagValues { tag, values }) if tag == self.tag() => {
                self.artists.clone_from(values);
                self.artists_loaded = true;
                self.sync(ctx);
            }
            MpdEvent::Reply(Reply::Albums { tag, value, albums })
                if tag == self.tag() && Some(value) == self.albums_of.as_ref() =>
            {
                self.albums.clone_from(albums);
                self.sync(ctx);
            }
            MpdEvent::Reply(Reply::Find { filter, songs })
                if Some(filter) == self.songs_filter.as_ref() =>
            {
                self.songs.clone_from(songs);
            }
            MpdEvent::Changed(subsystems)
                if subsystems.contains(&Subsystem::Database) && self.artists_loaded =>
            {
                self.reload(ctx);
            }
            _ => {}
        }
    }

    fn on_enter(&mut self, ctx: &mut Ctx) {
        if !self.artists_loaded {
            self.reload(ctx);
        }
    }

    fn on_mouse(&mut self, event: MouseEvent, ctx: &mut Ctx) {
        let position = Position::new(event.column, event.row);
        let column = [Column::Artists, Column::Albums, Column::Songs]
            .into_iter()
            .find(|&c| {
                let list = match c {
                    Column::Artists => &self.artist_list,
                    Column::Albums => &self.album_list,
                    Column::Songs => &self.song_list,
                };
                list.contains(position)
            });
        let Some(column) = column else {
            return;
        };
        self.focus = column;
        let (list, len) = self.focused_list();
        let click = list.mouse(event, len);
        self.sync(ctx);
        if let Some(Click::Activate(_)) = click {
            self.activate(ctx);
        }
    }

    fn search(&mut self, query: &str, ctx: &Ctx) -> bool {
        match self.focus {
            Column::Artists => {
                self.artist_list.search(
                    query,
                    self.artists.iter().map(String::as_str),
                    self.artists.len(),
                );
            }
            Column::Albums => {
                self.album_list.search(
                    query,
                    self.albums.iter().map(|a| a.name.as_str()),
                    self.albums.len(),
                );
            }
            Column::Songs => {
                self.song_list.search(
                    query,
                    self.songs.iter().map(Song::display_title),
                    self.songs.len(),
                );
            }
        }
        self.sync(ctx);
        true
    }
}
