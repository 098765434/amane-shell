//! A countdown the launcher's Start Timer action sets: one at a time, a
//! notification when it runs out, and a chip in the bar while it runs.
//! amane has no timers, so a thread sleeps it out.

use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use amane::Notify;

amane_kit::marker! {
    /// The timer started, ran out or was stopped: what asked how long
    /// is left draws again.
    pub Countdown
}

struct Running {
    // which start this is, so an older thread waking finds it replaced
    id: u64,
    ends: SystemTime,
    label: String,
}

static TIMER: Mutex<Option<Running>> = Mutex::new(None);
static STARTS: Mutex<u64> = Mutex::new(0);

/// Starts a timer for `length`, replacing any running, called `label`.
pub fn start(length: Duration, label: String) {
    let id = {
        let mut starts = STARTS.lock().unwrap_or_else(|e| e.into_inner());
        *starts += 1;
        *starts
    };
    *TIMER.lock().unwrap_or_else(|e| e.into_inner()) = Some(Running {
        id,
        ends: SystemTime::now() + length,
        label,
    });
    Countdown::changed();
    std::thread::spawn(move || {
        std::thread::sleep(length);
        let done = {
            let mut timer = TIMER.lock().unwrap_or_else(|e| e.into_inner());
            match timer.as_ref() {
                Some(running) if running.id == id => timer.take(),
                _ => None,
            }
        };
        if let Some(done) = done {
            Notify::new("Timer")
                .body(&format!("{} is done", done.label))
                .send();
            Countdown::changed();
        }
    });
}

/// Stops the timer running, if one is.
pub fn cancel() {
    *TIMER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Countdown::changed();
}

/// How long the running timer has left, and its name.
pub fn left() -> Option<(Duration, String)> {
    Countdown::watch();
    let timer = TIMER.lock().unwrap_or_else(|e| e.into_inner());
    let running = timer.as_ref()?;
    let left = running
        .ends
        .duration_since(SystemTime::now())
        .unwrap_or_default();
    Some((left, running.label.clone()))
}

/// `left` as a clock shows it: "4:59", or "1:04:59" past an hour.
pub fn format(left: Duration) -> String {
    // a second still running shows as a whole one, so "0:00" never shows
    let secs = left.as_secs() + u64::from(left.subsec_nanos() > 0);
    let (hours, minutes, seconds) = (secs / 3600, secs / 60 % 60, secs % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_left_reads_as_a_clock() {
        assert_eq!(format(Duration::from_secs(299)), "4:59");
        assert_eq!(format(Duration::from_millis(299_400)), "5:00");
        assert_eq!(format(Duration::from_secs(3899)), "1:04:59");
        assert_eq!(format(Duration::from_secs(5)), "0:05");
    }
}
