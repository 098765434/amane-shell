use std::time::Duration;

use amane::Service;
use amane_kit::settings::status_bar;
use chrono::{DateTime, Local};

/// A tiny clock service: holds the wall-clock time and re-reads it once a
/// second, signalling a redraw only when the bar's clock would read
/// differently (so the bar repaints once a minute, or a second while it
/// shows seconds).
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

    fn interval() -> Duration {
        Duration::from_secs(1)
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
        use chrono::Timelike;
        (self.now.hour(), self.now.minute())
    }

    /// The day in the bar's format, such as "Sunday 25 October".
    pub fn date(&self) -> String {
        self.now
            .format(status_bar().date_format.pattern())
            .to_string()
    }
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
