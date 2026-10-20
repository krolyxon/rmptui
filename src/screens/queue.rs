//! Screen 1: the play queue.

use std::{collections::HashSet, time::Duration};

use crossterm::event::MouseEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::Line,
};

use super::{Ctx, Screen, draw_lyrics};
use crate::{
    action::Action,
    config::Config,
    mpd::{MpdEvent, Reply, Selection, commands},
    popups::{AddToPlaylistPopup, InputPopup, InputPurpose},
    ui::{
        self,
        list::{Click, Header, ListView},
    },
    util::{format_length, plural},
};

pub struct QueueScreen {
    list: ListView,
    /// Marked songs by queue id.
    marks: HashSet<u32>,
    lyrics_pane: bool,
}

impl QueueScreen {
    pub fn new(config: &Config) -> Self {
        Self {
            list: ListView::default(),
            marks: HashSet::new(),
            lyrics_pane: config.ui.lyrics_pane,
        }
    }

    pub fn lyrics_pane_visible(&self) -> bool {
        self.lyrics_pane
    }

    /// Ids of the marked songs in queue order, or of the selected song.
    fn targets(&self, ctx: &Ctx) -> Vec<u32> {
        let queue = &ctx.mpd.queue;
        if self.marks.is_empty() {
            self.list
                .selected(queue.len())
                .and_then(|i| queue[i].id)
                .into_iter()
                .collect()
        } else {
            queue
                .iter()
                .filter_map(|s| s.id.filter(|id| self.marks.contains(id)))
                .collect()
        }
    }

    fn target_files(&self, ctx: &Ctx) -> Vec<String> {
        let ids: HashSet<u32> = self.targets(ctx).into_iter().collect();
        ctx.mpd
            .queue
            .iter()
            .filter(|s| s.id.is_some_and(|id| ids.contains(&id)))
            .map(|s| s.file.clone())
            .collect()
    }

    fn play_selected(&self, ctx: &Ctx) {
        let queue = &ctx.mpd.queue;
        if let Some(id) = self.list.selected(queue.len()).and_then(|i| queue[i].id) {
            ctx.client.exec(commands::play_id(id));
        }
    }

    fn move_selected(&mut self, down: bool, ctx: &Ctx) {
        let len = ctx.mpd.queue.len();
        let Some(index) = self.list.selected(len) else {
            return;
        };
        let target = if down {
            index + 1
        } else {
            index.wrapping_sub(1)
        };
        if target >= len {
            return;
        }
        ctx.client
            .exec(commands::move_pos(index as u32, target as u32));
        self.list.select(target, len);
    }
}

impl Screen for QueueScreen {
    fn title(&self, ctx: &Ctx) -> Line<'static> {
        let queue = &ctx.mpd.queue;
        let total: Duration = queue.iter().filter_map(|s| s.duration).sum();
        Line::from(format!(
            "Playlist ({}, length: {})",
            plural(queue.len(), "item"),
            format_length(total)
        ))
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let theme = ctx.theme();
        let area = if self.lyrics_pane {
            let width = ctx.config.ui.lyrics_pane_width;
            let [queue, lyrics] = <[Rect; 2]>::try_from(ui::split_columns(
                frame,
                area,
                &[
                    Constraint::Percentage(100 - width),
                    Constraint::Percentage(width),
                ],
                theme,
            ))
            .expect("two columns");
            draw_lyrics(frame, lyrics, ctx);
            queue
        } else {
            area
        };

        let queue = &ctx.mpd.queue;
        let columns = &ctx.config.ui.song_columns;
        let marks = &self.marks;
        self.list.render(
            frame,
            area,
            Some(Header::Columns(ui::song_header(columns))),
            &ui::song_widths(columns),
            queue.len(),
            true,
            theme,
            |i| {
                let song = &queue[i];
                let row = ui::song_row(song, columns, ctx.mpd.is_current(song), theme);
                if song.id.is_some_and(|id| marks.contains(&id)) {
                    row.style(theme.marked())
                } else {
                    row
                }
            },
        );
    }

    fn handle_action(&mut self, action: Action, ctx: &mut Ctx) -> bool {
        let len = ctx.mpd.queue.len();
        if self.list.navigate(action, len) {
            return true;
        }
        match action {
            Action::Activate | Action::Right => self.play_selected(ctx),
            Action::Delete | Action::AddToQueue if self.marks.is_empty() => {
                // By position, so that repeated presses delete successive rows
                // even before the queue has been refreshed.
                if let Some(index) = self.list.selected(len) {
                    ctx.client.exec(commands::delete_pos(index as u32));
                }
            }
            Action::Delete | Action::AddToQueue => {
                let ids = self.targets(ctx);
                ctx.client
                    .exec_all(ids.into_iter().map(commands::delete_id).collect());
                self.marks.clear();
            }
            Action::MoveItemUp => self.move_selected(false, ctx),
            Action::MoveItemDown => self.move_selected(true, ctx),
            Action::ShuffleQueue => ctx.client.exec(commands::shuffle()),
            Action::SaveQueue if len > 0 => {
                ctx.popup(InputPopup::new(
                    "Save queue as",
                    "",
                    InputPurpose::SaveQueue,
                ));
            }
            Action::JumpToCurrent => match ctx.mpd.status.song {
                Some(pos) => self.list.select(pos as usize, len),
                None => ctx.notify("Nothing is playing"),
            },
            Action::ToggleLyricsPane => self.lyrics_pane = !self.lyrics_pane,
            Action::ToggleMark => {
                if let Some(id) = self.list.selected(len).and_then(|i| ctx.mpd.queue[i].id) {
                    if !self.marks.remove(&id) {
                        self.marks.insert(id);
                    }
                    self.list.navigate(Action::Down, len);
                }
            }
            Action::ClearMarks => self.marks.clear(),
            Action::AddToPlaylist => {
                let files = self.target_files(ctx);
                if !files.is_empty() {
                    let popup = AddToPlaylistPopup::new(Selection::Uris(files), ctx);
                    ctx.popup(popup);
                }
            }
            _ => return false,
        }
        true
    }

    fn on_mpd(&mut self, event: &MpdEvent, ctx: &mut Ctx) {
        if let MpdEvent::Reply(Reply::QueueChanges { .. }) = event
            && !self.marks.is_empty()
        {
            let ids: HashSet<u32> = ctx.mpd.queue.iter().filter_map(|s| s.id).collect();
            self.marks.retain(|id| ids.contains(id));
        }
    }

    fn on_enter(&mut self, ctx: &mut Ctx) {
        // Start on the playing song the first time.
        if self.list.selected(ctx.mpd.queue.len()) == Some(0)
            && let Some(pos) = ctx.mpd.status.song
        {
            self.list.select(pos as usize, ctx.mpd.queue.len());
        }
    }

    fn on_mouse(&mut self, event: MouseEvent, ctx: &mut Ctx) {
        if let Some(Click::Activate(_)) = self.list.mouse(event, ctx.mpd.queue.len()) {
            self.play_selected(ctx);
        }
    }

    fn search(&mut self, query: &str, ctx: &Ctx) -> bool {
        let texts: Vec<String> = ctx.mpd.queue.iter().map(ui::song_search_text).collect();
        self.list
            .search(query, texts.iter().map(String::as_str), texts.len());
        true
    }
}
