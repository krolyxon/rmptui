use ratatui::prelude::*;
use rmptui::app::App;
use rmptui::app::AppResult;
use rmptui::event::event::Event;
use rmptui::event::event::EventHandler;
use rmptui::event::handler;
use rmptui::tui;
use std::env;
use std::io;

fn main() -> AppResult<()> {
    // Connection
    let env_host = env::var("MPD_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let env_port = env::var("MPD_PORT").unwrap_or_else(|_| "6600".to_string());
    let url = format!("{}:{}", env_host, env_port);
    let mut app = App::builder(&url)?;

    // UI
    let backend = CrosstermBackend::new(io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(1000);
    let mut tui = tui::Tui::new(terminal, events);
    tui.init()?;

    // initial directory read
    app.browser.update_directory(&mut app.conn)?;

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
