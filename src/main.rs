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
    // let args = Args::parse();

    // if args.no_tui {
    // handle_tui()?;
    // } else {
    //     match args.command {
    //         Command::Volume { vol } => {
    //             conn.set_volume(vol);
    //         }
    //         Command::Dmenu => conn.play_dmenu().unwrap(),
    //         Command::Fzf => conn.play_fzf().unwrap(),
    //         Command::Status => conn.status(),
    //         Command::Pause => conn.pause(),
    //         Command::Toggle => conn.toggle_pause(),
    //     };
    // }

 let backend = CrosstermBackend::new(io::stderr());
    let terminal = Terminal::new(backend)?;
    let mut app = App::new("127.0.0.1:6600");
    let events = EventHandler::new(250);

    let mut tui = tui::Tui::new(terminal, events);
    tui.init()?;

    while app.running {
        tui.draw(&mut app)?;
        match tui.events.next()? {
            Event::Tick => app.tick(),
            Event::Key(key_event) => handler::handle_key_events(key_event, &mut app)?,
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        }
    }


    Ok(())
}
