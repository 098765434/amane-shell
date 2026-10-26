//! Which status icons the bar shows: volume, Bluetooth, network, Tailscale,
//! battery and Keep Awake are each optional, switched from their Quick
//! Settings page ("Show in Status Bar") or over IPC. Kept in
//! `$XDG_STATE_HOME/amane-shell/status-bar` as the hidden ones' names, one
//! per line; a missing file shows them all.

use std::cell::RefCell;
use std::collections::HashSet;

use crate::services::{save_state, state_file};

amane_kit::marker! {
    /// An indicator was shown or hidden: what asked whether one shows
    /// (the bars, Customize) draws again.
    pub Indicators
}

const FILE: &str = "status-bar";

/// Where they were kept while the bar was the taskbar, read when the new
/// file isn't there yet.
const OLD_FILE: &str = "taskbar";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Indicator {
    Volume,
    Bluetooth,
    Network,
    Tailscale,
    Battery,
    KeepAwake,
}

impl Indicator {
    const ALL: [Indicator; 6] = [
        Self::Volume,
        Self::Bluetooth,
        Self::Network,
        Self::Tailscale,
        Self::Battery,
        Self::KeepAwake,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Volume => "volume",
            Self::Bluetooth => "bluetooth",
            Self::Network => "network",
            Self::Tailscale => "tailscale",
            Self::Battery => "battery",
            Self::KeepAwake => "keep-awake",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|indicator| indicator.name() == name)
    }

    pub fn shown(self) -> bool {
        Indicators::watch();
        HIDDEN.with_borrow(|hidden| !hidden.contains(&self))
    }

    pub fn set_shown(self, shown: bool) {
        let changed = HIDDEN.with_borrow_mut(|hidden| {
            if shown {
                hidden.remove(&self)
            } else {
                hidden.insert(self)
            }
        });
        if changed {
            HIDDEN.with_borrow(|hidden| save_state(FILE, &format(hidden)));
            Indicators::changed();
        }
    }
}

thread_local! {
    static HIDDEN: RefCell<HashSet<Indicator>> = RefCell::new(load(|file| {
        std::fs::read_to_string(state_file(file)).ok()
    }));
}

/// The hidden ones, from the file or else the old one.
fn load(read: impl Fn(&str) -> Option<String>) -> HashSet<Indicator> {
    read(FILE)
        .or_else(|| read(OLD_FILE))
        .map(|text| parse(&text))
        .unwrap_or_default()
}

fn parse(text: &str) -> HashSet<Indicator> {
    text.lines()
        .filter_map(|line| Indicator::named(line.trim()))
        .collect()
}

/// In a fixed order, so the file doesn't churn.
fn format(hidden: &HashSet<Indicator>) -> String {
    Indicator::ALL
        .into_iter()
        .filter(|indicator| hidden.contains(indicator))
        .map(|indicator| format!("{}\n", indicator.name()))
        .collect()
}

/// `amane-shell` IPC: `status-bar (show | hide | toggle) <indicator>`, also
/// still answering to `taskbar`.
pub fn ipc(args: &[String]) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (action, name) = match args.as_slice() {
        [action @ ("show" | "hide" | "toggle"), name] => (*action, *name),
        _ => {
            return String::from(
                "usage: status-bar (show | hide | toggle) (volume | bluetooth | network | tailscale | battery | keep-awake)",
            );
        }
    };
    let Some(indicator) = Indicator::named(name) else {
        return format!(
            "unknown indicator {name}; volume, bluetooth, network, tailscale, battery or keep-awake"
        );
    };
    indicator.set_shown(match action {
        "show" => true,
        "hide" => false,
        _ => !indicator.shown(),
    });
    String::from("ok")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_round_trips_and_ignores_what_it_doesnt_know() {
        let hidden: HashSet<_> = [Indicator::Battery, Indicator::Volume].into();
        assert_eq!(format(&hidden), "volume\nbattery\n");
        assert_eq!(parse(&format(&hidden)), hidden);
        assert_eq!(parse("wifi\n network \n"), [Indicator::Network].into());
    }

    #[test]
    fn the_old_file_is_read_until_there_is_a_new_one() {
        let old = |file: &str| (file == OLD_FILE).then(|| String::from("battery\n"));
        assert_eq!(load(old), [Indicator::Battery].into());

        let both = |file: &str| {
            Some(String::from(if file == FILE {
                "volume\n"
            } else {
                "battery\n"
            }))
        };
        assert_eq!(load(both), [Indicator::Volume].into());

        assert!(load(|_| None).is_empty());
    }
}
