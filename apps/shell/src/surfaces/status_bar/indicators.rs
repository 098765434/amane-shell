//! Which status icons the bar shows: volume, Bluetooth, network, Tailscale,
//! battery and Keep Awake are each optional, switched from their Quick
//! Settings page ("Show in Status Bar"), from Emaki Settings or over IPC;
//! so are the privacy icons, the icons for what's switched on, and the
//! notification bell. Kept as the hidden ones' names in
//! `~/.config/emaki/status-bar.json` (`StatusBar::hidden`), which the
//! bar follows live.

use emaki_kit::shell_ipc::{StatusBar as Command, Switch};

use emaki_kit::settings::{change_status_bar, status_bar};

use crate::services::status_apps::{self, AppItem};

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
            return Some(Self::App(emaki_kit::intern::intern(name)));
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

    /// Puts it back in the bar.
    pub fn show(self) {
        if self.hideable() {
            change_status_bar(|now| now.set_shown(self.name(), true));
        }
    }

    /// What Emaki Settings calls it: "Volume", "Apps", or a program's item.
    pub fn label(self) -> &'static str {
        if self.app_id().is_some() {
            return "the app's item";
        }
        emaki_kit::settings::BAR_ITEMS
            .iter()
            .find(|item| item.0 == self.name())
            .map_or("the item", |item| item.1)
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

/// `emaki` IPC: `status-bar (show | hide | toggle) <indicator>`,
/// `status-bar move <item> <index>`, `status-bar reset-order`, and the
/// tray's, programs' items' and activities' own.
pub fn run(command: Command) -> String {
    let (name, shown) = match command {
        Command::Move { item: name, index } => {
            let Some(item) = Item::named(&name) else {
                let items = Item::ALL.map(Item::name).join(" | ");
                return format!("unknown item {name}; one of {items}");
            };
            move_item(item, index);
            return String::from("ok");
        }
        Command::Tray(listed) => return super::tray::run(listed),
        Command::Apps(listed) => return super::apps::run(listed),
        Command::Activities(activities) => return super::activities::run(activities),
        Command::ResetOrder => {
            reset_order();
            return String::from("ok");
        }
        Command::Indicator { indicator, shown } => (indicator, shown),
    };
    let Some(indicator) = Indicator::named(&name) else {
        let names = Indicator::ALL.map(Indicator::name).join(" | ");
        return format!("unknown indicator {name}; one of {names}");
    };
    indicator.set_shown(match shown {
        Switch::On => true,
        Switch::Off => false,
        Switch::Toggle => !indicator.shown(),
    });
    String::from("ok")
}
