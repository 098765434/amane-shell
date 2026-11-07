//! A stream the shell follows from another program's output, kept going: when
//! it ends (the program quit, or the daemon behind it restarted) it's
//! started again, a second later at first and backing off to a minute while
//! it keeps ending at once. A run that heard something starts the pacing
//! afresh. PipeWire's `pw-dump --monitor` and Tailscale's change stream run
//! this way.

use std::thread;
use std::time::Duration;

/// The first wait before a stream is started again.
pub const FIRST: Duration = Duration::from_secs(1);
/// The longest wait.
pub const MOST: Duration = Duration::from_secs(60);

/// How long to wait before the next start: doubling from [`FIRST`] to
/// [`MOST`], and back to the first after a run that heard something.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Backoff(Duration);

impl Default for Backoff {
    fn default() -> Self {
        Self(FIRST)
    }
}

impl Backoff {
    /// The wait after a run that `heard` something or not; the one after is
    /// longer unless it hears something.
    pub fn after(&mut self, heard: bool) -> Duration {
        if heard {
            self.0 = FIRST;
        }
        let wait = self.0;
        self.0 = (self.0 * 2).min(MOST);
        wait
    }
}

/// Runs `stream` again each time it returns, on this thread, forever:
/// `stream` follows its program until the output ends and says whether it
/// heard anything. `name` is for the log.
pub fn supervise(name: &str, mut stream: impl FnMut() -> bool) -> ! {
    let mut backoff = Backoff::default();
    loop {
        let heard = stream();
        let wait = backoff.after(heard);
        log::info!("{name} ended; starting it again in {}s", wait.as_secs());
        thread::sleep(wait);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wait_doubles_to_a_minute_and_starts_over_after_a_good_run() {
        let mut backoff = Backoff::default();
        let waits: Vec<u64> = (0..8).map(|_| backoff.after(false).as_secs()).collect();
        assert_eq!(waits, [1, 2, 4, 8, 16, 32, 60, 60]);
        assert_eq!(backoff.after(true), FIRST);
        assert_eq!(backoff.after(false), FIRST * 2);
    }
}
