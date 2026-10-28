//! Do Not Disturb — hides toasts until it's turned off or its time runs out.
//!
//! Notifications still arrive and collect in the center, and critical ones
//! still show as toasts. The state is the one thing the shell keeps on disk
//! (`$XDG_STATE_HOME/amane-shell/dnd`), so a timed Do Not Disturb survives a
//! restart; a missing or unreadable file means off.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use amane::Service;
use chrono::{Days, Local, NaiveDateTime, NaiveTime, TimeZone};

use crate::surfaces::context_menu::{self, ContextMenu, MenuItem};

/// When a timed Do Not Disturb runs out: "this evening" and "tomorrow".
const EVENING: NaiveTime = NaiveTime::from_hms_opt(19, 0, 0).unwrap();
const MORNING: NaiveTime = NaiveTime::from_hms_opt(8, 0, 0).unwrap();

const MENU_KEY: &str = "amane.notifications.dnd";

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
        let until = std::fs::read_to_string(path())
            .ok()
            .and_then(|text| parse(&text, SystemTime::now()));
        Self { until }
    }

    // only a timed one changes by itself, and half a minute late is fine
    fn interval() -> Duration {
        Duration::from_secs(30)
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
    }

    pub fn toggle() {
        let on = Self::read().on();
        Self::set(if on { None } else { Some(Until::Forever) });
    }

    /// The menu of timed choices, hung under `parent_ns` at `point`.
    pub fn open_menu(parent_ns: &'static str, point: (i32, i32)) {
        let on = Self::read().on();

        let mut menu = ContextMenu::new();
        for (label, end) in choices(Local::now().naive_local()) {
            menu = menu.item(MenuItem::label(label).on_select(move || Self::set(Some(end))));
        }
        menu = menu.separator();
        menu = if on {
            menu.item(MenuItem::label("Turn Off").on_select(|| Self::set(None)))
        } else {
            menu.item(MenuItem::label("Turn On").on_select(|| Self::set(Some(Until::Forever))))
        };

        context_menu::show_under(MENU_KEY, menu, parent_ns, point);
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

fn path() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(std::env::temp_dir);
    state.join("amane-shell").join("dnd")
}

/// One line: `forever`, or `until <unix seconds>`. A time already past
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

fn format(until: Until) -> String {
    match until {
        Until::Forever => String::from("forever"),
        Until::At(end) => {
            let seconds = end.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            format!("until {seconds}")
        }
    }
}

fn save(until: Option<Until>) {
    let path = path();
    let result = match until {
        None => match std::fs::remove_file(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        },
        Some(until) => path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, format(until) + "\n")),
    };
    if let Err(e) = result {
        eprintln!("amane-shell: {}: {e}", path.display());
    }
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
    fn the_file_round_trips() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let later = UNIX_EPOCH + Duration::from_secs(5_000);

        assert_eq!(parse(&format(Until::Forever), now), Some(Until::Forever));
        assert_eq!(
            parse(&format(Until::At(later)), now),
            Some(Until::At(later))
        );
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
}
