use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(version, about, author = "krolyxon")]
/// MPD client made with Rust
pub struct Args {
    /// No TUI
    #[arg(short= 'n', default_value="false")]
    pub tui: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(arg_required_else_help = true, long_flag = "volume" , short_flag = 'v')]
    /// Set Volume
    Volume {
        vol: String,
    },

    /// Use dmenu for selection
    #[command(long_flag = "dmenu" , short_flag = 'd')]
    Dmenu,

    /// Use Fzf for selection
    #[command(long_flag = "fzf" , short_flag = 'f')]
    Fzf,

    /// Check Status
    #[command(long_flag = "status" , short_flag = 's')]
    Status,

    /// Pause playback
    #[command(long_flag = "pause" , short_flag = 'p')]
    Pause,

    /// Toggle Playback
    #[command(long_flag = "toggle" , short_flag = 't')]
    Toggle,

}
