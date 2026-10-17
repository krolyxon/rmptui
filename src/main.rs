use std::{io, path::PathBuf, process::ExitCode};

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use rmptui::{app::App, config::Config, logging};

/// A fast, keyboard-driven MPD client for the terminal.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Use this configuration file instead of the default location.
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
    /// Print the default configuration and exit.
    #[arg(long)]
    print_default_config: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    if args.print_default_config {
        print!("{}", rmptui::config::DEFAULT_CONFIG);
        return ExitCode::SUCCESS;
    }
    if let Err(error) = logging::init(logging::LOG_PATH.as_ref()) {
        eprintln!("rmptui: cannot log to {}: {error}", logging::LOG_PATH);
    }
    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("exiting: {error:#}");
            eprintln!("rmptui: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> anyhow::Result<()> {
    let (config, path) = Config::load(args.config.as_deref())?;
    tracing::info!("using configuration {}", path.display());
    let mouse = config.ui.mouse;
    let app = App::new(config)?;

    // `init` installs a panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    if mouse {
        execute!(io::stdout(), EnableMouseCapture)?;
    }
    let result = app.run(&mut terminal).await;
    if mouse {
        let _ = execute!(io::stdout(), DisableMouseCapture);
    }
    ratatui::restore();
    result
}
