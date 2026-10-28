//! Release builds have no console, so a panic used to close Lumen without a
//! trace. Panics are written to `crash.log` in the data dir; the next start
//! shows the report and clears it.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::OnceLock,
};

static LOG: OnceLock<PathBuf> = OnceLock::new();

pub fn install(data_dir: &Path) {
    let _ = LOG.set(data_dir.join("crash.log"));
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(path) = LOG.get() {
            if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
                let when = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or_default();
                let _ = writeln!(
                    file,
                    "[{when}] Lumen {} panicked: {info}\n{}",
                    env!("CARGO_PKG_VERSION"),
                    std::backtrace::Backtrace::force_capture()
                );
            }
        }
        default(info);
    }));
}

/// The last crash report, removed once read.
pub fn take(data_dir: &Path) -> Option<String> {
    let path = data_dir.join("crash.log");
    let text = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(path);
    // The first line says what happened; the backtrace is for bug reports.
    Some(text.chars().take(4000).collect())
}
