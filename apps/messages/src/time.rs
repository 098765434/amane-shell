//! When a message was sent, in words: a time today, "Yesterday", a weekday
//! this week, else a date, as Messages on a phone writes them.

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone};

fn local(ms: i64) -> Option<DateTime<Local>> {
    Local.timestamp_millis_opt(ms).single()
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// The time of day, "14:02".
pub fn clock(ms: i64) -> String {
    local(ms).map_or_else(String::new, |at| at.format("%H:%M").to_string())
}

/// For the inbox: the time today, "Yesterday", the weekday this week, else
/// the date.
pub fn short(ms: i64) -> String {
    let Some(at) = local(ms) else {
        return String::new();
    };
    short_on(at.date_naive(), today()).unwrap_or_else(|| at.format("%H:%M").to_string())
}

/// `None` for today, which shows the time instead.
fn short_on(day: NaiveDate, today: NaiveDate) -> Option<String> {
    let days = (today - day).num_days();
    Some(match days {
        ..=0 => return None,
        1 => String::from("Yesterday"),
        2..=6 => day.format("%A").to_string(),
        _ if day.year() == today.year() => day.format("%-d %b").to_string(),
        _ => day.format("%-d %b %Y").to_string(),
    })
}

/// The day a run of messages was sent on, over them in a conversation.
pub fn day(ms: i64) -> String {
    local(ms).map_or_else(String::new, |at| day_on(at.date_naive(), today()))
}

fn day_on(day: NaiveDate, today: NaiveDate) -> String {
    match (today - day).num_days() {
        ..=0 => String::from("Today"),
        1 => String::from("Yesterday"),
        2..=6 => day.format("%A").to_string(),
        _ if day.year() == today.year() => day.format("%A %-d %B").to_string(),
        _ => day.format("%-d %B %Y").to_string(),
    }
}

/// Whether two messages were sent on different days, here.
pub fn new_day(previous: i64, next: i64) -> bool {
    match (local(previous), local(next)) {
        (Some(a), Some(b)) => a.date_naive() != b.date_naive(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn the_inbox_says_how_long_ago_in_words() {
        let today = date(2026, 11, 5);
        assert_eq!(short_on(today, today), None);
        assert_eq!(short_on(date(2026, 11, 4), today).unwrap(), "Yesterday");
        assert_eq!(short_on(date(2026, 11, 2), today).unwrap(), "Monday");
        assert_eq!(short_on(date(2026, 10, 1), today).unwrap(), "1 Oct");
        assert_eq!(short_on(date(2025, 10, 1), today).unwrap(), "1 Oct 2025");
    }

    #[test]
    fn a_conversation_heads_each_day() {
        let today = date(2026, 11, 5);
        assert_eq!(day_on(today, today), "Today");
        assert_eq!(day_on(date(2026, 11, 4), today), "Yesterday");
        assert_eq!(day_on(date(2026, 10, 1), today), "Thursday 1 October");
        assert_eq!(day_on(date(2025, 10, 1), today), "1 October 2025");
    }
}
