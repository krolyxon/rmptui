//! The application: owns the state, routes events, and draws the frame.

use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use futures_util::StreamExt;
use ratatui::{DefaultTerminal, Frame, layout::Position};
use tokio::{
    sync::mpsc,
    time::{MissedTickBehavior, interval},
};
use tracing::info;

use crate::{
    action::{Action, Context, ScreenId},
    config::{Config, KeyCombo},
    event::AppEvent,
    lyrics::LyricsService,
    mpd::{self, MpdEvent, MpdHandle, PlayState, Query, Reply, Subsystem, commands},
    popups::{HelpPopup, Popup, PopupState},
    screens::{Ctx, Effect, Screens},
    state::MpdState,
    tags,
    ui::{self, FrameLayout, input::InputEvent, input::LineInput},
    visualizer::Visualizer,
};

struct Message {
    text: String,
    error: bool,
    until: Instant,
}

pub struct App {
    config: Config,
    mpd: MpdState,
    client: MpdHandle,
    events_tx: mpsc::UnboundedSender<AppEvent>,
    events_rx: Option<mpsc::UnboundedReceiver<AppEvent>>,
    screens: Screens,
    active: ScreenId,
    popups: Vec<Box<dyn Popup>>,
    /// The search prompt, while it is open.
    search: Option<LineInput>,
    message: Option<Message>,
    lyrics: LyricsService,
    visualizer: Visualizer,
    layout: Option<FrameLayout>,
    tab_hits: Vec<(u16, u16, ScreenId)>,
    /// The volume before muting.
    muted_volume: Option<u8>,
    queue_requested: bool,
    running: bool,
}

impl App {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let (address, password) = config.mpd_connection()?;
        info!("connecting to MPD at {address}");
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let client = mpd::spawn(address, password, events_tx.clone());
        let visualizer = Visualizer::spawn(config.visualizer.clone());
        let screens = Screens::new(&config, visualizer.subscribe());
        let mut mpd = MpdState::default();
        mpd.music_dir = config.music_directory();
        let mut app = Self {
            lyrics: LyricsService::new(config.lyrics.clone(), events_tx.clone()),
            active: config.ui.startup_screen,
            config,
            mpd,
            client,
            events_tx,
            events_rx: Some(events_rx),
            screens,
            popups: Vec::new(),
            search: None,
            message: None,
            visualizer,
            layout: None,
            tab_hits: Vec::new(),
            muted_volume: None,
            queue_requested: false,
            running: true,
        };
        app.switch_to(app.active);
        Ok(app)
    }

    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        let mut events = self.events_rx.take().expect("run is called once");
        let mut input = EventStream::new();
        let mut spectrum = self.visualizer.subscribe();
        // Redraws the progress bar while playing and expires messages.
        let mut tick = interval(Duration::from_millis(250));
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        while self.running {
            terminal.draw(|frame| self.draw(frame))?;
            tokio::select! {
                event = input.next() => match event {
                    Some(Ok(event)) => self.on_terminal_event(event),
                    Some(Err(error)) => return Err(error.into()),
                    None => break,
                },
                Some(event) = events.recv() => {
                    self.on_app_event(event);
                    // Apply everything that is already queued before redrawing.
                    while let Ok(event) = events.try_recv() {
                        self.on_app_event(event);
                    }
                }
                _ = tick.tick() => {}
                Ok(()) = spectrum.changed(), if self.visualizer.is_active() => {}
            }
        }
        Ok(())
    }

    /// Runs `f` with a context, then applies the effects it queued.
    fn with_ctx<R>(&mut self, f: impl FnOnce(&mut Screens, &mut Ctx) -> R) -> R {
        let mut ctx = Ctx::new(&self.config, &self.mpd, &self.client, &self.lyrics.state);
        let result = f(&mut self.screens, &mut ctx);
        let effects = ctx.effects;
        self.apply_effects(effects);
        result
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Notify { text, error } => self.notify(text, error),
                Effect::Popup(popup) => self.popups.push(popup),
                Effect::Message(id, message) => {
                    self.with_ctx(|screens, ctx| screens.get_mut(id).on_message(message, ctx));
                }
                Effect::WriteTags(writes) => self.write_tags(writes),
                Effect::ReloadLyrics => {
                    self.lyrics.ensure(
                        self.mpd.current.as_ref(),
                        self.mpd.music_dir.as_deref(),
                        true,
                    );
                }
            }
        }
    }

    fn notify(&mut self, text: String, error: bool) {
        self.message = Some(Message {
            text,
            error,
            until: Instant::now() + self.config.message_timeout(),
        });
    }

    fn switch_to(&mut self, screen: ScreenId) {
        self.active = screen;
        self.search = None;
        self.visualizer.set_active(screen == ScreenId::Visualizer);
        self.with_ctx(|screens, ctx| screens.get_mut(screen).on_enter(ctx));
        self.refresh_lyrics();
    }

    fn lyrics_visible(&self) -> bool {
        match self.active {
            ScreenId::Lyrics => true,
            ScreenId::Queue => self.screens.queue.lyrics_pane_visible(),
            _ => false,
        }
    }

    /// Loads lyrics for the current song if they are on screen.
    fn refresh_lyrics(&mut self) {
        if self.lyrics_visible() {
            self.lyrics.ensure(
                self.mpd.current.as_ref(),
                self.mpd.music_dir.as_deref(),
                false,
            );
        }
    }

    // ----- terminal input -------------------------------------------------

    fn on_terminal_event(&mut self, event: Event) {
        match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => self.on_key(key.into()),
            Event::Mouse(mouse) if self.config.ui.mouse => self.on_mouse(mouse),
            _ => {}
        }
    }

    fn on_key(&mut self, key: KeyCombo) {
        if let Some(mut popup) = self.popups.pop() {
            let mut ctx = Ctx::new(&self.config, &self.mpd, &self.client, &self.lyrics.state);
            let state = popup.handle_key(key, &mut ctx);
            let effects = ctx.effects;
            if state == PopupState::Open {
                self.popups.push(popup);
            }
            self.apply_effects(effects);
            return;
        }
        if self.search.is_some() {
            self.on_search_key(key);
            return;
        }
        let contexts = [self.active.context(), Context::Navigation, Context::Global];
        let Some(action) = self.config.keymap.resolve(&contexts, key) else {
            return;
        };
        if action.contexts().contains(&Context::Global) {
            self.global_action(action);
        } else if action == Action::Search {
            self.open_search();
        } else {
            let active = self.active;
            let lyrics_before = self.lyrics_visible();
            self.with_ctx(|screens, ctx| screens.get_mut(active).handle_action(action, ctx));
            if !lyrics_before {
                self.refresh_lyrics();
            }
        }
    }

    fn open_search(&mut self) {
        let active = self.active;
        let ctx = Ctx::new(&self.config, &self.mpd, &self.client, &self.lyrics.state);
        if self.screens.get_mut(active).search("", &ctx) {
            self.search = Some(LineInput::default());
        } else {
            self.notify("Nothing to search on this screen".into(), false);
        }
    }

    fn on_search_key(&mut self, key: KeyCombo) {
        let Some(input) = self.search.as_mut() else {
            return;
        };
        let event = match self.config.keymap.resolve(&[Context::Input], key) {
            Some(action) => input.apply(action),
            None => match key.as_text() {
                Some(ch) => {
                    input.insert(ch);
                    InputEvent::Changed
                }
                None => InputEvent::Ignored,
            },
        };
        let query = input.text().to_owned();
        let active = self.active;
        let ctx = Ctx::new(&self.config, &self.mpd, &self.client, &self.lyrics.state);
        match event {
            InputEvent::Changed => {
                self.screens.get_mut(active).search(&query, &ctx);
            }
            // Keep the results for `n`/`N`.
            InputEvent::Submit => self.search = None,
            InputEvent::Cancel => {
                self.screens.get_mut(active).search("", &ctx);
                self.search = None;
            }
            InputEvent::Moved | InputEvent::Ignored => {}
        }
    }

    fn global_action(&mut self, action: Action) {
        let status = &self.mpd.status;
        let seek = i64::from(self.config.playback.seek_seconds);
        let step = i32::from(self.config.playback.volume_step);
        let client = &self.client;
        match action {
            Action::Quit => self.running = false,
            Action::Help => {
                let ctx = Ctx::new(&self.config, &self.mpd, &self.client, &self.lyrics.state);
                let popup = HelpPopup::new(self.active.context(), &ctx);
                self.popups.push(Box::new(popup));
            }
            Action::TogglePause => client.exec(match status.state {
                PlayState::Play => commands::pause(true),
                PlayState::Pause => commands::pause(false),
                PlayState::Stop => commands::play(),
            }),
            Action::Pause => client.exec(commands::pause(true)),
            Action::Stop => client.exec(commands::stop()),
            Action::NextTrack => client.exec(commands::next()),
            Action::PreviousTrack => client.exec(commands::previous()),
            Action::SeekForward | Action::SeekBackward if status.state != PlayState::Stop => {
                let delta = if action == Action::SeekForward {
                    seek
                } else {
                    -seek
                };
                client.exec(commands::seek_relative(delta));
            }
            Action::SeekForward | Action::SeekBackward => {}
            Action::VolumeUp | Action::VolumeDown => match status.volume {
                Some(_) => {
                    let delta = if action == Action::VolumeUp {
                        step
                    } else {
                        -step
                    };
                    client.exec(commands::change_volume(delta));
                }
                None => self.notify("MPD has no mixer".into(), true),
            },
            Action::ToggleMute => match status.volume {
                Some(0) => {
                    let volume = self.muted_volume.take().unwrap_or(50);
                    client.exec(commands::set_volume(volume));
                }
                Some(volume) => {
                    self.muted_volume = Some(volume);
                    client.exec(commands::set_volume(0));
                }
                None => self.notify("MPD has no mixer".into(), true),
            },
            Action::ToggleRepeat => client.exec(commands::repeat(!status.repeat)),
            Action::ToggleRandom => client.exec(commands::random(!status.random)),
            Action::ToggleSingle => client.exec(commands::single(!status.single.is_on())),
            Action::ToggleConsume => client.exec(commands::consume(!status.consume.is_on())),
            Action::UpdateDatabase => {
                client.exec(commands::update(None));
                self.notify("Updating the database…".into(), false);
            }
            Action::NextScreen => self.switch_to(self.active.next()),
            Action::PreviousScreen => self.switch_to(self.active.previous()),
            Action::ScreenQueue => self.switch_to(ScreenId::Queue),
            Action::ScreenBrowser => self.switch_to(ScreenId::Browser),
            Action::ScreenPlaylists => self.switch_to(ScreenId::Playlists),
            Action::ScreenLibrary => self.switch_to(ScreenId::Library),
            Action::ScreenTagEditor => self.switch_to(ScreenId::TagEditor),
            Action::ScreenVisualizer => self.switch_to(ScreenId::Visualizer),
            Action::ScreenLyrics => self.switch_to(ScreenId::Lyrics),
            _ => {}
        }
    }

    fn on_mouse(&mut self, mouse: MouseEvent) {
        if !self.popups.is_empty() {
            return;
        }
        let Some(layout) = &self.layout else {
            return;
        };
        let position = Position::new(mouse.column, mouse.row);
        let left_click = mouse.kind == MouseEventKind::Down(MouseButton::Left);
        if layout.tabs.contains(position) {
            if left_click
                && let Some(&(_, _, screen)) = self
                    .tab_hits
                    .iter()
                    .find(|(start, end, _)| (*start..*end).contains(&mouse.column))
            {
                self.switch_to(screen);
            }
        } else if layout.progress.contains(position) {
            if left_click
                && self.mpd.status.state != PlayState::Stop
                && let Some(target) =
                    ui::progress_seek_target(layout.progress, mouse.column, &self.mpd)
            {
                self.client.exec(commands::seek_to(target.as_secs_f64()));
            }
        } else if layout.body.contains(position) {
            let active = self.active;
            self.with_ctx(|screens, ctx| screens.get_mut(active).on_mouse(mouse, ctx));
        }
    }

    // ----- background events ----------------------------------------------

    fn on_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Mpd(event) => self.on_mpd_event(event),
            AppEvent::Lyrics { file, result } => {
                self.lyrics.finish(file, result);
            }
            AppEvent::TagsWritten(Ok(dirs)) => {
                let count = dirs.len();
                self.client
                    .exec_all(dirs.iter().map(|dir| commands::update(Some(dir))).collect());
                self.notify(format!("Saved tags; rescanning {count} folder(s)"), false);
            }
            AppEvent::TagsWritten(Err(error)) => self.notify(error, true),
        }
    }

    fn on_mpd_event(&mut self, event: MpdEvent) {
        match &event {
            MpdEvent::Connected { version } => {
                info!("connected to MPD {version}");
                self.mpd.connected = true;
                self.mpd.reset_queue();
                self.queue_requested = false;
                self.client.query(Query::Status);
                self.client.query(Query::Playlists);
                if self.config.music_directory().is_none() {
                    self.client.query(Query::MusicDirectory);
                }
                self.notify(format!("Connected to MPD {version}"), false);
            }
            MpdEvent::Disconnected(reason) => {
                self.mpd.connected = false;
                self.notify(reason.clone(), true);
            }
            MpdEvent::Changed(subsystems) => {
                let mut status = false;
                for subsystem in subsystems {
                    match subsystem {
                        Subsystem::StoredPlaylist => self.client.query(Query::Playlists),
                        Subsystem::Database => {
                            self.mpd.db_version += 1;
                            status = true;
                        }
                        _ => status = true,
                    }
                }
                if status {
                    self.client.query(Query::Status);
                }
            }
            MpdEvent::Reply(Reply::Status { status, current }) => {
                let song_changed = self.mpd.set_status(status.clone(), current.clone());
                self.request_queue_if_stale();
                if song_changed {
                    self.refresh_lyrics();
                }
            }
            MpdEvent::Reply(Reply::QueueChanges {
                status,
                since,
                changes,
            }) => {
                self.queue_requested = false;
                self.mpd
                    .apply_queue_changes(status, *since, changes.clone());
                self.request_queue_if_stale();
            }
            MpdEvent::Reply(Reply::Playlists(playlists)) => {
                self.mpd.playlists.clone_from(playlists)
            }
            MpdEvent::Reply(Reply::MusicDirectory(dir)) => {
                if self.config.music_directory().is_none() {
                    self.mpd.music_dir.clone_from(dir);
                }
            }
            MpdEvent::Reply(_) => {}
            MpdEvent::Error(error) => self.notify(error.clone(), true),
        }
        self.with_ctx(|screens, ctx| screens.for_each(|screen| screen.on_mpd(&event, ctx)));
    }

    fn request_queue_if_stale(&mut self) {
        if !self.queue_requested
            && let Some(since) = self.mpd.stale_queue_since()
        {
            self.queue_requested = true;
            self.client.query(Query::QueueChanges { since });
        }
    }

    fn write_tags(&mut self, writes: Vec<tags::TagWrite>) {
        let events = self.events_tx.clone();
        self.notify(format!("Writing tags of {} file(s)…", writes.len()), false);
        tokio::task::spawn_blocking(move || {
            let mut dirs: Vec<String> = Vec::new();
            let mut errors = Vec::new();
            for write in &writes {
                match tags::write(write) {
                    Ok(()) => {
                        let dir = write
                            .uri
                            .rsplit_once('/')
                            .map_or("", |(dir, _)| dir)
                            .to_owned();
                        if !dirs.contains(&dir) {
                            dirs.push(dir);
                        }
                    }
                    Err(error) => errors.push(error),
                }
            }
            let result = match errors.as_slice() {
                [] => Ok(dirs),
                [error] => Err(error.clone()),
                [first, rest @ ..] => Err(format!("{first} (and {} more errors)", rest.len())),
            };
            let _ = events.send(AppEvent::TagsWritten(result));
        });
    }

    // ----- drawing --------------------------------------------------------

    fn draw(&mut self, frame: &mut Frame) {
        let layout = FrameLayout::new(frame.area());
        let theme = &self.config.theme;
        ui::draw_header(frame, layout.header, &self.mpd, theme);
        self.tab_hits = ui::draw_tabs(frame, layout.tabs, self.active, theme);

        let ctx = Ctx::new(&self.config, &self.mpd, &self.client, &self.lyrics.state);
        self.screens
            .get_mut(self.active)
            .draw(frame, layout.body, &ctx);
        ui::draw_progress(frame, layout.progress, &self.mpd, theme);

        if self
            .message
            .as_ref()
            .is_some_and(|m| m.until <= Instant::now())
        {
            self.message = None;
        }
        if let Some(search) = &self.search {
            search.render(frame, layout.status, " /", theme);
        } else if let Some(message) = &self.message {
            ui::draw_message(frame, layout.status, &message.text, message.error, theme);
        } else {
            let help = self
                .config
                .keymap
                .keys_for(Context::Global, Action::Help)
                .first()
                .map_or_else(String::new, |key| format!("{key} for help"));
            ui::draw_hint(frame, layout.status, &help, theme);
        }
        for popup in &mut self.popups {
            popup.draw(frame, frame.area(), &ctx);
        }
        self.layout = Some(layout);
    }
}
