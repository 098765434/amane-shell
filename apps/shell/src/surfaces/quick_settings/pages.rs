//! Which page Quick Settings shows: the grid, or one tile's page.

use super::controls::ControlId;
use super::detail::DetailPage;
use super::tiles::{
    appearance, background, bluetooth, keep_awake, keyboard, network, notifications, phone, power,
    session, sound, tailscale,
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
    Tailscale,
    KeepAwake,
    Phone,
}

impl Page {
    /// Every page by its IPC name, the grid first.
    const ALL: [(&'static str, Page); 15] = [
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
        ("tailscale", Self::Tailscale),
        ("keep-awake", Self::KeepAwake),
        ("phone", Self::Phone),
    ];

    pub const NAMES: [&'static str; 15] = {
        let mut names = [""; 15];
        let mut i = 0;
        while i < names.len() {
            names[i] = Self::ALL[i].0;
            i += 1;
        }
        names
    };

    /// The controls on the grid that open this page, the one it grows out
    /// of first.
    pub(super) const fn controls(self) -> &'static [ControlId] {
        match self {
            Self::Grid => &[],
            Self::Sound => &[ControlId::Sound],
            Self::Wifi => &[ControlId::Wifi, ControlId::Wired],
            Self::Vpn => &[ControlId::Vpn],
            Self::Bluetooth => &[ControlId::Bluetooth],
            Self::PowerMode => &[ControlId::PowerMode],
            Self::Power => &[ControlId::Battery],
            Self::NightLight => &[ControlId::NightLight],
            Self::DoNotDisturb => &[ControlId::Focus],
            Self::Keyboard => &[ControlId::Keyboard],
            Self::Session => &[ControlId::PowerOff],
            Self::BackgroundApps => &[ControlId::BackgroundApps],
            Self::Tailscale => &[ControlId::Tailscale],
            Self::KeepAwake => &[ControlId::KeepAwake],
            Self::Phone => &[ControlId::Phone],
        }
    }

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
        Page::Grid => DetailPage::new("amane.qs.grid", ""),
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
        Page::Tailscale => tailscale::page(),
        Page::KeepAwake => keep_awake::page(),
        Page::Phone => phone::page(),
    }
}

/// The control `page` is opened from, if it belongs to one.
fn control(page: Page) -> Option<ControlId> {
    Some(match page {
        Page::Grid | Page::Session | Page::BackgroundApps => return None,
        Page::Sound => ControlId::Sound,
        Page::Wifi => ControlId::Wifi,
        Page::Vpn => ControlId::Vpn,
        Page::Bluetooth => ControlId::Bluetooth,
        Page::PowerMode => ControlId::PowerMode,
        Page::Power => ControlId::Battery,
        Page::NightLight => ControlId::NightLight,
        Page::DoNotDisturb => ControlId::Focus,
        Page::Keyboard => ControlId::Keyboard,
        Page::Tailscale => ControlId::Tailscale,
        Page::KeepAwake => ControlId::KeepAwake,
        Page::Phone => ControlId::Phone,
    })
}

/// Whether `page` has what it shows: its control's hardware or daemon is
/// there. A page whose setting goes away while it's open gives way to the
/// grid, and IPC won't open one. Background Apps' page shows even with none.
pub(super) fn available(page: Page) -> bool {
    control(page).is_none_or(super::controls::available)
}

/// `page` was left: forget what it was in the middle of.
pub(super) fn left(page: Page) {
    match page {
        Page::Wifi => network::left_wifi(),
        Page::Bluetooth => bluetooth::left(),
        Page::Phone => phone::left(),
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

    #[test]
    fn every_page_but_the_grid_grows_out_of_a_control() {
        for (_, page) in Page::ALL {
            assert_eq!(page.controls().is_empty(), page == Page::Grid);
        }
    }
}
