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
    /// A phone through KDE Connect, with its charge, while it's connected.
    Phone,
    KeepAwake,
    /// Screen sharing, camera and microphone, while in use.
    Privacy,
    /// Night Light, VPN, power mode and Airplane, while on.
    StatusIcons,
    /// The bell for unseen notifications, or the moon for Do Not Disturb.
    Notifications,
}

impl Indicator {
    pub const ALL: [Indicator; 10] = [
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
            Self::Volume => "volume",
            Self::Bluetooth => "bluetooth",
            Self::Network => "network",
            Self::Tailscale => "tailscale",
            Self::Battery => "battery",
            Self::Phone => "phone",
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

/// Every item on the bar's right, in the order it has them unless
/// `StatusBar::order` says otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    Privacy,
    StatusIcons,
    Volume,
    Bluetooth,
    Network,
    Tailscale,
    KeepAwake,
    Phone,
    Battery,
    QuickSettings,
    Notifications,
    Timer,
    Clock,
}

impl Item {
    /// The usual order, left to right.
    pub const ALL: [Item; 13] = [
        Self::Privacy,
        Self::StatusIcons,
        Self::Volume,
        Self::Bluetooth,
        Self::Network,
        Self::Tailscale,
        Self::KeepAwake,
        Self::Phone,
        Self::Battery,
        Self::QuickSettings,
        Self::Notifications,
        Self::Timer,
        Self::Clock,
    ];

    pub fn name(self) -> &'static str {
        match self.indicator() {
            Some(indicator) => indicator.name(),
            None => match self {
                Self::QuickSettings => "quick-settings",
                Self::Timer => "timer",
                _ => "clock",
            },
        }
    }

    /// What Shell Settings calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Privacy => "Camera, Microphone and Screen Sharing",
            Self::StatusIcons => "Settings That Are On",
            Self::Volume => "Sound",
            Self::Bluetooth => "Bluetooth",
            Self::Network => "Wi‑Fi",
            Self::Tailscale => "Tailscale",
            Self::KeepAwake => "Keep Awake",
            Self::Phone => "Phone",
            Self::Battery => "Battery",
            Self::QuickSettings => "Quick Settings",
            Self::Notifications => "Notifications",
            Self::Timer => "Timer",
            Self::Clock => "Clock",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|item| item.name() == name)
    }

    /// The switch that hides it, for the ones that can be hidden; Quick
    /// Settings and the clock always show, as macOS's Control Center and
    /// clock do, and the timer shows while it runs.
    pub fn indicator(self) -> Option<Indicator> {
        Some(match self {
            Self::Privacy => Indicator::Privacy,
            Self::StatusIcons => Indicator::StatusIcons,
            Self::Volume => Indicator::Volume,
            Self::Bluetooth => Indicator::Bluetooth,
            Self::Network => Indicator::Network,
            Self::Tailscale => Indicator::Tailscale,
            Self::KeepAwake => Indicator::KeepAwake,
            Self::Phone => Indicator::Phone,
            Self::Battery => Indicator::Battery,
            Self::Notifications => Indicator::Notifications,
            Self::QuickSettings | Self::Timer | Self::Clock => return None,
        })
    }

    /// Whether it's switched on; whether it has anything to show is its own.
    pub fn shown(self) -> bool {
        self.indicator().is_none_or(Indicator::shown)
    }

    /// The items in the order the settings keep them.
    pub fn ordered() -> Vec<Item> {
        let names = status_bar().order.clone();
        ordered(&names)
    }
}

/// `order`'s items, ignoring names it doesn't know, with the ones it
/// leaves out each put back after the item it usually follows.
fn ordered(order: &[String]) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    for name in order {
        if let Some(item) = Item::named(name)
            && !items.contains(&item)
        {
            items.push(item);
        }
    }
    for (index, item) in Item::ALL.into_iter().enumerate() {
        if items.contains(&item) {
            continue;
        }
        let after = Item::ALL[..index]
            .iter()
            .rev()
            .find_map(|before| items.iter().position(|placed| placed == before));
        let at = after.map_or(0, |at| at + 1);
        items.insert(at, item);
    }
    items
}

/// Moves `item` to `index` among the others and keeps the order.
pub fn move_item(item: Item, index: usize) {
    change_status_bar(|now| {
        let mut items = ordered(&now.order);
        items.retain(|other| *other != item);
        items.insert(index.min(items.len()), item);
        now.order = items.iter().map(|item| String::from(item.name())).collect();
    });
}

/// Back to the usual order.
pub fn reset_order() {
    change_status_bar(|now| now.order.clear());
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
        ["reset-order"] => {
            reset_order();
            return String::from("ok");
        }
        [action @ ("show" | "hide" | "toggle"), name] => (*action, *name),
        _ => {
            return format!(
                "usage: status-bar (show | hide | toggle) ({names}) | move <item> <index> | reset-order"
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

    fn names(items: &[Item]) -> Vec<&'static str> {
        items.iter().map(|item| item.name()).collect()
    }

    #[test]
    fn no_order_is_the_usual_one() {
        assert_eq!(ordered(&[]), Item::ALL);
    }

    #[test]
    fn items_left_out_of_the_order_follow_their_usual_neighbour() {
        let order = ["clock", "volume", "nonsense", "battery"].map(String::from);
        let items = ordered(&order);
        assert_eq!(items.len(), Item::ALL.len());
        assert_eq!(&names(&items)[..2], ["privacy", "status-icons"]);
        assert_eq!(items[2], Item::Clock);
        // the rest of the volume's neighbours come after it, then the battery
        let volume = items.iter().position(|item| *item == Item::Volume).unwrap();
        assert_eq!(items[volume + 1], Item::Bluetooth);
        let battery = items
            .iter()
            .position(|item| *item == Item::Battery)
            .unwrap();
        assert_eq!(items[battery + 1], Item::QuickSettings);
    }

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
