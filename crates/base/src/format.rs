//! Times, lengths, sizes and counts in words, one way across the desktop.

use std::time::Duration;

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// How long ago something was, `elapsed` since: "just now", "5 min ago",
/// "3 h ago", "yesterday", "4 days ago", "3 weeks ago", "5 months ago",
/// "2 years ago".
pub fn ago(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    let days = seconds / DAY;
    match seconds {
        0..MINUTE => String::from("just now"),
        MINUTE..HOUR => format!("{} min ago", seconds / MINUTE),
        HOUR..DAY => format!("{} h ago", seconds / HOUR),
        _ => match days {
            1 => String::from("yesterday"),
            2..14 => format!("{days} days ago"),
            14..61 => format!("{} weeks ago", days / 7),
            61..730 => format!("{} months ago", days / 30),
            _ => format!("{} years ago", days / 365),
        },
    }
}

/// A length of time to the minute: "45 s", "5 min", "2 h", "1 h 24 min".
/// The nearest minute, and seconds under one, so a length that's nearly
/// over never reads as over.
pub fn span(length: Duration) -> String {
    let seconds = length.as_secs();
    if seconds < MINUTE {
        return format!("{seconds} s");
    }
    let minutes = (seconds + MINUTE / 2) / MINUTE;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// A length of time to the second: "1 h 30 min", "1 min 30 s", "45 s".
pub fn span_exact(length: Duration) -> String {
    let seconds = length.as_secs();
    let (hours, minutes, seconds) = (seconds / HOUR, seconds / MINUTE % 60, seconds % MINUTE);
    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours} h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes} min"));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{seconds} s"));
    }
    parts.join(" ")
}

/// A size in powers of 1000, as file managers count: "830 B", "9.8 GB",
/// "12 kB", "240 MB".
pub fn bytes(count: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut value = count as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 || value >= 10.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// `n` things: "1 change", "3 changes", "0 changes".
pub fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// `n` with its thousands set apart by commas: "7", "5,139", "108,517".
pub fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_are_set_apart() {
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(12), "12");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(5139), "5,139");
        assert_eq!(thousands(108517), "108,517");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    fn s(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn ago_reads_in_words() {
        assert_eq!(ago(s(5)), "just now");
        assert_eq!(ago(s(5 * MINUTE + 9)), "5 min ago");
        assert_eq!(ago(s(3 * HOUR)), "3 h ago");
        assert_eq!(ago(s(DAY + HOUR)), "yesterday");
        assert_eq!(ago(s(4 * DAY)), "4 days ago");
        assert_eq!(ago(s(21 * DAY)), "3 weeks ago");
        assert_eq!(ago(s(150 * DAY)), "5 months ago");
        assert_eq!(ago(s(800 * DAY)), "2 years ago");
    }

    #[test]
    fn spans_round_to_the_minute_and_keep_seconds_under_one() {
        assert_eq!(span(s(45)), "45 s");
        assert_eq!(span(s(5 * MINUTE + 20)), "5 min");
        assert_eq!(span(s(5 * MINUTE + 40)), "6 min");
        assert_eq!(span(s(2 * HOUR)), "2 h");
        assert_eq!(span(s(HOUR + 24 * MINUTE)), "1 h 24 min");
        assert_eq!(span_exact(s(90)), "1 min 30 s");
        assert_eq!(span_exact(s(HOUR + 30 * MINUTE)), "1 h 30 min");
        assert_eq!(span_exact(s(0)), "0 s");
    }

    #[test]
    fn sizes_and_counts() {
        assert_eq!(bytes(830), "830 B");
        assert_eq!(bytes(12_300), "12 kB");
        assert_eq!(bytes(9_800_000_000), "9.8 GB");
        assert_eq!(bytes(240_000_000), "240 MB");
        assert_eq!(count(1, "change", "changes"), "1 change");
        assert_eq!(count(3, "change", "changes"), "3 changes");
    }
}
