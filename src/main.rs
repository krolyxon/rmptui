mod cli;
mod connection;
use clap::Parser;
use cli::Args;
use cli::Command;
use connection::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut conn = Connection::new("127.0.0.1:6600")?;

    match args.command {
        Command::Volume { vol } => {
            conn.set_volume(vol);
        }
        Command::Dmenu => conn.play_dmenu(),
        Command::Fzf => conn.play_fzf(),
        Command::Status => conn.status(),
        Command::Pause => conn.pause(),
        Command::Toggle => conn.toggle_pause(),
    }

    Ok(())
}
