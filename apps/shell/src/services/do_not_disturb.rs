//! Do Not Disturb — hides toasts until it's turned off or its time runs out.
//! The notifications surface hides them, and offers [`choices`] in its menu.
//!
//! Notifications still arrive and collect in the center, and critical ones
//! still show as toasts. It's kept in the state file `dnd.json` (the older
//! one-line `dnd` is read when that isn't there), so a timed Do Not Disturb
//! survives a restart; a missing or unreadable file means off.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use amane::Service;
use amane::serde::{Deserialize, Serialize};

use crate::services::state::{self, State};

use crate::services::Wake;
use chrono::{Days, Local, NaiveDateTime, NaiveTime, TimeZone};

/// When a timed Do Not Disturb runs out: "this evening" and "tomorrow".
const EVENING: NaiveTime = NaiveTime::from_hms_opt(19, 0, 0).unwrap();
const MORNING: NaiveTime = NaiveTime::from_hms_opt(8, 0, 0).unwrap();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Until {
    /// Until it's turned off.
    Forever,
    At(SystemTime),
}

pub struct DoNotDisturb {
    until: Option<Until>,
}

impl Service for DoNotDisturb {
    fn new() -> Self {
        let until = state::load::<Kept>().and_then(|kept| kept.until(SystemTime::now()));
        Self { until }
    }

    // only a timed one changes by itself: the listener waits for its end, or for a new setting
    fn listen() {
        loop {
            let end = match Self::peek().until {
                Some(Until::At(end)) => Some(end),
                _ => None,
            };
            WAKE.wait(end);

            let mut dnd = Self::write();
            if !dnd.update() {
                dnd.quiet();
            }
        }
    }

    fn update(&mut self) -> bool {
        if let Some(Until::At(end)) = self.until
            && SystemTime::now() >= end
        {
            self.until = None;
            save(None);
            return true;
        }
        false
    }
}

// a new setting moves the end the listener waits for
static WAKE: Wake = Wake::new();

impl DoNotDisturb {
    pub fn on(&self) -> bool {
        match self.until {
            None => false,
            Some(Until::Forever) => true,
            Some(Until::At(end)) => SystemTime::now() < end,
        }
    }

    pub fn until(&self) -> Option<Until> {
        self.until.filter(|_| self.on())
    }

    /// `None` turns it off.
    pub fn set(until: Option<Until>) {
        Self::write().until = until;
        save(until);
        WAKE.nudge();
    }

    pub fn toggle() {
        let on = Self::read().on();
        Self::set(if on { None } else { Some(Until::Forever) });
    }
}

/// The timed choices at `now`: for an hour, until this evening while it's
/// still ahead, and until tomorrow morning. The menu and Quick Settings'
/// Do Not Disturb page both offer these.
pub fn choices(now: NaiveDateTime) -> Vec<(&'static str, Until)> {
    let at = |time: NaiveDateTime| {
        let end = Local
            .from_local_datetime(&time)
            .earliest()
            .map_or(SystemTime::now(), SystemTime::from);
        Until::At(end)
    };

    let mut choices = vec![("For 1 Hour", at(now + Duration::from_secs(3600)))];
    if let Some(evening) = this_evening(now) {
        choices.push(("Until This Evening", at(evening)));
    }
    choices.push(("Until Tomorrow", at(tomorrow_morning(now))));
    choices
}

/// Today's evening, while it's still ahead.
fn this_evening(now: NaiveDateTime) -> Option<NaiveDateTime> {
    let evening = now.date().and_time(EVENING);
    (now < evening).then_some(evening)
}

/// The next morning: tomorrow's, or today's when it's still before it.
fn tomorrow_morning(now: NaiveDateTime) -> NaiveDateTime {
    let today = now.date().and_time(MORNING);
    if now < today {
        return today;
    }
    let tomorrow = now
        .date()
        .checked_add_days(Days::new(1))
        .unwrap_or(now.date());
    tomorrow.and_time(MORNING)
}

// ───── the file ──────────────────────────────────────────────────────────────

/// The file from before the state files, one line, deleted when off.
const OLD_FILE: &str = "dnd";

/// What's kept: off, on until it's turned off, or on until a time (in
/// seconds since 1970).
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
enum Kept {
    Off,
    Forever,
    Until(u64),
}

impl Kept {
    fn of(until: Option<Until>) -> Self {
        match until {
            None => Kept::Off,
            Some(Until::Forever) => Kept::Forever,
            Some(Until::At(end)) => {
                Kept::Until(end.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
            }
        }
    }

    /// What it means at `now`: a time already past is off.
    fn until(&self, now: SystemTime) -> Option<Until> {
        match *self {
            Kept::Off => None,
            Kept::Forever => Some(Until::Forever),
            Kept::Until(seconds) => {
                let end = UNIX_EPOCH + Duration::from_secs(seconds);
                (end > now).then_some(Until::At(end))
            }
        }
    }
}

impl State for Kept {
    const FILE: &'static str = "dnd.json";
    const VERSION: u32 = 1;

    fn legacy() -> Option<Self> {
        // no old file meant off, and so does one that can't be read
        let text = state::legacy_text(OLD_FILE).unwrap_or_default();
        Some(Kept::of(parse(&text, SystemTime::now())))
    }
}

/// The old file's one line: `forever`, or `until <unix seconds>`. A time already past
/// reads as off.
fn parse(text: &str, now: SystemTime) -> Option<Until> {
    let text = text.trim();
    if text == "forever" {
        return Some(Until::Forever);
    }
    let seconds: u64 = text.strip_prefix("until ")?.trim().parse().ok()?;
    let end = UNIX_EPOCH + Duration::from_secs(seconds);
    (end > now).then_some(Until::At(end))
}

fn save(until: Option<Until>) {
    state::save(&Kept::of(until));
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn at(hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 23)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    #[test]
    fn evening_is_offered_only_before_it() {
        assert_eq!(this_evening(at(14, 0)), Some(at(19, 0)));
        assert_eq!(this_evening(at(19, 0)), None);
        assert_eq!(this_evening(at(23, 30)), None);
    }

    #[test]
    fn tomorrow_is_the_next_morning() {
        let next = NaiveDate::from_ymd_opt(2026, 10, 24)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap();
        assert_eq!(tomorrow_morning(at(14, 0)), next);
        assert_eq!(tomorrow_morning(at(23, 59)), next);
        // past midnight, "tomorrow" means this morning
        assert_eq!(tomorrow_morning(at(2, 0)), at(8, 0));
    }

    #[test]
    fn the_evening_choice_goes_once_it_has_passed() {
        let labels = |now| -> Vec<&str> { choices(now).into_iter().map(|(l, _)| l).collect() };
        assert_eq!(
            labels(at(14, 0)),
            ["For 1 Hour", "Until This Evening", "Until Tomorrow"]
        );
        assert_eq!(labels(at(20, 0)), ["For 1 Hour", "Until Tomorrow"]);
    }

    #[test]
    fn the_kept_state_round_trips() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let later = UNIX_EPOCH + Duration::from_secs(5_000);
        for until in [None, Some(Until::Forever), Some(Until::At(later))] {
            let text = serde_json::to_string(&Kept::of(until)).unwrap();
            let kept: Kept = serde_json::from_str(&text).unwrap();
            assert_eq!(kept.until(now), until, "{text}");
        }
        assert_eq!(
            serde_json::to_string(&Kept::Until(5)).unwrap(),
            r#"{"until":5}"#
        );
        assert_eq!(Kept::Until(500).until(now), None);
    }

    #[test]
    fn a_bad_or_past_file_reads_as_off() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);

        assert_eq!(parse("until 500", now), None);
        assert_eq!(parse("until soon", now), None);
        assert_eq!(parse("", now), None);
        assert_eq!(parse("garbage\n", now), None);
        assert_eq!(parse("forever\n", now), Some(Until::Forever));
    }

    #[test]
    fn an_old_file_migrates_and_none_is_off() {
        use crate::services::state::tests::in_folder;
        in_folder("dnd-forever", &[(OLD_FILE, "forever\n")], |_| {
            assert_eq!(state::load::<Kept>(), Some(Kept::Forever));
        });
        in_folder("dnd-until", &[(OLD_FILE, "until 99999999999\n")], |_| {
            assert_eq!(state::load::<Kept>(), Some(Kept::Until(99_999_999_999)));
        });
        in_folder("dnd-off", &[], |_| {
            assert_eq!(state::load::<Kept>(), Some(Kept::Off));
        });
    }
}
