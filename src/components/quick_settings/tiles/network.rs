//! The network toggles — GNOME's Wired, Wi‑Fi and VPN, each a tile of
//! its own.
//!
//! Wi‑Fi's toggle switches the radio and opens the Wi‑Fi panel's networks
//! (rescanned every 15 s while shown, as GNOME does); while the machine is
//! a hotspot it says so and a click stops it. Wired shows only with a cable
//! in, VPN only when NetworkManager has a VPN profile.

use std::cell::Cell as Slot;
use std::time::{Duration, Instant};

use amane::{Link, Network, ScrollArea, Service, Size};

use super::super::{Page, open};
use super::{connected_count, settings_link};
use crate::components::buttons::Switch;
use crate::components::icons::{
    ICON_HOTSPOT, ICON_VPN, ICON_WIFI_NONE, ICON_WIFI_OFF, ICON_WIRED, wifi_glyph,
};
use crate::components::layout::boxed;
use crate::components::list::{List, ListItem};
use crate::components::motion::redraw_after;
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::detail::page_width;
use crate::components::quick_settings::tile::QuickToggle;
use crate::components::taskbar::wifi;
use crate::services::connections::Connections;
use crate::services::connections::Vpn;
use crate::services::rfkill::Rfkill;

/// How often the Wi‑Fi page looks for networks again.
const RESCAN: Duration = Duration::from_secs(15);

const NETWORK_SETTINGS: &str = "nm-connection-editor";

thread_local! {
    static LAST_SCAN: Slot<Option<Instant>> = const { Slot::new(None) };
}

pub(crate) fn wired_toggle() -> Option<QuickToggle> {
    let wired = Connections::read().wired().cloned()?;

    let device = wired.device.clone();
    let subtitle = if wired.connected {
        wired.name
    } else {
        String::from("Not Connected")
    };
    Some(
        QuickToggle::new("amane.qs.wired", ICON_WIRED, "Wired")
            .subtitle(subtitle)
            .checked(wired.connected)
            .on_toggle(move |on| Connections::set_wired(&device, on)),
    )
}

/// Whether there's Wi‑Fi to show: a radio, or a hotspot running on one.
pub(crate) fn wifi_available() -> bool {
    Connections::read().hotspot().is_some() || Rfkill::read().has("wlan")
}

pub(crate) fn vpn_available() -> bool {
    !Connections::read().vpns().is_empty()
}

pub(crate) fn wifi_toggle() -> Option<QuickToggle> {
    if let Some(hotspot) = Connections::read().hotspot().cloned() {
        // the disc stops the hotspot; the tile opens the networks
        return Some(
            QuickToggle::new(
                "amane.qs.wifi",
                ICON_HOTSPOT,
                format!("{} Hotspot", hotspot.ssid),
            )
            .subtitle("Sharing this connection")
            .checked(true)
            .on_press(move || Connections::stop_hotspot(&hotspot.uuid))
            .on_open(|| open(Page::Wifi)),
        );
    }

    // a machine with no Wi‑Fi radio has nothing to switch
    if !Rfkill::read().has("wlan") {
        return None;
    }

    let net = Network::read();
    let enabled = net.wifi_enabled();
    // a hardware switch holds the radio off, so the disc can't turn it on;
    // the page still opens
    let held = Rfkill::read().hard_blocked("wlan");
    let subtitle = if held {
        String::from("Hardware switch off")
    } else if !enabled {
        String::from("Off")
    } else if net.connecting() {
        String::from("Connecting…")
    } else if net.link() == Link::Wifi {
        net.ssid().to_string()
    } else {
        String::from("Not Connected")
    };
    let glyph = match (enabled, net.link()) {
        (false, _) => ICON_WIFI_OFF,
        (true, Link::Wifi) => wifi_glyph(net.strength()),
        (true, _) => ICON_WIFI_NONE,
    };
    drop(net);

    Some(
        QuickToggle::new("amane.qs.wifi", glyph, "Wi‑Fi")
            .subtitle(subtitle)
            .checked(enabled)
            .on_toggle(move |on| {
                if !held {
                    Network::set_wifi(on);
                }
            })
            .on_open(|| open(Page::Wifi)),
    )
}

pub(crate) fn wifi_page() -> DetailPage {
    let net = Network::read();
    let enabled = net.wifi_enabled();
    let subtitle = (net.link() == Link::Wifi).then(|| net.ssid().to_string());
    drop(net);

    // GNOME looks again every 15 s while its menu is open
    if enabled && LAST_SCAN.get().is_none_or(|at| at.elapsed() >= RESCAN) {
        LAST_SCAN.set(Some(Instant::now()));
        Network::scan();
        redraw_after(RESCAN);
    }

    let page = DetailPage::new("amane.qs.wifi.page", "Wi‑Fi")
        .subtitle(
            subtitle.unwrap_or_else(|| String::from(if enabled { "Not Connected" } else { "Off" })),
        )
        .switch(enabled, Network::set_wifi)
        // the page stays open to show the network connecting
        .body(|height| wifi::networks_in(height, page_width(), || {}));
    settings_link(page, "Network Settings…", NETWORK_SETTINGS)
}

/// The Wi‑Fi page was left: the next one scans straight away.
pub(crate) fn forget_scan() {
    LAST_SCAN.set(None);
}

pub(crate) fn vpn_toggle() -> Option<QuickToggle> {
    let connections = Connections::read();
    if connections.vpns().is_empty() {
        return None;
    }
    let active: Vec<_> = connections
        .vpns()
        .iter()
        .filter(|vpn| vpn.active)
        .cloned()
        .collect();
    let preferred = connections.preferred_vpn().cloned();
    drop(connections);

    Some(
        QuickToggle::new("amane.qs.vpn", ICON_VPN, "VPN")
            .subtitle(vpn_status(&active))
            .checked(!active.is_empty())
            .on_toggle(move |on| set_vpns(on, &preferred, &active))
            .on_open(|| open(Page::Vpn)),
    )
}

/// "Off", the tunnel that's up, or how many are.
fn vpn_status(active: &[Vpn]) -> String {
    match active {
        [] => String::from("Off"),
        [one] => one.name.clone(),
        many => connected_count(many.len()),
    }
}

/// On: the preferred tunnel comes up. Off: every active one goes down.
fn set_vpns(on: bool, preferred: &Option<Vpn>, active: &[Vpn]) {
    if on {
        if let Some(vpn) = preferred {
            Connections::set_vpn(&vpn.uuid, true);
        }
    } else {
        for vpn in active {
            Connections::set_vpn(&vpn.uuid, false);
        }
    }
}

pub(crate) fn vpn_page() -> DetailPage {
    let connections = Connections::read();
    let vpns = connections.vpns().to_vec();
    let preferred = connections.preferred_vpn().cloned();
    drop(connections);
    let active: Vec<Vpn> = vpns.iter().filter(|vpn| vpn.active).cloned().collect();
    let any = !active.is_empty();

    let page = DetailPage::new("amane.qs.vpn.page", "VPN")
        .subtitle(vpn_status(&active))
        .switch(any, move |on| set_vpns(on, &preferred, &active))
        .body(move |height| {
            let items: Vec<ListItem> = vpns
                .iter()
                .map(|vpn| {
                    let uuid = vpn.uuid.clone();
                    ListItem::new(format!("amane.qs.vpn.{}", vpn.uuid), vpn.name.clone())
                        .leading(ICON_VPN)
                        .selected(vpn.active)
                        .trailing(
                            Switch::new(format!("amane.qs.vpn.{}.switch", vpn.uuid), vpn.active)
                                .on_toggle(move |on| Connections::set_vpn(&uuid, on))
                                .build(),
                        )
                })
                .collect();
            vec![boxed(
                ScrollArea::new(
                    "amane.qs.vpn.list",
                    List::new(items)
                        .header("VPN Connections")
                        .empty("No VPN connections")
                        .build(),
                )
                .height(Size::Fixed(height)),
            )]
        });
    settings_link(page, "VPN Settings…", NETWORK_SETTINGS)
}
