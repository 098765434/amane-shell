//! Now, as Unix time.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Milliseconds since 1970; 0 for a clock set before it.
pub fn unix_ms() -> i64 {
    i64::try_from(since_epoch().as_millis()).unwrap_or(i64::MAX)
}

/// Seconds since 1970; 0 for a clock set before it.
pub fn unix_s() -> u64 {
    since_epoch().as_secs()
}

/// How long ago `at`, in seconds since 1970, was; nothing for a time ahead.
pub fn since_unix_s(at: u64) -> Duration {
    Duration::from_secs(unix_s().saturating_sub(at))
}

fn since_epoch() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clocks_agree() {
        let (ms, s) = (unix_ms(), unix_s());
        assert!((ms / 1000).abs_diff(s as i64) <= 1);
        assert_eq!(since_unix_s(u64::MAX), Duration::ZERO);
    }
}
