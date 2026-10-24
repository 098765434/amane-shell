//! Which page Quick Settings shows: the grid, or one tile's page.

use super::detail::DetailPage;
use super::tiles::{
    appearance, background, bluetooth, keyboard, network, notifications, power, session, sound,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Grid,
    Sound,
    Wifi,
    Vpn,
    Bluetooth,
    PowerMode,
    /// The battery, from the system row.
    Power,
    NightLight,
    DoNotDisturb,
    Keyboard,
    /// The power menu: suspend, restart, power off, log out.
    Session,
    BackgroundApps,
}

impl Page {
    /// Every page by its IPC name, the grid first.
    const ALL: [(&'static str, Page); 12] = [
        ("grid", Self::Grid),
        ("sound", Self::Sound),
        ("wifi", Self::Wifi),
        ("vpn", Self::Vpn),
        ("bluetooth", Self::Bluetooth),
        ("power-mode", Self::PowerMode),
        ("power", Self::Power),
        ("night-light", Self::NightLight),
        ("do-not-disturb", Self::DoNotDisturb),
        ("keyboard", Self::Keyboard),
        ("session", Self::Session),
        ("background-apps", Self::BackgroundApps),
    ];

    pub const NAMES: [&'static str; 12] = {
        let mut names = [""; 12];
        let mut i = 0;
        while i < names.len() {
            names[i] = Self::ALL[i].0;
            i += 1;
        }
        names
    };

    /// The page `quick-settings open <name>` asks for.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, page)| *page)
    }
}

/// The page itself. The grid isn't one, so it gets an empty page.
pub(super) fn page(page: Page) -> DetailPage {
    match page {
        Page::Grid => DetailPage::new("amane.qs.grid", ' ', ""),
        Page::Sound => sound::page(),
        Page::Wifi => network::wifi_page(),
        Page::Vpn => network::vpn_page(),
        Page::Bluetooth => bluetooth::page(),
        Page::PowerMode => power::mode_page(),
        Page::Power => power::battery_page(),
        Page::NightLight => appearance::night_light_page(),
        Page::DoNotDisturb => notifications::page(),
        Page::Keyboard => keyboard::page(),
        Page::Session => session::page(),
        Page::BackgroundApps => background::page(),
    }
}

/// `page` was left: forget what it was in the middle of.
pub(super) fn left(page: Page) {
    match page {
        Page::Session => session::reset(),
        Page::Wifi => network::forget_scan(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_has_a_name_that_finds_it() {
        for (name, page) in Page::ALL {
            assert_eq!(Page::named(name), Some(page));
        }
        assert_eq!(Page::NAMES[2], "wifi");
        assert_eq!(Page::named("nope"), None);
    }
}
