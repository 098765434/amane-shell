use std::time::Duration;

use amane::Service;
use chrono::{DateTime, Local};

/// A tiny clock service: holds the formatted wall-clock time and date and
/// re-reads them once a second, signalling a redraw only when the visible
/// text changes (so the bar repaints at most once a minute for the clock).
pub struct Clock {
    text: String,
    date: String,
}

impl Service for Clock {
    fn new() -> Self {
        let now = Local::now();
        Self {
            text: time(now),
            date: date(now),
        }
    }

    fn interval() -> Duration {
        Duration::from_secs(1)
    }

    fn update(&mut self) -> bool {
        let now = Local::now();
        let next = time(now);
        if next == self.text {
            return false;
        }
        self.text = next;
        self.date = date(now);
        true
    }
}

impl Clock {
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The day, such as "Sunday 25 October".
    pub fn date(&self) -> &str {
        &self.date
    }
}

fn time(now: DateTime<Local>) -> String {
    now.format("%H:%M").to_string()
}

fn date(now: DateTime<Local>) -> String {
    now.format("%A %-d %B").to_string()
}
