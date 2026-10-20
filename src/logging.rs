//! File logging. The terminal belongs to the UI, so diagnostics go to a file.

use std::{
    fs::OpenOptions,
    io,
    path::Path,
    sync::{Arc, Mutex},
};

use tracing_subscriber::{EnvFilter, fmt};

pub const LOG_PATH: &str = "/tmp/rmptui.log";

/// Appends logs to `path`. The level can be set with `RMPTUI_LOG`
/// (e.g. `RMPTUI_LOG=debug`); the default is `info`.
pub fn init(path: &Path) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = Arc::new(Mutex::new(options.open(path)?));
    let filter = EnvFilter::try_from_env("RMPTUI_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(move || MutexWriter(Arc::clone(&file)))
        .try_init()
        .map_err(io::Error::other)?;

    // Record panics in the log as well as on the (restored) terminal.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("panic: {info}");
        previous(info);
    }));
    Ok(())
}

struct MutexWriter(Arc<Mutex<std::fs::File>>);

impl io::Write for MutexWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .flush()
    }
}
