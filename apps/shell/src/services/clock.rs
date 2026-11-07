use std::time::{Duration, SystemTime};

use amane::{Config, Logind, Service};
use amane_kit::settings::{StatusBar, status_bar};
use chrono::{DateTime, Local, Timelike};

use crate::services::Wake;

/// A tiny clock service: holds the wall-clock time and wakes when the bar's clock would next
/// read differently: on the minute, or each second while it shows seconds or flashes its
/// colons. A change of the bar's settings, or the computer waking from sleep, wakes it too.
pub struct Clock {
    now: DateTime<Local>,
    // the bar's clock as last signalled
    shown: String,
}

impl Service for Clock {
    fn new() -> Self {
        let now = Local::now();
        Self {
            shown: shown(now),
            now,
        }
    }

    fn listen() {
        Config::<StatusBar>::subscribe(|_| WAKE.nudge()).forever();
        // a sleep that spans a suspend wakes late: the wake-up itself is the cue
        Logind::subscribe(|_| WAKE.nudge()).forever();

        loop {
            WAKE.wait(Some(next_turn(SystemTime::now(), per_second())));
            let mut clock = Self::write();
            if !clock.update() {
                clock.quiet();
            }
        }
    }

    fn update(&mut self) -> bool {
        let now = Local::now();
        let next = shown(now);
        if next == self.shown {
            return false;
        }
        self.now = now;
        self.shown = next;
        true
    }
}

impl Clock {
    /// The time in the bar's format, such as "14:05" or "2:05 PM".
    pub fn text(&self) -> String {
        self.now.format(status_bar().time_pattern()).to_string()
    }

    /// What the bar's clock shows: the day and date when they show (the
    /// date only `dated` when it shows "when space allows"), then the
    /// time, its colons gone every other second while they flash.
    pub fn bar_text(&self, dated: bool) -> String {
        let bar = status_bar();
        let text = self.now.format(&bar.clock_pattern(dated)).to_string();
        if bar.flash_separators && self.now.timestamp() % 2 == 1 {
            // a figure space keeps the width a colon had, near enough
            text.replace(':', "\u{2008}")
        } else {
            text
        }
    }

    /// The hour and minute, for an analog clock's hands.
    pub fn hands(&self) -> (u32, u32) {
        (self.now.hour(), self.now.minute())
    }

    /// The day in the bar's format, such as "Sunday 25 October".
    pub fn date(&self) -> String {
        self.now
            .format(status_bar().date_format.pattern())
            .to_string()
    }
}

static WAKE: Wake = Wake::new();

// whether what the bar shows changes every second
fn per_second() -> bool {
    let bar = status_bar();
    let pattern = format!("{}{}", bar.clock_pattern(true), bar.time_pattern());
    bar.flash_separators
        || ["%S", "%T", "%X", "%r", "%s"]
            .iter()
            .any(|s| pattern.contains(s))
}

// a moment past the next second, or minute, turning over
fn next_turn(now: SystemTime, per_second: bool) -> SystemTime {
    let local: DateTime<Local> = now.into();
    let into_second = Duration::from_nanos(u64::from(local.nanosecond() % 1_000_000_000));
    let into = if per_second {
        into_second
    } else {
        Duration::from_secs(u64::from(local.second())) + into_second
    };
    let period = Duration::from_secs(if per_second { 1 } else { 60 });
    now + period.saturating_sub(into) + Duration::from_millis(5)
}

// the clock as the bar shows it now, and its day, so a change of format or
// of day redraws too; each second while the separators flash
fn shown(now: DateTime<Local>) -> String {
    let bar = status_bar();
    let flash = if bar.flash_separators { " %S" } else { "" };
    now.format(&format!(
        "{} {} {}{flash}",
        bar.clock_pattern(true),
        bar.time_pattern(),
        bar.date_format.pattern()
    ))
    .to_string()
}

/// `time` on the local clock, in chrono's `format`.
pub fn local(time: std::time::SystemTime, format: &str) -> String {
    chrono::DateTime::<chrono::Local>::from(time)
        .format(format)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wakes_just_past_the_next_turn() {
        let now = SystemTime::now();
        let minute = next_turn(now, false);
        let at: DateTime<Local> = minute.into();
        assert_eq!(at.second(), 0);
        assert!(minute > now && minute <= now + Duration::from_secs(61));

        let second = next_turn(now, true);
        assert!(second > now && second <= now + Duration::from_millis(1010));
        let at: DateTime<Local> = second.into();
        assert!(at.nanosecond() < 50_000_000, "just past the turn");
    }
}
