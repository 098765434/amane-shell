//! Which status icons the bar shows: volume, Bluetooth, network, Tailscale,
//! battery and Keep Awake are each optional, switched from their Quick
//! Settings page ("Show in Status Bar"), from Shell Settings or over IPC;
//! so are the privacy icons, the icons for what's switched on, and the
//! notification bell. Kept as the hidden ones' names in
//! `~/.config/amane-shell/status-bar.json` (`StatusBar::hidden`), which the
//! bar follows live; they were once a state file, read in once.

use std::sync::{Mutex, PoisonError};

use amane::Config;
use amane_kit::settings::{StatusBar, change_status_bar, status_bar};

use crate::services::state_file;
use crate::services::status_apps::{self, AppItem};

/// Where they were kept before the settings file, read in once when there
/// is no settings file yet.
const STATE_FILE: &str = "status-bar";

/// Where they were kept while the bar was the taskbar.
const OLD_FILE: &str = "taskbar";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Indicator {
    /// Apps' own items: the system tray.
    Tray,
    Volume,
    Bluetooth,
    Network,
    Tailscale,
    Battery,
    /// A phone through KDE Connect, with its charge, while it's connected.
    Phone,
    KeepAwake,
    /// Screen sharing, camera and microphone, while in use.
    Privacy,
    /// Night Light, VPN, power mode and Airplane, while on.
    StatusIcons,
    /// Activities: what's playing, the timer, screen sharing, and
    /// other programs' (`activities.rs`).
    Activities,
    Weather,
    /// The keyboard layout, with two or more.
    Keyboard,
    /// The bell for unseen notifications, or the moon for Do Not Disturb.
    Notifications,
}

impl Indicator {
    pub const ALL: [Indicator; 14] = [
        Self::Activities,
        Self::Weather,
        Self::Keyboard,
        Self::Tray,
        Self::Volume,
        Self::Bluetooth,
        Self::Network,
        Self::Tailscale,
        Self::Battery,
        Self::Phone,
        Self::KeepAwake,
        Self::Privacy,
        Self::StatusIcons,
        Self::Notifications,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Tray => "tray",
            Self::Volume => "volume",
            Self::Bluetooth => "bluetooth",
            Self::Network => "network",
            Self::Tailscale => "tailscale",
            Self::Battery => "battery",
            Self::Phone => "phone",
            Self::KeepAwake => "keep-awake",
            Self::Privacy => "privacy",
            Self::StatusIcons => "status-icons",
            Self::Activities => "activities",
            Self::Weather => "weather",
            Self::Keyboard => "keyboard",
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

/// Every item on the bar's right, in the order it has them unless
/// `StatusBar::order` says otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    Tray,
    Privacy,
    StatusIcons,
    Activities,
    Weather,
    Volume,
    Bluetooth,
    Network,
    Tailscale,
    KeepAwake,
    Keyboard,
    Phone,
    Battery,
    QuickSettings,
    Notifications,
    Clock,
    /// A program's own item, by its name in the order (`app:<id>`).
    App(&'static str),
}

/// Program items' names, kept for as long as the shell runs so an `Item`
/// stays `Copy`; there are as many as programs that ever put one up.
static NAMES: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

fn intern(name: &str) -> &'static str {
    let mut names = NAMES.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(known) = names.iter().find(|known| **known == name) {
        return known;
    }
    let kept: &'static str = Box::leak(name.to_owned().into_boxed_str());
    names.push(kept);
    kept
}

impl Item {
    /// The usual order, left to right.
    pub const ALL: [Item; 16] = [
        Self::Tray,
        Self::Privacy,
        Self::StatusIcons,
        Self::Activities,
        Self::Weather,
        Self::Volume,
        Self::Bluetooth,
        Self::Network,
        Self::Tailscale,
        Self::KeepAwake,
        Self::Keyboard,
        Self::Phone,
        Self::Battery,
        Self::QuickSettings,
        Self::Notifications,
        Self::Clock,
    ];

    pub fn name(self) -> &'static str {
        if let Self::App(name) = self {
            return name;
        }
        match self.indicator() {
            Some(indicator) => indicator.name(),
            None => match self {
                Self::QuickSettings => "quick-settings",
                _ => "clock",
            },
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        if name.starts_with("app:") {
            return Some(Self::App(intern(name)));
        }
        Self::ALL.into_iter().find(|item| item.name() == name)
    }

    /// A program's item's id, for its own items.
    pub fn app_id(self) -> Option<&'static str> {
        match self {
            Self::App(name) => name.strip_prefix("app:"),
            _ => None,
        }
    }

    /// Whether it can be hidden, by dragging it off the bar too.
    pub fn hideable(self) -> bool {
        self.app_id().is_some() || self.indicator().is_some()
    }

    /// Takes it out of the bar.
    pub fn hide(self) {
        match self.indicator() {
            Some(indicator) => indicator.set_shown(false),
            None if self.app_id().is_some() => {
                change_status_bar(|now| now.set_shown(self.name(), false));
            }
            None => {}
        }
    }

    /// The switch that hides it, for the ones that can be hidden; Quick
    /// Settings and the clock always show.
    pub fn indicator(self) -> Option<Indicator> {
        Some(match self {
            Self::Tray => Indicator::Tray,
            Self::Privacy => Indicator::Privacy,
            Self::StatusIcons => Indicator::StatusIcons,
            Self::Activities => Indicator::Activities,
            Self::Weather => Indicator::Weather,
            Self::Keyboard => Indicator::Keyboard,
            Self::Volume => Indicator::Volume,
            Self::Bluetooth => Indicator::Bluetooth,
            Self::Network => Indicator::Network,
            Self::Tailscale => Indicator::Tailscale,
            Self::KeepAwake => Indicator::KeepAwake,
            Self::Phone => Indicator::Phone,
            Self::Battery => Indicator::Battery,
            Self::Notifications => Indicator::Notifications,
            Self::QuickSettings | Self::Clock | Self::App(_) => return None,
        })
    }

    /// Whether it's switched on; whether it has anything to show is its own.
    pub fn shown(self) -> bool {
        if self.app_id().is_some() {
            return status_bar().shows(self.name());
        }
        self.indicator().is_none_or(Indicator::shown)
    }

    /// The items in the order the settings keep them, programs' own among
    /// them.
    pub fn ordered() -> Vec<Item> {
        status_bar()
            .items_with(&app_names())
            .iter()
            .filter_map(|name| Item::named(name))
            .collect()
    }
}

/// The names of the programs' items up now.
fn app_names() -> Vec<String> {
    status_apps::items().iter().map(AppItem::name).collect()
}

/// Moves `item` to `index` among the others and keeps the order.
pub fn move_item(item: Item, index: usize) {
    let apps = app_names();
    change_status_bar(|now| now.move_item_with(item.name(), index, &apps));
}

/// Back to the usual order.
pub fn reset_order() {
    change_status_bar(|now| now.order.clear());
}

/// Brings the hidden icons over from the old state file, once: when there
/// is no settings file yet. Called at start, before any window. Now
/// Playing hidden before it became an activity is kept out as one.
pub fn migrate() {
    if status_bar().hidden.iter().any(|name| name == "media") {
        change_status_bar(|now| {
            now.hidden.retain(|name| name != "media");
            now.set_activity_allowed(super::activities::NOW_PLAYING, false);
        });
    }

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

/// `amane-shell` IPC: `status-bar (show | hide | toggle) <indicator>`,
/// `status-bar move <item> <index>` and `status-bar reset-order`, also
/// still answering to `taskbar`.
pub fn ipc(args: &[String]) -> String {
    let names = Indicator::ALL.map(Indicator::name).join(" | ");

    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (action, name) = match args.as_slice() {
        ["move", name, index] => {
            let Some(item) = Item::named(name) else {
                let items = Item::ALL.map(Item::name).join(" | ");
                return format!("unknown item {name}; one of {items}");
            };
            let Ok(index) = index.parse::<usize>() else {
                return format!("not an index: {index}");
            };
            move_item(item, index);
            return String::from("ok");
        }
        ["tray", rest @ ..] => return super::tray::ipc(rest),
        ["apps", rest @ ..] => return super::apps::ipc(rest),
        ["activities", rest @ ..] => return super::activities::ipc(rest),
        ["reset-order"] => {
            reset_order();
            return String::from("ok");
        }
        [action @ ("show" | "hide" | "toggle"), name] => (*action, *name),
        _ => {
            return format!(
                "usage: status-bar (show | hide | toggle) ({names}) | move <item> <index> | reset-order | tray … | apps … | activities …"
            );
        }
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
