use std::{env, io, net::Ipv6Addr, str::FromStr, time::Duration};

use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEventKind};
use futures_util::StreamExt;
use ratatui::prelude::*;
use rmptui::{
    app::App,
    config::Config,
    connection::{Connection, MpdSnapshot},
    event_handler::handler,
    logging, tui, AppResult,
};
use tokio::{sync::mpsc, time};

#[tokio::main]
async fn main() -> AppResult<()> {
    let log_path = std::path::Path::new("/tmp/rmptui.log");
    if let Err(error) = logging::init(log_path) {
        eprintln!(
            "Could not initialize logging at {}: {error}",
            log_path.display()
        );
    }

    let result = run().await;
    if let Err(error) = &result {
        logging::error(&format!("application exited with error: {error}"));
    }
    result
}

async fn run() -> AppResult<()> {
    let (config, config_path) = Config::load_or_create()?;
    eprintln!("Using configuration: {}", config_path.display());

    let configured_host = env::var("MPD_HOST").unwrap_or_else(|_| config.mpd.host.clone());
    let (host, password) = configured_host
        .split_once('@')
        .map(|(password, host)| (host, Some(password.to_string())))
        .unwrap_or_else(|| {
            (
                configured_host.as_str(),
                (!config.mpd.password.is_empty()).then(|| config.mpd.password.clone()),
            )
        });
    if host.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "MPD host cannot be empty").into());
    }
    let port = env::var("MPD_PORT")
        .map(|port| {
            port.parse::<u16>().map_err(|error| {
                io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid MPD_PORT value `{port}`: {error}"),
                )
            })
        })
        .unwrap_or(Ok(config.mpd.port))?;
    if port == 0 {
        return Err(io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "MPD port must be between 1 and 65535",
        )
        .into());
    }
    let address = mpd_address(host, port);
    let mut app = App::new(&address, password.as_deref(), config).await?;

    let backend = CrosstermBackend::new(io::stderr());
    let terminal = Terminal::new(backend)?;
    let mut tui = tui::Tui::new(terminal);
    tui.init()?;

    let (updates_tx, mut updates_rx) = mpsc::channel(16);
    let watcher_address = address.clone();
    let watcher_password = password.clone();
    let watcher = tokio::spawn(async move {
        let result = watch_mpd(watcher_address, watcher_password, updates_tx).await;
        if let Err(error) = &result {
            logging::error(&format!("MPD change watcher failed: {error}"));
        }
        result
    });
    let mut terminal_events = EventStream::new();
    let mut redraw = time::interval(Duration::from_millis(50));
    redraw.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    let run_result: AppResult<()> = async {
        while app.running {
            tui.draw(&mut app)?;
            tokio::select! {
                event = terminal_events.next() => {
                    match event {
                        Some(Ok(CrosstermEvent::Key(key)))
                            if key.kind != KeyEventKind::Release =>
                        {
                            handler::handle_key_events(key, &mut app).await?;
                        }
                        Some(Ok(CrosstermEvent::Key(_))) => {}
                        Some(Ok(CrosstermEvent::Mouse(mouse))) => {
                            handler::handle_mouse_events(mouse, &mut app).await?;
                        }
                        Some(Ok(CrosstermEvent::Resize(_, _)))
                        | Some(Ok(CrosstermEvent::FocusGained))
                        | Some(Ok(CrosstermEvent::FocusLost))
                        | Some(Ok(CrosstermEvent::Paste(_))) => {}
                        Some(Err(error)) => {
                            return Err(Box::new(error) as Box<dyn std::error::Error + Send + Sync>);
                        }
                        None => {
                            return Err(std::io::Error::other("terminal event stream ended").into());
                        }
                    }
                }
                _ = redraw.tick() => {}
                snapshot = updates_rx.recv() => {
                    match snapshot {
                        Some(snapshot) => app.apply_snapshot(snapshot).await?,
                        None => {
                            return Err(std::io::Error::other(
                                "MPD change watcher stopped unexpectedly",
                            )
                            .into());
                        }
                    }
                }
            }
        }
        Ok(())
    }
    .await;

    watcher.abort();
    let exit_result = tui.exit();
    run_result?;
    exit_result?;
    Ok(())
}

async fn watch_mpd(
    address: String,
    password: Option<String>,
    updates: mpsc::Sender<MpdSnapshot>,
) -> AppResult<()> {
    let mut client = Connection::connect(&address, password.as_deref()).await?;
    loop {
        client.idle().await?;
        let snapshot = client.snapshot().await?;
        if updates.send(snapshot).await.is_err() {
            return Ok(());
        }
    }
}

fn mpd_address(host: &str, port: u16) -> String {
    if host.starts_with('[') && host.contains("]:") {
        host.to_string()
    } else if let Some(ipv6) = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .filter(|host| Ipv6Addr::from_str(host).is_ok())
    {
        format!("[{ipv6}]:{port}")
    } else if host.contains(':') && Ipv6Addr::from_str(host).is_ok() {
        format!("[{host}]:{port}")
    } else if host
        .rsplit_once(':')
        .is_some_and(|(_, suffix)| suffix.parse::<u16>().is_ok())
    {
        host.to_string()
    } else {
        format!("{host}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::mpd_address;

    #[test]
    fn formats_mpd_host_addresses() {
        assert_eq!(mpd_address("localhost", 6600), "localhost:6600");
        assert_eq!(mpd_address("localhost:6601", 6600), "localhost:6601");
        assert_eq!(mpd_address("[::1]", 6600), "[::1]:6600");
        assert_eq!(mpd_address("[::1]:6601", 6600), "[::1]:6601");
        assert_eq!(mpd_address("::1", 6600), "[::1]:6600");
    }
}
