mod connection;
use clap::Parser;
use connection::Connection;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Ignore case in search
    #[arg(short, long, default_value = "false")]
    pub pause: bool,

    #[arg(short, long, default_value = "false")]
    pub toggle_pause: bool,

    #[arg(short, long, default_value = "false")]
    pub show_status: bool,

    #[arg(short, long, default_value = "false")]
    pub fzf_select: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut conn = Connection::new("127.0.0.1:6600")?;
    if args.show_status {
        conn.status();
    }

    if args.fzf_select {
        conn.play_fzf()
    }

    Ok(())
}
