//! The one logger every amane-shell program sets up.
//!
//! Programs log through the [`log`] crate's macros; [`init`] sends the lines to
//! stderr, which systemd puts in the journal with its own timestamps:
//!
//! ```text
//! [amane-shell] WARN amane_shell::services::state: couldn't save dock-pins.json: …
//! ```
//!
//! The level is `info` unless `AMANE_LOG` names another one (`error`, `warn`,
//! `info`, `debug`, `trace` or `off`).

#![warn(missing_docs)]

use std::io::Write;
use std::sync::OnceLock;

use log::{LevelFilter, Log, Metadata, Record};

/// The environment variable that sets the level.
pub const LEVEL_VAR: &str = "AMANE_LOG";

static LOGGER: Logger = Logger;
static APP: OnceLock<&'static str> = OnceLock::new();

struct Logger;

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let app = APP.get().copied().unwrap_or("amane");
        let line = format!(
            "[{app}] {} {}: {}\n",
            record.level(),
            record.target(),
            record.args()
        );

        // stderr gone means nobody is left to tell
        let _ = std::io::stderr().lock().write_all(line.as_bytes());
    }

    fn flush(&self) {}
}

/// Sets up logging for the program `app`, once, at the top of `main`.
///
/// A second call does nothing, so a library's tests can call it too.
pub fn init(app: &'static str) {
    if APP.set(app).is_err() {
        return;
    }

    let level = std::env::var(LEVEL_VAR)
        .ok()
        .and_then(|value| level(&value))
        .unwrap_or(LevelFilter::Info);

    // another logger already set up (by a test harness) keeps the lines
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(level);
    }
}

fn level(value: &str) -> Option<LevelFilter> {
    value.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_read_in_any_case() {
        assert_eq!(level("debug"), Some(LevelFilter::Debug));
        assert_eq!(level(" WARN "), Some(LevelFilter::Warn));
        assert_eq!(level("off"), Some(LevelFilter::Off));
        assert_eq!(level("loud"), None);
    }
}
