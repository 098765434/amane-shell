//! The system controls — GNOME's top row, at the bottom as Control Center
//! has it: the battery tile (opening its page), then round buttons to take
//! a screenshot, lock the screen and open the power menu.

use amane::{Battery, BatteryState, Service, Windows};

use super::tile::QuickToggle;
use super::{Page, open};
use crate::components::icons::{ICON_LOCK, ICON_POWER, ICON_SCREENSHOT, battery_glyph};
use crate::components::motion;
use crate::components::taskbar::battery_low;
use crate::components::taskbar::close;
use crate::components::taskbar::power::battery_detail;
use crate::services::session::Session;

/// The battery, when there is one: its charge and how long it lasts.
pub(super) fn battery_toggle() -> Option<QuickToggle> {
    let battery = Battery::read();
    if !battery.present() {
        return None;
    }
    let percent = battery.percent();
    let state = battery.state();
    let low = battery_low(&battery);
    let detail = battery_detail(&battery);
    drop(battery);

    // the charge, and how long it lasts when that's known ("on battery"
    // says nothing the percentage doesn't)
    let subtitle = match state {
        BatteryState::Full => String::from("Full"),
        _ if detail.is_empty() || detail == "on battery" => format!("{percent}%"),
        _ => format!("{percent}% · {detail}"),
    };
    let charging = state == BatteryState::Charging;
    Some(
        QuickToggle::new(
            "amane.qs.battery",
            battery_glyph(percent, charging),
            "Battery",
        )
        .subtitle(subtitle)
        .alert(low)
        .on_open(|| open(Page::Power)),
    )
}

pub(super) fn screenshot_toggle() -> QuickToggle {
    QuickToggle::new("amane.qs.screenshot", ICON_SCREENSHOT, "Screenshot").on_press(|| {
        close();
        // the card has to have faded away before the screenshot UI
        // freezes the screen, so the card isn't in it
        std::thread::spawn(|| {
            std::thread::sleep(motion::CLOSE * 2);
            Windows::screenshot();
        });
    })
}

pub(super) fn lock_toggle() -> QuickToggle {
    QuickToggle::new("amane.qs.lock", ICON_LOCK, "Lock Screen").on_press(|| {
        close();
        Session::lock();
    })
}

pub(super) fn power_off_toggle() -> QuickToggle {
    QuickToggle::new("amane.qs.power", ICON_POWER, "Power Off…").on_open(|| open(Page::Session))
}
