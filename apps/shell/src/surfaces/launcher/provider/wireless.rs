//! Wi‑Fi and Bluetooth: the networks in range and the devices paired and
//! near, each a list the "Wi‑Fi Networks" and "Bluetooth Devices" actions
//! open ([`Scope::Wifi`], [`Scope::Bluetooth`]), and a network's password
//! typed hidden ([`Scope::WifiPassword`]). From the pill, the networks
//! saved and the devices paired are found by name.
//!
//! Joining or pairing closes the launcher; how it goes shows in the status
//! bar, and a pairing code asks through the shell's alert.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use amane::{AccessPoint, Bluetooth, BluetoothDevice, Network, NetworkOperation, Service};
use emaki_kit::icons::{
    ICON_BLUETOOTH, ICON_BLUETOOTH_OFF, ICON_LOCK, ICON_WIFI, ICON_WIFI_OFF, device_glyph,
    wifi_glyph,
};

use crate::surfaces::quick_settings as qs;

use super::*;

/// How often the list of networks looks again while it shows.
const RESCAN: Duration = Duration::from_secs(15);

/// How many letters from the pill before networks and devices are named.
const FROM_PILL_CHARS: usize = 2;

/// How a network or device found by name from the pill compares with an
/// app named the same.
const NAMED_WEIGHT: f32 = 0.9;

const MY_DEVICES: &str = "My Devices";
const NEARBY: &str = "Nearby";

thread_local! {
    // when the list of networks last asked for a scan
    static LAST_SCAN: Cell<Option<Instant>> = const { Cell::new(None) };
    // whether the list of devices started BlueZ searching
    static SEARCHING: Cell<bool> = const { Cell::new(false) };
}

/// The networks saved and the devices paired, by name, from the pill.
pub(super) struct Wireless;

impl Provider for Wireless {
    fn id(&self) -> &'static str {
        "wireless"
    }

    fn modes(&self) -> &'static [Mode] {
        HOME
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        if query.needle.chars().count() < FROM_PILL_CHARS {
            return;
        }

        let net = Network::read();
        for ap in net.access_points() {
            if !ap.saved() || ap.active() || qs::password_refused(net.failure(ap.ssid())) {
                continue;
            }
            if let Some(matched) = rank::fuzzy(&query.needle, ap.ssid()) {
                let ssid = ap.ssid().to_string();
                out.push(Candidate {
                    entry: Entry::new(
                        format!("wifi:{ssid}"),
                        format!("Connect to {ssid}"),
                        Icon::Glyph(wifi_glyph(ap.strength())),
                        Action::Run(Rc::new(move || Network::connect(&ssid, None))),
                    )
                    .subtitle("Wi‑Fi network"),
                    matched: matched * NAMED_WEIGHT,
                    group: Group::Ranked,
                });
            }
        }
        drop(net);

        let bt = Bluetooth::read();
        if !bt.powered() {
            return;
        }
        for device in bt.devices().iter().filter(|device| device.paired()) {
            if let Some(matched) = rank::fuzzy(&query.needle, device.name()) {
                let title = if device.connected() {
                    format!("Disconnect {}", device.name())
                } else {
                    format!("Connect {}", device.name())
                };
                let picked = device.clone();
                out.push(Candidate {
                    entry: Entry::new(
                        format!("bt:{}", device.address()),
                        title,
                        Icon::Glyph(device_glyph(device.icon())),
                        Action::Run(Rc::new(move || qs::bluetooth_act(&picked))),
                    )
                    .subtitle("Bluetooth device"),
                    matched: matched * NAMED_WEIGHT,
                    group: Group::Ranked,
                });
            }
        }
    }
}

/// Whether `needle` finds `name`: everything while nothing's typed.
fn named(needle: &str, name: &str) -> bool {
    needle.is_empty() || name.to_lowercase().contains(needle)
}

fn listed(entry: Entry, out: &mut Vec<Candidate>) {
    out.push(Candidate {
        entry,
        matched: 1.0,
        group: Group::Listed,
    });
}

/// A row that only says something.
fn note(key: &str, title: &str, glyph: char) -> Entry {
    Entry::new(key, title, Icon::Glyph(glyph), Action::Hint)
}

/// The networks in range, the strongest first, narrowed to what's typed; a
/// scan now and then while they show.
pub(super) fn networks(query: &Query, out: &mut Vec<Candidate>) {
    let net = Network::read();
    if !net.wifi_enabled() {
        drop(net);
        listed(
            Entry::new(
                "wifi:turn-on",
                "Turn Wi‑Fi On",
                Icon::Glyph(ICON_WIFI_OFF),
                Action::Stay(Rc::new(|| Network::set_wifi(true))),
            ),
            out,
        );
        return;
    }

    let due = LAST_SCAN.get().is_none_or(|at| at.elapsed() >= RESCAN);
    if due {
        LAST_SCAN.set(Some(Instant::now()));
        Network::scan();
        emaki_kit::motion::redraw_after::<super::super::Launcher>(RESCAN);
    }

    let mut found = 0;
    for ap in net.access_points() {
        if !named(&query.needle, ap.ssid()) {
            continue;
        }
        found += 1;
        let connecting = net.operation(ap.ssid()) == Some(NetworkOperation::Connecting);
        let failure = qs::recent_failure(net.failure(ap.ssid())).map(qs::wifi_failure);
        let refused = qs::password_refused(net.failure(ap.ssid()));
        listed(network_entry(ap, connecting, failure, refused), out);
    }
    drop(net);

    if found == 0 {
        let title = if query.needle.is_empty() {
            "Searching…"
        } else {
            "No networks found"
        };
        listed(note("wifi:none", title, ICON_WIFI), out);
    }
}

/// A network's row: what picking it does, and where it stands.
fn network_entry(
    ap: &AccessPoint,
    connecting: bool,
    failure: Option<&'static str>,
    refused: bool,
) -> Entry {
    let ssid = ap.ssid().to_string();
    let glyph = Icon::Glyph(wifi_glyph(ap.strength()));
    let key = format!("wifi:{ssid}");

    let status = if connecting && !ap.active() {
        "Connecting…"
    } else if let Some(failure) = failure {
        failure
    } else if ap.active() {
        "Connected · Return disconnects"
    } else if ap.saved() {
        "Saved"
    } else if ap.secured() {
        "Secured"
    } else {
        "Open"
    };

    let entry = if ap.active() {
        Entry::new(key, ssid, glyph, Action::Run(Rc::new(Network::disconnect)))
    } else if qs::needs_password(ap, refused) {
        let scope = Scope::WifiPassword { ssid: ssid.clone() };
        Entry::new(key, ssid, glyph, Action::Ask(scope.clone())).scope(scope)
    } else {
        let joined = ssid.clone();
        Entry::new(
            key,
            ssid,
            glyph,
            Action::Run(Rc::new(move || Network::connect(&joined, None))),
        )
    };
    entry.subtitle(status)
}

/// The one row while a password is typed for `ssid`: Return joins with it.
/// What's typed isn't searched, and the row's key keeps it from being
/// remembered.
pub(super) fn password(ssid: &str, query: &Query, out: &mut Vec<Candidate>) {
    let key = format!("{}wifi", entries::SECRET_KEY);
    if query.text.is_empty() {
        listed(
            note(&key, &format!("Password for {ssid}"), ICON_LOCK).subtitle("Type it, then Return"),
            out,
        );
        return;
    }
    let ssid = ssid.to_string();
    // untrimmed: a password may start or end with a space
    let typed = query.raw.to_string();
    listed(
        Entry::new(
            key,
            format!("Join {ssid}"),
            Icon::Glyph(ICON_LOCK),
            Action::Run(Rc::new(move || Network::connect(&ssid, Some(&typed)))),
        )
        .subtitle("Wi‑Fi network"),
        out,
    );
}

/// The devices paired, then the ones near enough to pair, narrowed to
/// what's typed; BlueZ searches while they show.
pub(super) fn devices(query: &Query, out: &mut Vec<Candidate>) {
    let bt = Bluetooth::read();
    if !bt.powered() {
        drop(bt);
        listed(
            Entry::new(
                "bt:turn-on",
                "Turn Bluetooth On",
                Icon::Glyph(ICON_BLUETOOTH_OFF),
                Action::Stay(Rc::new(|| Bluetooth::set_powered(true))),
            ),
            out,
        );
        return;
    }
    if !SEARCHING.replace(true) {
        Bluetooth::start_scan();
    }

    let (paired, others): (Vec<BluetoothDevice>, Vec<BluetoothDevice>) = bt
        .devices()
        .iter()
        .filter(|device| named(&query.needle, device.name()))
        .cloned()
        .partition(BluetoothDevice::paired);
    let scanning = bt.scanning();
    let rows: Vec<Entry> = paired
        .iter()
        .map(|device| device_entry(&bt, device, MY_DEVICES))
        .chain(
            qs::bluetooth_nearby(others)
                .iter()
                .map(|device| device_entry(&bt, device, NEARBY)),
        )
        .collect();
    drop(bt);

    if rows.is_empty() {
        let title = if scanning && query.needle.is_empty() {
            "Searching…"
        } else {
            "No devices found"
        };
        listed(note("bt:none", title, ICON_BLUETOOTH), out);
    }
    for entry in rows {
        listed(entry, out);
    }
}

/// A device's row, under `section`: what picking it does, and where it
/// stands.
fn device_entry(bt: &Bluetooth, device: &BluetoothDevice, section: &str) -> Entry {
    let operation = bt.operation(device.path());
    let failure = qs::recent_failure(bt.failure(device.path())).map(qs::bluetooth_failure);

    let status = if let Some(operation) = operation {
        qs::bluetooth_operation(operation).to_string()
    } else if let Some(failure) = failure {
        failure.to_string()
    } else if device.connected() {
        let battery = device
            .battery()
            .map(|percent| format!(" · {percent}%"))
            .unwrap_or_default();
        format!("Connected{battery} · Return disconnects")
    } else if device.paired() {
        String::from("Paired")
    } else {
        String::from("Return pairs")
    };

    let picked = device.clone();
    let mut entry = Entry::new(
        format!("bt:{}", device.address()),
        device.name(),
        Icon::Glyph(device_glyph(device.icon())),
        Action::Run(Rc::new(move || qs::bluetooth_act(&picked))),
    )
    .subtitle(status);
    entry.section = Some(section.to_string());
    entry
}

/// The list of devices was left: the search it started ends.
pub(in super::super) fn left_devices() {
    if SEARCHING.replace(false) {
        Bluetooth::stop_scan();
    }
}

/// The list of networks was left: the next one scans straight away.
pub(in super::super) fn left_networks() {
    LAST_SCAN.set(None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_typed_names_everything() {
        assert!(named("", "Home"));
        assert!(named("ho", "Home"));
        assert!(!named("cafe", "Home"));
    }
}
