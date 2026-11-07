//! A countdown the launcher's Start Timer action sets: one at a time, a
//! notification when it runs out, and an activity in the bar while it
//! runs.
//! It runs out on a timer on amane's event loop, cancelled when replaced or stopped.

use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use amane::{Notify, Timer, TimerHandle};
use amane_kit::sync::Locked as _;

amane_kit::marker! {
    /// The timer started, ran out or was stopped: what asked how long
    /// is left draws again.
    pub Countdown
}

struct Running {
    ends: SystemTime,
    label: String,
    due: TimerHandle,
}

static TIMER: Mutex<Option<Running>> = Mutex::new(None);

/// Starts a timer for `length`, replacing any running, called `label`.
pub fn start(length: Duration, label: String) {
    let due = Timer::after(length, ran_out);
    let replaced = TIMER.locked().replace(Running {
        ends: SystemTime::now() + length,
        label,
        due,
    });
    if let Some(replaced) = replaced {
        replaced.due.cancel();
    }
    Countdown::changed();
}

// on the event loop, when the running timer's time is up
fn ran_out() {
    let Some(done) = TIMER.locked().take() else {
        return;
    };
    Countdown::changed();
    // the notification goes over D-Bus, which isn't for the UI thread
    std::thread::spawn(move || {
        Notify::new("Timer")
            .body(&format!("{} is done", done.label))
            .send();
    });
}

/// Stops the timer running, if one is.
pub fn cancel() {
    if let Some(running) = TIMER.locked().take() {
        running.due.cancel();
    }
    Countdown::changed();
}

/// How long the running timer has left, and its name.
pub fn left() -> Option<(Duration, String)> {
    Countdown::watch();
    let timer = TIMER.locked();
    let running = timer.as_ref()?;
    let left = running
        .ends
        .duration_since(SystemTime::now())
        .unwrap_or_default();
    Some((left, running.label.clone()))
}

/// When the running timer runs out, and its name, for the bar's Live
/// Activity, which counts down to it itself.
pub fn ends() -> Option<(SystemTime, String)> {
    Countdown::watch();
    let timer = TIMER.locked();
    let running = timer.as_ref()?;
    Some((running.ends, running.label.clone()))
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
