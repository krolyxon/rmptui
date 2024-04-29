#![allow(unused_imports)]
use clap::Parser;
use rmptui::app;
use rmptui::app::App;
use rmptui::app::AppResult;
use rmptui::cli::Args;
use rmptui::cli::Command;
use rmptui::connection::Connection;
use rmptui::event::Event;
use rmptui::event::EventHandler;
use rmptui::handler;
use rmptui::tui;
use std::env;
use std::io;

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    prelude::*,
    symbols::border,
    widgets::{block::*, *},
};

pub type Result<T> = core::result::Result<T, Error>;
pub type Error = Box<dyn std::error::Error>;

fn main() -> AppResult<()> {
    let args = Args::parse();
    let env_host = env::var("MPD_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let env_port = env::var("MPD_PORT").unwrap_or_else(|_| "6600".to_string());
    let mut app = App::builder(format!("{}:{}", env_host, env_port).as_str())?;

    if !args.tui {
        handle_tui(&mut app)?;
    } else {
        match args.command {
            Some(Command::Dmenu) => app.conn.play_dmenu()?,
            Some(Command::Fzf) => app.conn.play_fzf().unwrap(),
            Some(Command::Status) => app.conn.status(),
            Some(Command::Pause) => app.conn.pause(),
            Some(Command::Toggle) => app.conn.toggle_pause(),
            _ => {}
        }
    }
    Ok(())
}

pub fn handle_tui(app: &mut App) -> AppResult<()> {
    let backend = CrosstermBackend::new(io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(1000);

    let mut tui = tui::Tui::new(terminal, events);
    tui.init()?;

    // update the directory
    app.browser.update_directory(&mut app.conn).unwrap();

    while app.running {
        tui.draw(app)?;
        match tui.events.next()? {
            Event::Tick => app.tick(),
            Event::Key(key_event) => handler::handle_key_events(key_event, app)?,
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }

    Ok(())
}
