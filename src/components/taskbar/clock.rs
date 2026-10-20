use std::time::Duration;

use amane::Service;
use chrono::Local;

/// A tiny clock service: holds the formatted wall-clock string and re-reads
/// it once a second, signalling a redraw only when the visible text changes
/// (so the bar repaints at most once a minute for the clock).
pub struct Clock {
    text: String,
}

impl Service for Clock {
    fn new() -> Self {
        Self { text: now() }
    }

    fn interval() -> Duration {
        Duration::from_secs(1)
    }

    fn update(&mut self) -> bool {
        let next = now();
        if next == self.text {
            return false;
        }
        self.text = next;
        true
    }
}

impl Clock {
    pub fn text(&self) -> &str {
        &self.text
    }
}

fn now() -> String {
    Local::now().format("%H:%M").to_string()
}
