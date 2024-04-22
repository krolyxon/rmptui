use clap::{Parser, Subcommand};
#[derive(Parser, Debug)]
#[command(version, about)]
#[clap(author = "krolyxon")]
/// MPD client made with Rust
pub struct Args {
    /// pause
    #[clap(short, long, default_value = "false")]
    pub pause: bool,

    /// toggle pause
    #[arg(short, long, default_value = "false")]
    pub toggle_pause: bool,

    /// show current status
    #[arg(short, long, default_value = "false")]
    pub show_status: bool,

    /// use fzf selector for selecting songs
    #[arg(short, long, default_value = "false")]
    pub fzf_select: bool,

    /// use dmenu selector for selecting songss
    #[arg(short, long, default_value = "false")]
    pub dmenu_select: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(arg_required_else_help = true)]
    Volume {
        vol: String,
    },

    Dmenu,
    Fzf,
    Status,
    Pause,
    Toggle,
}
