mod connection;
use clap::Parser;
use connection::Connection;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// pause
    #[arg(short, long, default_value = "false")]
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut conn = Connection::new("127.0.0.1:6600")?;

    if args.show_status {
        conn.status();
    }

    if args.toggle_pause {
        conn.toggle_pause();
    }

    if args.pause {
        conn.pause();
    }

    if args.fzf_select {
        conn.play_fzf();
    }

    if args.dmenu_select {
        conn.play_dmenu();
    }

    Ok(())
}
