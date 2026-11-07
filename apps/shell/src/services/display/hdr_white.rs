//! Keeps the screen's brightness in HDR across a config reload and a new
//! session. In HDR, amane's `Brightness` moves niri's SDR white (the
//! panel no longer follows its backlight), which is niri's temporary kind
//! of change: `displays.kdl` would put the old level back on the next
//! reload. So once a level has rested for a moment, its `sdr-brightness`
//! line is written into the panel's block there.
//!
//! Only that line changes, and nothing is committed: a key held down
//! would otherwise be a commit a step. Shell Settings commits the file
//! with its next change to Displays. A level idle dimmed to
//! ([`dimmed`]) is never written.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once, PoisonError};
use std::time::{Duration, SystemTime};

use amane::{Brightness, Service};

use crate::services::Wake;

// how long a level stays put before it counts as rested
const REST: Duration = Duration::from_secs(1);

static DIMMED: AtomicBool = AtomicBool::new(false);

// the panel's SDR white as last seen, and when it changed
type Seen = (Option<(String, f64)>, SystemTime);
static LATEST: Mutex<Option<Seen>> = Mutex::new(None);
static WAKE: Wake = Wake::new();

/// Whether idle has the screen dimmed, so its level isn't kept.
pub fn dimmed(on: bool) {
    DIMMED.store(on, Ordering::Relaxed);
}

fn hdr_white(brightness: &Brightness) -> Option<(String, f64)> {
    brightness
        .hdr()
        .map(|panel| (panel.output.clone(), panel.sdr_white.round()))
}

/// Starts watching, once.
pub fn watch() {
    static WATCH: Once = Once::new();
    WATCH.call_once(|| {
        Brightness::subscribe(|brightness| {
            let now = hdr_white(brightness);
            let mut latest = LATEST.lock().unwrap_or_else(PoisonError::into_inner);
            if latest.as_ref().is_some_and(|(seen, _)| *seen == now) {
                return;
            }
            *latest = Some((now, SystemTime::now()));
            drop(latest);
            WAKE.nudge();
        })
        .forever();

        std::thread::spawn(|| {
            // what's in the file: the first reading is what it already says
            let mut kept = hdr_white(&Brightness::read());
            loop {
                let latest = LATEST
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone();
                let Some((now, at)) = latest else {
                    WAKE.wait(None);
                    continue;
                };
                // not rested yet: wait out the rest, or the next change
                let rested = at + REST;
                if SystemTime::now() < rested {
                    WAKE.wait(Some(rested));
                    continue;
                }
                if now.is_some()
                    && now != kept
                    && !DIMMED.load(Ordering::Relaxed)
                    && let Some((output, nits)) = &now
                    && keep(&displays_file(), output, *nits).is_ok()
                {
                    kept.clone_from(&now);
                }
                WAKE.wait(None);
            }
        });
    });
}

/// `displays.kdl`, beside the file `~/.config/niri/config.kdl` leads to.
fn displays_file() -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_default()
        .join("niri/config.kdl");
    fs::canonicalize(&config)
        .unwrap_or(config)
        .with_file_name("displays.kdl")
}

fn keep(file: &Path, output: &str, nits: f64) -> Result<(), String> {
    let text = fs::read_to_string(file).map_err(|e| e.to_string())?;
    let Some(changed) = with_sdr_white(&text, output, nits) else {
        return Ok(());
    };
    // into place by a rename, so niri never reads half a file
    let temp = file.with_extension("kdl.brightness");
    fs::write(&temp, changed).map_err(|e| e.to_string())?;
    fs::rename(&temp, file).map_err(|e| e.to_string())
}

/// `text` with `sdr-brightness` set in `output`'s block, or none when it
/// already says so or has no block for it (Settings writes the blocks).
fn with_sdr_white(text: &str, output: &str, nits: f64) -> Option<String> {
    let opening = format!("output \"{output}\" {{");
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let start = lines.iter().position(|line| line.trim() == opening)?;
    let end = start + lines[start..].iter().position(|line| line.trim() == "}")?;
    let setting = format!("    sdr-brightness {nits}");
    match (start..end).find(|&at| lines[at].trim_start().starts_with("sdr-brightness ")) {
        Some(at) if lines[at] == setting => return None,
        Some(at) => lines[at] = setting,
        None => lines.insert(end, setting),
    }
    let mut changed = lines.join("\n");
    if text.ends_with('\n') {
        changed.push('\n');
    }
    Some(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "// Written by Shell Settings\n\noutput \"eDP-1\" {\n    scale 1.75\n    sdr-brightness 80\n}\n\noutput \"DP-1\" {\n    scale 1\n}\n";

    #[test]
    fn changes_the_line_in_its_block() {
        let changed = with_sdr_white(FILE, "eDP-1", 140.0).unwrap();
        assert!(changed.contains("    sdr-brightness 140\n}"));
        assert!(!changed.contains("sdr-brightness 80"));
        assert!(changed.ends_with("scale 1\n}\n"));
    }

    #[test]
    fn adds_the_line_when_missing() {
        let changed = with_sdr_white(FILE, "DP-1", 203.0).unwrap();
        assert!(changed.contains("    scale 1\n    sdr-brightness 203\n}"));
    }

    #[test]
    fn leaves_the_file_alone_when_it_agrees_or_has_no_block() {
        assert_eq!(with_sdr_white(FILE, "eDP-1", 80.0), None);
        assert_eq!(with_sdr_white(FILE, "HDMI-A-1", 80.0), None);
    }
}
