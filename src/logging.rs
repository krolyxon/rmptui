use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::AppResult;

static LOG_FILE: OnceLock<Mutex<fs::File>> = OnceLock::new();

pub fn init(path: &Path) -> AppResult<PathBuf> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    let _ = LOG_FILE.set(Mutex::new(file));

    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let message = panic
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        let location = panic
            .location()
            .map(|location| {
                format!(
                    " at {}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_default();
        error(&format!("panic: {message}{location}"));
        panic_hook(panic);
    }));

    info(&format!("logging initialized: {}", path.display()));
    Ok(path.to_path_buf())
}

pub fn info(message: &str) {
    write("INFO", message);
}

pub fn error(message: &str) {
    write("ERROR", message);
}

fn write(level: &str, message: &str) {
    let Some(file) = LOG_FILE.get() else {
        return;
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let line = format!(
        "[{}.{:03}] {level} {message}",
        timestamp.as_secs(),
        timestamp.subsec_millis()
    );
    let mut file = file
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Err(error) = writeln!(file, "{line}") {
        report_write_error(error);
    }
}

fn report_write_error(error: io::Error) {
    eprintln!("failed to write rmptui log: {error}");
}
