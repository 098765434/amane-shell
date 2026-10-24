//! The network tiles — GNOME's Wired, Wi-Fi and VPN toggles.
//!
//! Wi-Fi's tile switches the radio and opens the Wi-Fi panel's networks
//! (rescanned every 15 s while shown, as GNOME does); while the machine is
//! a hotspot it says so and a click stops it. Wired shows only with a cable
//! in, VPN only when NetworkManager has a VPN profile.

use std::cell::Cell as Slot;
use std::time::{Duration, Instant};

use amane::{Link, Network, Service, Widget};

use super::{GridCell, on_off, settings_link};
use crate::components::buttons::Switch;
use crate::components::icons::{
    ICON_HOTSPOT, ICON_VPN, ICON_WIFI, ICON_WIFI_OFF, ICON_WIRED, wifi_glyph,
};
use crate::components::layout::{boxed, caption, text_height};
use crate::components::list::{List, ListItem};
use crate::components::motion::redraw_after;
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::grid::Cell;
use crate::components::quick_settings::tile::QuickToggle;
use crate::components::quick_settings::{Page, open};
use crate::components::taskbar::wifi;
use crate::services::connections::Connections;
use crate::services::rfkill::Rfkill;
use crate::tokens::sys;

/// How often the Wi-Fi page looks for networks again.
const RESCAN: Duration = Duration::from_secs(15);

const NETWORK_SETTINGS: &str = "nm-connection-editor";

thread_local! {
    static LAST_SCAN: Slot<Option<Instant>> = const { Slot::new(None) };
}

pub(crate) fn wired_cell() -> Option<GridCell> {
    let wired = Connections::read().wired().cloned()?;

    let device = wired.device.clone();
    let subtitle = if wired.connected {
        wired.name
    } else {
        String::from("Disconnected")
    };
    let tile = QuickToggle::new("amane.qs.wired", ICON_WIRED, "Wired")
        .subtitle(subtitle)
        .checked(wired.connected)
        .on_toggle(move |on| Connections::set_wired(&device, on))
        .build();
    Some(Cell::Half(boxed(tile)))
}

pub(crate) fn wifi_cell() -> Option<GridCell> {
    if let Some(hotspot) = Connections::read().hotspot().cloned() {
        let tile = QuickToggle::new(
            "amane.qs.wifi",
            ICON_HOTSPOT,
            format!("{} Hotspot", hotspot.ssid),
        )
        .subtitle("Click to stop")
        .checked(true)
        .on_press(move || Connections::stop_hotspot(&hotspot.uuid))
        .build();
        return Some(Cell::Half(boxed(tile)));
    }

    // a machine with no Wi-Fi radio has nothing to switch
    if !Rfkill::read().has("wlan") {
        return None;
    }

    let net = Network::read();
    let enabled = net.wifi_enabled();
    let subtitle = if !enabled {
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
        (true, _) => ICON_WIFI,
    };
    drop(net);

    // a hardware switch holds the radio off, so the tile can't turn it on
    let held = Rfkill::read().hard_blocked("wlan");
    let tile = QuickToggle::new("amane.qs.wifi", glyph, "Wi-Fi")
        .subtitle(subtitle)
        .checked(enabled)
        .disabled(held)
        .on_toggle(Network::set_wifi)
        .on_open(|| open(Page::Wifi))
        .build();
    Some(Cell::Half(boxed(tile)))
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

    let asking = wifi::asking_password();
    let mut height = sys::size::list::max_height();
    if asking {
        // the password section under the list: a caption and the field
        height += sys::space::gap::l()
            + text_height(sys::r#type::caption())
            + sys::space::gap::s()
            + sys::size::control::height();
    }

    let mut page = DetailPage::new("amane.qs.wifi.page", ICON_WIFI, "Wi-Fi")
        .switch(enabled, Network::set_wifi)
        .body(wifi::networks(), height);
    if let Some(ssid) = subtitle {
        page = page.subtitle(ssid);
    }
    if asking {
        return page;
    }
    settings_link(page, "Network Settings…", NETWORK_SETTINGS)
}

/// The Wi-Fi page was left: the next one scans straight away.
pub(crate) fn forget_scan() {
    LAST_SCAN.set(None);
}

pub(crate) fn vpn_cell() -> Option<GridCell> {
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

    let subtitle = match active.as_slice() {
        [] => String::from("Off"),
        [one] => one.name.clone(),
        many => format!("{} Connected", many.len()),
    };
    let tile = QuickToggle::new("amane.qs.vpn", ICON_VPN, "VPN")
        .subtitle(subtitle)
        .checked(!active.is_empty())
        .on_toggle(move |on| {
            if on {
                if let Some(vpn) = &preferred {
                    Connections::set_vpn(&vpn.uuid, true);
                }
            } else {
                for vpn in &active {
                    Connections::set_vpn(&vpn.uuid, false);
                }
            }
        })
        .on_open(|| open(Page::Vpn))
        .build();
    Some(Cell::Half(boxed(tile)))
}

pub(crate) fn vpn_page() -> DetailPage {
    let vpns = Connections::read().vpns().to_vec();
    let any = vpns.iter().any(|vpn| vpn.active);

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
    let rows = items.len() as f32;
    let height = (rows * sys::size::row::height() + (rows - 1.0).max(0.0) * sys::space::gap::xs())
        .min(sys::size::list::max_height());

    let body: Vec<Box<dyn Widget>> = if items.is_empty() {
        vec![boxed(caption("No VPNs"))]
    } else {
        vec![boxed(List::new(items).build())]
    };

    let page = DetailPage::new("amane.qs.vpn.page", ICON_VPN, "VPN")
        .subtitle(on_off(any))
        .body(body, height.max(sys::size::row::height()));
    settings_link(page, "VPN Settings…", NETWORK_SETTINGS)
}
