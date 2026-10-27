//! Which status icons the bar shows: volume, Bluetooth, network, Tailscale,
//! battery and Keep Awake are each optional, switched from their Quick
//! Settings page ("Show in Status Bar"), from Shell Settings or over IPC;
//! so are the privacy icons, the icons for what's switched on, and the
//! notification bell. Kept as the hidden ones' names in
//! `~/.config/amane-shell/status-bar.json` (`StatusBar::hidden`), which the
//! bar follows live; they were once a state file, read in once.

use amane::Config;
use amane_kit::settings::{StatusBar, change_status_bar, status_bar};

use crate::services::state_file;

/// Where they were kept before the settings file, read in once when there
/// is no settings file yet.
const STATE_FILE: &str = "status-bar";

/// Where they were kept while the bar was the taskbar.
const OLD_FILE: &str = "taskbar";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Indicator {
    Volume,
    Bluetooth,
    Network,
    Tailscale,
    Battery,
    KeepAwake,
    /// Screen sharing, camera and microphone, while in use.
    Privacy,
    /// Night Light, VPN, power mode and Airplane, while on.
    StatusIcons,
    /// The bell for unseen notifications, or the moon for Do Not Disturb.
    Notifications,
}

impl Indicator {
    pub const ALL: [Indicator; 9] = [
        Self::Volume,
        Self::Bluetooth,
        Self::Network,
        Self::Tailscale,
        Self::Battery,
        Self::KeepAwake,
        Self::Privacy,
        Self::StatusIcons,
        Self::Notifications,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Volume => "volume",
            Self::Bluetooth => "bluetooth",
            Self::Network => "network",
            Self::Tailscale => "tailscale",
            Self::Battery => "battery",
            Self::KeepAwake => "keep-awake",
            Self::Privacy => "privacy",
            Self::StatusIcons => "status-icons",
            Self::Notifications => "notifications",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|indicator| indicator.name() == name)
    }

    pub fn shown(self) -> bool {
        status_bar().shows(self.name())
    }

    pub fn set_shown(self, shown: bool) {
        change_status_bar(|now| now.set_shown(self.name(), shown));
    }
}

/// Brings the hidden icons over from the old state file, once: when there
/// is no settings file yet. Called at start, before any window.
pub fn migrate() {
    if Config::<StatusBar>::path().exists() {
        return;
    }

    let hidden = load(|file| std::fs::read_to_string(state_file(file)).ok());

    if !hidden.is_empty() {
        change_status_bar(|now| now.hidden = hidden);
    }
}

/// The hidden ones' names, from the state file or else the old one.
fn load(read: impl Fn(&str) -> Option<String>) -> Vec<String> {
    read(STATE_FILE)
        .or_else(|| read(OLD_FILE))
        .map(|text| parse(&text))
        .unwrap_or_default()
}

fn parse(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| Indicator::named(line.trim()))
        .map(|indicator| String::from(indicator.name()))
        .collect()
}

/// `amane-shell` IPC: `status-bar (show | hide | toggle) <indicator>`, also
/// still answering to `taskbar`.
pub fn ipc(args: &[String]) -> String {
    let names = Indicator::ALL.map(Indicator::name).join(" | ");

    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (action, name) = match args.as_slice() {
        [action @ ("show" | "hide" | "toggle"), name] => (*action, *name),
        _ => return format!("usage: status-bar (show | hide | toggle) ({names})"),
    };
    let Some(indicator) = Indicator::named(name) else {
        return format!("unknown indicator {name}; one of {names}");
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
    fn the_state_file_is_read_ignoring_what_it_doesnt_know() {
        assert_eq!(parse("wifi\n network \nbattery\n"), ["network", "battery"]);
    }

    #[test]
    fn the_old_file_is_read_until_there_is_a_new_one() {
        let old = |file: &str| (file == OLD_FILE).then(|| String::from("battery\n"));
        assert_eq!(load(old), ["battery"]);

        let both = |file: &str| {
            Some(String::from(if file == STATE_FILE {
                "volume\n"
            } else {
                "battery\n"
            }))
        };
        assert_eq!(load(both), ["volume"]);

        assert!(load(|_| None).is_empty());
    }
}
