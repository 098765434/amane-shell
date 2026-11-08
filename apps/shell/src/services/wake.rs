//! A listener's wait: until a deadline, or until something nudges it, whichever comes first,
//! with no wakeups in between. For a service whose only clock is its own next deadline (a
//! toast's lifetime, a timed Do Not Disturb), in place of polling every so often.

use std::sync::{Condvar, Mutex, PoisonError};
use std::time::SystemTime;

use emaki_kit::sync::Locked as _;

/// One listener's wake-up call.
pub struct Wake {
    nudged: Mutex<bool>,
    ready: Condvar,
}

impl Wake {
    pub const fn new() -> Self {
        Self {
            nudged: Mutex::new(false),
            ready: Condvar::new(),
        }
    }

    /// Wakes the listener: something it works its deadline out from changed.
    pub fn nudge(&self) {
        *self.nudged.locked() = true;
        self.ready.notify_all();
    }

    /// Waits until `deadline` passes or [`nudge`](Self::nudge) is called; forever for `None`.
    /// A nudge made before the wait ends it at once.
    pub fn wait(&self, deadline: Option<SystemTime>) {
        let mut nudged = self.nudged.locked();
        while !*nudged {
            match deadline {
                None => {
                    nudged = self
                        .ready
                        .wait(nudged)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                Some(deadline) => {
                    let Ok(left) = deadline.duration_since(SystemTime::now()) else {
                        break;
                    };
                    if left.is_zero() {
                        break;
                    }
                    nudged = self
                        .ready
                        .wait_timeout(nudged, left)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0;
                }
            }
        }
        *nudged = false;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn ends_at_the_deadline_or_a_nudge() {
        let wake = Wake::new();
        let start = Instant::now();
        wake.wait(Some(SystemTime::now() + Duration::from_millis(30)));
        assert!(start.elapsed() >= Duration::from_millis(25));

        // a nudge before the wait counts
        wake.nudge();
        let start = Instant::now();
        wake.wait(None);
        assert!(start.elapsed() < Duration::from_secs(1));

        let wake = Arc::new(Wake::new());
        let nudger = Arc::clone(&wake);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            nudger.nudge();
        });
        let start = Instant::now();
        wake.wait(None);
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
