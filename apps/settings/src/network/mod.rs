//! Network: Wi‑Fi, Ethernet, VPNs and the hotspot, and a page for each
//! connection with its addresses and IPv4 settings.
//!
//! What's shown comes from amane's `Network` (NetworkManager over D-Bus),
//! which also joins, forgets, and brings profiles up and down. Changes to a
//! saved profile, VPN imports and the hotspot go through `nmcli` (see
//! [`nmcli`]), which keeps a profile's stored secrets and has the VPN
//! plugins' importers.

mod ipv4;
mod nmcli;

use std::thread;
use std::time::Duration;

use amane::{
    Center, Column, ConnectionDetails, FileDialog, Network, NetworkProfile, Rfkill, Row, Service,
    TextInput, Widget,
};
use emaki_kit::app::{Dialog, field_box, text_field};
use emaki_kit::buttons::{IconButton, PushButton, Style, Switch};
use emaki_kit::color::Accent;
use emaki_kit::dropdown::Dropdown;
use emaki_kit::form::FormRow;
use emaki_kit::icons::{
    ICON_CABLE, ICON_DNS, ICON_HOTSPOT, ICON_LOCK, ICON_VPN, ICON_WIFI, ICON_WIFI_OFF, wifi_glyph,
};
use emaki_kit::layout::boxed;
use emaki_kit::process::installed;

use crate::{View, group, groups, problem_row, switch_row};
use ipv4::{Form, Method};

const FIELD: f32 = 240.0;

// the fields, kept by amane under their ids
const ADDRESS: &str = "settings.network.ipv4.address";
const PREFIX: &str = "settings.network.ipv4.prefix";
const GATEWAY: &str = "settings.network.ipv4.gateway";
const DNS: &str = "settings.network.ipv4.dns";
const HIDDEN_NAME: &str = "settings.network.hidden.name";
const HIDDEN_PASSWORD: &str = "settings.network.hidden.password";
const HOTSPOT_NAME: &str = "settings.network.hotspot.name";
const HOTSPOT_PASSWORD: &str = "settings.network.hotspot.password";

const EDITOR: &str = "nm-connection-editor";

/// The page's own state: the connection its sub-page shows, that one's
/// live details, the IPv4 form's choices, and the last `nmcli` job.
#[derive(Default)]
pub struct Net {
    /// The connection page's profile, by UUID.
    selected: String,
    details: Option<ConnectionDetails>,
    /// The IPv4 menu's and the DNS switch's choices; the fields keep their
    /// own text.
    method: Option<Method>,
    auto_dns: bool,
    /// What `nmcli` is doing now, in a few words.
    busy: Option<&'static str>,
    /// How the last job went: what to say, or what went wrong.
    outcome: Option<Result<String, String>>,
    /// Whether the hotspot's saved name and password were asked for.
    hotspot_asked: bool,
    reveal: bool,
    /// Text for fields, found on a thread: the fields live on the window's
    /// thread, so the page puts it in when it next draws.
    pending: Vec<(&'static str, String)>,
}

// how often the open connection page reads its details again
const DETAILS_EVERY: Duration = Duration::from_secs(3);

impl Service for Net {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {
        loop {
            thread::sleep(DETAILS_EVERY);
            if !Self::watched_within(DETAILS_EVERY * 2) {
                continue;
            }
            let uuid = Self::peek().selected.clone();
            if uuid.is_empty() {
                continue;
            }
            let details = Network::details(&uuid);
            let mut net = Self::write();
            if net.selected == uuid && net.details != details {
                net.details = details;
            } else {
                net.quiet();
            }
        }
    }
}

/// Runs `nmcli` once for each of `steps` on a thread, stopping at the first
/// that fails; `done` then says what to show, from the last answer.
fn job(
    what: &'static str,
    steps: Vec<Vec<String>>,
    done: impl FnOnce(String) -> String + Send + 'static,
) {
    {
        let mut net = Net::write();
        net.busy = Some(what);
        net.outcome = None;
    }
    thread::spawn(move || {
        let mut last = Ok(String::new());
        for step in &steps {
            last = nmcli::run(step);
            if last.is_err() {
                break;
            }
        }
        let outcome = last.map(done).map_err(|error| nmcli::said(&error));
        let mut net = Net::write();
        net.busy = None;
        net.outcome = Some(outcome);
    });
}

fn say(text: impl Into<String>) {
    Net::write().outcome = Some(Err(text.into()));
}

/// Shows the connection page for `uuid`, its form filled from what's saved.
fn select(uuid: &str) {
    let profile = Network::read()
        .profiles()
        .iter()
        .find(|profile| profile.uuid == uuid)
        .cloned();
    let Some(profile) = profile else {
        return;
    };
    fill(&profile);
    {
        let mut net = Net::write();
        net.selected = uuid.to_string();
        net.details = None;
        net.outcome = None;
    }
    read_details(uuid.to_string());
    crate::open(View::Sub("connection"));
}

/// Reads the connection's details on a thread, for its page.
fn read_details(uuid: String) {
    thread::spawn(move || {
        let details = Network::details(&uuid);
        let mut net = Net::write();
        if net.selected == uuid {
            net.details = details;
        }
    });
}

/// Puts the profile's saved IPv4 settings in the form.
fn fill(profile: &NetworkProfile) {
    let form = Form::from_settings(&profile.ipv4);
    TextInput::set_text(ADDRESS, &form.address);
    TextInput::set_text(PREFIX, &form.prefix);
    TextInput::set_text(GATEWAY, &form.gateway);
    TextInput::set_text(DNS, &form.dns);
    let mut net = Net::write();
    net.method = Some(form.method);
    net.auto_dns = form.auto_dns;
}

/// The job's state, at the top of a page: working, done, or what went wrong.
fn status_rows(net: &Net) -> Vec<FormRow> {
    let mut rows = Vec::new();
    if let Some(what) = net.busy {
        rows.push(FormRow::new("settings.network.busy", what).loading());
    }
    match &net.outcome {
        Some(Err(error)) => rows.push(problem_row(
            "settings.network.problem",
            "That didn't work",
            error,
            "",
        )),
        Some(Ok(said)) if !said.is_empty() => {
            rows.push(FormRow::new("settings.network.done", said.as_str()));
        }
        _ => {}
    }
    rows
}

fn kind_label(profile: &NetworkProfile) -> &'static str {
    match profile.kind.as_str() {
        "802-11-wireless" if profile.wifi_mode.as_deref() == Some("ap") => "Hotspot",
        "802-11-wireless" => "Wi‑Fi",
        "802-3-ethernet" => "Ethernet",
        "wireguard" => "WireGuard",
        "vpn" => "VPN",
        _ => "Connection",
    }
}

pub fn page() -> Column {
    let net = Network::read();
    let rfkill = Rfkill::read();
    let has_wifi = rfkill.has("wlan");
    let held = rfkill.hard_blocked("wlan");
    drop(rfkill);
    let wifi_on = net.wifi_enabled();
    let joined =
        (net.link() == amane::Link::Wifi).then(|| (net.ssid().to_string(), net.strength()));
    let active = |kind: &str| {
        net.profiles()
            .iter()
            .find(|p| p.active && p.kind == kind && p.wifi_mode.as_deref() != Some("ap"))
            .map(|p| p.uuid.clone())
    };
    let wifi_uuid = active("802-11-wireless");
    let wired_uuid = active("802-3-ethernet");
    let saved = net
        .profiles()
        .iter()
        .filter(|p| p.kind == "802-11-wireless" && p.wifi_mode.as_deref() != Some("ap"))
        .count();
    let wired = net.wired().cloned();
    let vpns: Vec<NetworkProfile> = net.vpns().cloned().collect();
    let hotspot = net.hotspot().map(|p| p.uuid.clone());
    drop(net);

    load_hotspot();
    put_pending();
    let state = Net::read();
    let status = status_rows(&state);
    let reveal = state.reveal;
    drop(state);

    let mut all = vec![group().rows(status)];

    if has_wifi {
        let mut wifi = group().header("Wi‑Fi").row(
            switch_row(
                "settings.network.wifi",
                "Wi‑Fi",
                wifi_on && !held,
                Network::set_wifi,
            )
            .icon(
                if wifi_on { ICON_WIFI } else { ICON_WIFI_OFF },
                Accent::Blue,
            )
            .subtitle(if held {
                "Hardware switch off"
            } else if wifi_on {
                "On"
            } else {
                "Off"
            })
            .disabled(held),
        );
        if let Some((ssid, strength)) = joined {
            let mut row = FormRow::new("settings.network.wifi.current", ssid)
                .icon(wifi_glyph(strength), Accent::Blue)
                .subtitle("Connected");
            if let Some(uuid) = wifi_uuid {
                row = row.on_press(move || select(&uuid));
            }
            wifi = wifi.row(row);
        }
        wifi = wifi
            .row(
                FormRow::new("settings.network.saved", "Saved Networks")
                    .value(saved.to_string())
                    .on_press(|| crate::open(View::Sub("saved"))),
            )
            .row(
                FormRow::new("settings.network.hidden", "Join a Hidden Network")
                    .on_press(|| crate::open(View::Sub("hidden"))),
            );
        all.push(wifi);
    }

    if let Some(wired) = wired {
        let device = wired.device.clone();
        let mut ethernet = group().header("Ethernet").row(
            switch_row(
                "settings.network.wired",
                "Ethernet",
                wired.connected,
                move |on| {
                    Network::set_device(&device, on);
                },
            )
            .icon(ICON_CABLE, Accent::Gray)
            .subtitle(if wired.connected {
                wired.name.clone()
            } else {
                String::from("Not connected")
            }),
        );
        if let Some(uuid) = wired_uuid {
            ethernet = ethernet.row(
                FormRow::new("settings.network.wired.details", "Details")
                    .on_press(move || select(&uuid)),
            );
        }
        all.push(ethernet);
    }

    all.push(vpn_group(&vpns));
    if has_wifi {
        all.push(hotspot_group(hotspot, reveal));
    }
    if installed(EDITOR) {
        all.push(
            group().row(
                FormRow::new("settings.network.editor", "More Options")
                    .subtitle("802.1X, proxies, IPv6 and VPN settings, in NetworkManager's editor")
                    .trailing(
                        PushButton::new("settings.network.editor.open", "Open…")
                            .on_press(|| emaki_kit::process::run(EDITOR, Vec::new()))
                            .build(),
                    ),
            ),
        );
    }
    groups(all)
}

fn vpn_group(vpns: &[NetworkProfile]) -> emaki_kit::form::Group {
    let mut vpn = group().header("VPN");
    for profile in vpns {
        let uuid = profile.uuid.clone();
        let toggled = uuid.clone();
        let on = profile.active;
        let switch = Switch::new(format!("settings.network.vpn.{uuid}.switch"), on)
            .on_toggle(move |on| {
                if on {
                    Network::activate(&toggled);
                } else {
                    Network::deactivate(&toggled);
                }
            })
            .build();
        let failed = Network::read()
            .failure(&uuid)
            .map(|failure| failure.to_string());
        let subtitle = match failed {
            Some(failure) => failure,
            None if on => format!("{} · Connected", kind_label(profile)),
            None => kind_label(profile).to_string(),
        };
        vpn = vpn.row(
            FormRow::new(
                format!("settings.network.vpn.{uuid}"),
                profile.name.as_str(),
            )
            .icon(ICON_VPN, Accent::Indigo)
            .subtitle(subtitle)
            .trailing(switch)
            .trailing(
                IconButton::new(
                    format!("settings.network.vpn.{uuid}.details"),
                    emaki_kit::icons::ICON_CHEVRON_RIGHT,
                )
                .on_press(move || select(&uuid))
                .build(),
            ),
        );
    }
    let import = PushButton::new("settings.network.vpn.import.choose", "Import from File…")
        .on_press(import)
        .build();
    vpn = vpn.row(
        FormRow::new("settings.network.vpn.import", "Add a VPN")
            .subtitle("A WireGuard .conf or an OpenVPN .ovpn file")
            .trailing(import),
    );
    if nmcli::openvpn_available() {
        vpn
    } else {
        vpn.footer(nmcli::OPENVPN_HINT)
    }
}

fn import() {
    FileDialog::open("Import a VPN")
        .filter("VPN files", &["*.conf", "*.ovpn"])
        .show(|picked| {
            let Some(file) = picked.into_iter().next() else {
                return;
            };
            let Some(kind) = nmcli::import_kind(&file) else {
                say("Only WireGuard .conf and OpenVPN .ovpn files can be imported.");
                return;
            };
            if kind == "openvpn" && !nmcli::openvpn_available() {
                say(nmcli::OPENVPN_HINT);
                return;
            }
            let name = file
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            job(
                "Importing…",
                vec![nmcli::import_args(kind, &file)],
                move |_| format!("Imported “{name}”."),
            );
        });
}

/// The hotspot's saved name and password, asked for once when the page
/// first shows.
fn load_hotspot() {
    if Net::read().hotspot_asked {
        return;
    }
    Net::write().hotspot_asked = true;
    thread::spawn(|| {
        let (ssid, password) = nmcli::run(&nmcli::hotspot_show_args())
            .map(|out| nmcli::hotspot_values(&out))
            .unwrap_or_default();
        let ssid = if ssid.is_empty() { hostname() } else { ssid };
        Net::write()
            .pending
            .extend([(HOTSPOT_NAME, ssid), (HOTSPOT_PASSWORD, password)]);
    });
}

/// This machine's name, which a new hotspot is called by.
fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_string())
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| String::from("Emaki"))
}

/// Puts text found on a thread into its fields, on the window's thread.
fn put_pending() {
    if Net::peek().pending.is_empty() {
        return;
    }
    for (id, text) in std::mem::take(&mut Net::write().pending) {
        TextInput::set_text(id, &text);
    }
}

fn hotspot_group(running: Option<String>, reveal: bool) -> emaki_kit::form::Group {
    let on = running.is_some();
    let mut password = text_field(HOTSPOT_PASSWORD, "8 characters or more");
    if !reveal {
        password = password.password();
    }
    let reveal_button = IconButton::new("settings.network.hotspot.reveal", ICON_LOCK)
        .selected(reveal)
        .on_press(|| {
            let mut net = Net::write();
            net.reveal = !net.reveal;
        })
        .build();
    group()
        .header("Hotspot")
        .row(
            switch_row(
                "settings.network.hotspot",
                "Share this connection",
                on,
                move |share| {
                    if share {
                        start_hotspot();
                    } else if let Some(uuid) = &running {
                        Network::deactivate(uuid);
                    }
                },
            )
            .icon(ICON_HOTSPOT, Accent::Green),
        )
        .row(
            FormRow::new("settings.network.hotspot.name.row", "Network name")
                .trailing(field_box(
                    HOTSPOT_NAME,
                    text_field(HOTSPOT_NAME, "Name"),
                    FIELD,
                ))
                .disabled(on),
        )
        .row(
            FormRow::new("settings.network.hotspot.password.row", "Password")
                .trailing(field_box(HOTSPOT_PASSWORD, password, FIELD))
                .trailing(reveal_button)
                .disabled(on),
        )
        .footer(
            "Other devices join it to use this machine's connection. While it runs, this \
             machine's Wi‑Fi can't join other networks. An empty password gets one made.",
        )
}

fn start_hotspot() {
    let ssid = TextInput::text(HOTSPOT_NAME).trim().to_string();
    let password = TextInput::text(HOTSPOT_PASSWORD);
    if ssid.is_empty() {
        say("The hotspot needs a name.");
        return;
    }
    if !password.is_empty() && password.chars().count() < 8 {
        say("A hotspot's password needs 8 characters or more.");
        return;
    }
    job(
        "Starting the hotspot…",
        vec![nmcli::hotspot_args(&ssid, &password)],
        |_| {
            // show the password nmcli made, if it made one
            let (_, password) = nmcli::run(&nmcli::hotspot_show_args())
                .map(|out| nmcli::hotspot_values(&out))
                .unwrap_or_default();
            Net::write().pending.push((HOTSPOT_PASSWORD, password));
            String::new()
        },
    );
}

/// A sub-page: `saved`, `hidden` or `connection`.
pub fn sub(name: &str) -> Option<(String, Box<dyn Widget>)> {
    match name {
        "saved" => Some((String::from("Saved Networks"), boxed(saved()))),
        "hidden" => Some((String::from("Join a Hidden Network"), boxed(hidden()))),
        "connection" => {
            let (title, body) = connection();
            Some((title, boxed(body)))
        }
        _ => None,
    }
}

fn saved() -> Column {
    let mut profiles: Vec<NetworkProfile> = Network::read()
        .profiles()
        .iter()
        .filter(|p| p.kind == "802-11-wireless" && p.wifi_mode.as_deref() != Some("ap"))
        .cloned()
        .collect();
    profiles.sort_by(|a, b| b.last_used.cmp(&a.last_used).then(a.name.cmp(&b.name)));
    let rows: Vec<FormRow> = profiles
        .iter()
        .map(|profile| {
            let uuid = profile.uuid.clone();
            let mut subtitle = if profile.active {
                String::from("Connected")
            } else {
                last_used(profile.last_used)
            };
            if profile.declared {
                subtitle.push_str(" · Set in NixOS");
            }
            let name = profile.ssid.clone().unwrap_or_else(|| profile.name.clone());
            FormRow::new(format!("settings.network.saved.{uuid}"), name)
                .icon(ICON_WIFI, Accent::Blue)
                .subtitle(subtitle)
                .on_press(move || select(&uuid))
        })
        .collect();
    let list = group();
    let list = if rows.is_empty() {
        list.footer("Networks show here once joined.")
    } else {
        list.rows(rows)
            .footer("Open one to see its addresses, change its settings or forget it.")
    };
    groups(vec![list])
}

fn last_used(at: u64) -> String {
    if at == 0 {
        return String::from("Never used");
    }
    let elapsed = emaki_kit::time::since_unix_s(at);
    format!("Last used {}", emaki_kit::format::ago(elapsed))
}

fn hidden() -> Column {
    let ssid = TextInput::text(HIDDEN_NAME).trim().to_string();
    let net = Network::read();
    let state = if ssid.is_empty() {
        None
    } else if net.link() == amane::Link::Wifi && net.ssid() == ssid {
        Some(String::from("Connected"))
    } else if net.operation(&ssid).is_some() {
        Some(String::from("Connecting…"))
    } else {
        net.failure(&ssid).map(|failure| match failure.kind {
            amane::ErrorKind::AuthenticationFailed => String::from("Wrong password"),
            amane::ErrorKind::Unreachable | amane::ErrorKind::Gone => String::from("Not in range"),
            amane::ErrorKind::TimedOut => String::from("Timed out"),
            _ => String::from("Couldn't join"),
        })
    };
    drop(net);
    let join = PushButton::new("settings.network.hidden.join", "Join")
        .style(Style::Filled)
        .on_press(|| {
            let ssid = TextInput::text(HIDDEN_NAME).trim().to_string();
            if ssid.is_empty() {
                return;
            }
            // taken out of the field, and wiped once handed on
            let password = TextInput::take_secret(HIDDEN_PASSWORD);
            let password = (!password.is_empty()).then_some(password.as_str());
            Network::connect_hidden(&ssid, password);
        })
        .build();
    let mut form = group()
        .row(
            FormRow::new("settings.network.hidden.name.row", "Network name").trailing(field_box(
                HIDDEN_NAME,
                text_field(HIDDEN_NAME, "Its exact name"),
                FIELD,
            )),
        )
        .row(
            FormRow::new("settings.network.hidden.password.row", "Password").trailing(field_box(
                HIDDEN_PASSWORD,
                text_field(HIDDEN_PASSWORD, "Empty for an open network").password(),
                FIELD,
            )),
        );
    form = form.row(match state {
        Some(state) => FormRow::new("settings.network.hidden.join.row", state).trailing(join),
        None => FormRow::new("settings.network.hidden.join.row", "").trailing(join),
    });
    groups(vec![form.footer(
        "A hidden network doesn't announce its name, so it's joined by typing it. WPA2 and \
         WPA3 personal networks take a password here.",
    )])
}

/// With nothing chosen (a search led here), the connection carrying the
/// traffic: the Wi‑Fi or Ethernet one up now.
fn choose_current() {
    if !Net::peek().selected.is_empty() {
        return;
    }
    let current = Network::read()
        .profiles()
        .iter()
        .find(|p| {
            p.active
                && matches!(p.kind.as_str(), "802-11-wireless" | "802-3-ethernet")
                && p.wifi_mode.as_deref() != Some("ap")
        })
        .cloned();
    if let Some(profile) = current {
        fill(&profile);
        Net::write().selected = profile.uuid.clone();
        read_details(profile.uuid);
    }
}

fn connection() -> (String, Column) {
    choose_current();
    let state = Net::read();
    let uuid = state.selected.clone();
    let details = state.details.clone();
    let method = state.method.unwrap_or(Method::Auto);
    let auto_dns = state.auto_dns;
    let status = status_rows(&state);
    drop(state);
    let net = Network::read();
    let profile = net.profiles().iter().find(|p| p.uuid == uuid).cloned();
    let failure = net.failure(&uuid).map(|f| f.to_string());
    drop(net);
    let Some(profile) = profile else {
        return (
            String::from("Connection"),
            groups(vec![group().footer("This connection is gone.")]),
        );
    };
    let title = profile.ssid.clone().unwrap_or_else(|| profile.name.clone());
    let locked = profile.declared;

    let mut all = vec![group().rows(status)];
    if let Some(failure) = failure {
        all.push(group().row(problem_row(
            "settings.network.connection.failure",
            "That didn't work",
            &failure,
            "",
        )));
    }

    let auto_uuid = profile.uuid.clone();
    let mut about = group()
        .row(
            FormRow::new("settings.network.connection.kind", kind_label(&profile)).value(
                if profile.active {
                    "Connected"
                } else {
                    "Not connected"
                },
            ),
        )
        .row(
            switch_row(
                "settings.network.connection.autoconnect",
                "Connect automatically",
                profile.autoconnect,
                move |on| {
                    job(
                        "Saving…",
                        vec![nmcli::autoconnect_args(&auto_uuid, on)],
                        |_| String::new(),
                    );
                },
            )
            .disabled(locked),
        );
    if locked {
        about = about.footer(
            "Your NixOS configuration sets this connection, so it's changed there: a change \
             here would be undone the next time the system starts.",
        );
    }
    all.push(about);

    if let Some(details) = &details {
        all.push(details_group(details));
    }

    if profile.wifi_mode.as_deref() != Some("ap") {
        all.push(ipv4_group(&profile, method, auto_dns, locked));
    }

    if matches!(
        profile.kind.as_str(),
        "802-11-wireless" | "vpn" | "wireguard"
    ) {
        let name = title.clone();
        let forgotten = profile.uuid.clone();
        let label = if profile.kind == "802-11-wireless" {
            "Forget This Network…"
        } else {
            "Remove This VPN…"
        };
        let forget = PushButton::new("settings.network.connection.forget", label)
            .style(Style::Destructive)
            .disabled(locked)
            .on_press(move || {
                let uuid = forgotten.clone();
                Dialog::new("settings.network.forget", format!("Forget “{name}”?"))
                    .message("Its password and settings go, so joining it again asks for them.")
                    .button("Cancel", || {})
                    .destructive("Forget", move || {
                        Network::forget(&uuid);
                        crate::back();
                    })
                    .open("main");
            })
            .build();
        all.push(
            group()
                .row(FormRow::new("settings.network.connection.forget.row", "").trailing(forget)),
        );
    }
    (title, groups(all))
}

fn details_group(details: &ConnectionDetails) -> emaki_kit::form::Group {
    let mut rows = Vec::new();
    let mut add = |id: &str, title: &str, value: String| {
        if !value.is_empty() {
            rows.push(
                FormRow::new(format!("settings.network.connection.{id}"), title).value(value),
            );
        }
    };
    let addresses = |list: &[(String, u8)]| {
        list.iter()
            .map(|(address, prefix)| format!("{address}/{prefix}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    add("ipv4-address", "IPv4 address", addresses(&details.ipv4));
    add("gateway", "Gateway", details.gateway.clone());
    add("dns-servers", "DNS", details.dns.join(", "));
    add("ipv6-address", "IPv6 address", addresses(&details.ipv6));
    add("interface", "Interface", details.interface.clone());
    add("mac", "Hardware address", details.mac.clone());
    add(
        "speed",
        "Link speed",
        details
            .speed
            .map(|mbps| format!("{mbps} Mb/s"))
            .unwrap_or_default(),
    );
    add(
        "frequency",
        "Frequency",
        details.frequency.map(band).unwrap_or_default(),
    );
    group().header("Details").rows(rows)
}

fn band(mhz: u32) -> String {
    let band = match mhz {
        2400..=2500 => "2.4 GHz",
        4900..=5900 => "5 GHz",
        5925..=7125 => "6 GHz",
        _ => return format!("{mhz} MHz"),
    };
    format!("{band} ({mhz} MHz)")
}

fn ipv4_group(
    profile: &NetworkProfile,
    method: Method,
    auto_dns: bool,
    locked: bool,
) -> emaki_kit::form::Group {
    let menu = Dropdown::choice(
        "settings.network.ipv4.method.menu",
        Method::ALL.map(|m| (m.label(), m)),
        method,
        |picked| Net::write().method = Some(picked),
    )
    .width(FIELD)
    .build();
    let field = |id: &'static str, title: &str, placeholder: &str| {
        FormRow::new(format!("{id}.row"), title)
            .trailing(field_box(id, text_field(id, placeholder), FIELD))
            .disabled(locked)
    };
    let mut ipv4 = group().header("IPv4").row(
        FormRow::new("settings.network.ipv4.method", "Method")
            .trailing(menu)
            .disabled(locked),
    );
    if method == Method::Manual {
        ipv4 = ipv4
            .row(field(ADDRESS, "Address", "192.168.1.20"))
            .row(field(PREFIX, "Prefix", "24"))
            .row(field(GATEWAY, "Gateway", "192.168.1.1"));
    }
    if method == Method::Auto {
        ipv4 = ipv4.row(
            switch_row(
                "settings.network.connection.dns",
                "Automatic DNS",
                auto_dns,
                |on| Net::write().auto_dns = on,
            )
            .subtitle("The DNS servers the network hands out")
            .icon(ICON_DNS, Accent::Gray)
            .disabled(locked),
        );
    }
    if method.has_dns() {
        let hint = if method == Method::Auto && auto_dns {
            "Also use, e.g. 1.1.1.1"
        } else {
            "1.1.1.1, 9.9.9.9"
        };
        ipv4 = ipv4.row(field(DNS, "DNS servers", hint));
    }
    let uuid = profile.uuid.clone();
    let active = profile.active;
    let name = profile.ssid.clone().unwrap_or_else(|| profile.name.clone());
    let apply = PushButton::new("settings.network.ipv4.apply", "Apply")
        .style(Style::Filled)
        .disabled(locked)
        .on_press(move || apply(&uuid, active, &name))
        .build();
    let reverted = profile.clone();
    let revert = PushButton::new("settings.network.ipv4.revert", "Revert")
        .disabled(locked)
        .on_press(move || fill(&reverted))
        .build();
    ipv4.row(
        FormRow::new("settings.network.ipv4.buttons", "").trailing(
            Row::new(vec![boxed(revert), boxed(apply)])
                .gap(8.0)
                .align(Center),
        ),
    )
    .footer(if active {
        "Apply reconnects it with the new settings."
    } else {
        "The new settings take effect the next time it connects."
    })
}

fn apply(uuid: &str, active: bool, name: &str) {
    let net = Net::read();
    let form = Form {
        method: net.method.unwrap_or(Method::Auto),
        address: TextInput::text(ADDRESS),
        prefix: TextInput::text(PREFIX),
        gateway: TextInput::text(GATEWAY),
        auto_dns: net.auto_dns,
        dns: TextInput::text(DNS),
    };
    drop(net);
    if let Err(problem) = form.check() {
        say(problem);
        return;
    }
    let mut steps = vec![form.args(uuid)];
    if !active {
        job("Saving…", steps, |_| String::from("Saved."));
        return;
    }
    steps.push(nmcli::up_args(uuid));
    let name = name.to_string();
    Dialog::new("settings.network.apply", format!("Reconnect “{name}”?"))
        .message("It drops for a moment while it takes the new settings.")
        .button("Cancel", || {})
        .main_button("Reconnect", move || {
            job("Reconnecting…", steps.clone(), |_| {
                String::from("Saved and reconnected.")
            });
        })
        .open("main");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequencies_read_as_bands() {
        assert_eq!(band(2437), "2.4 GHz (2437 MHz)");
        assert_eq!(band(5180), "5 GHz (5180 MHz)");
        assert_eq!(band(6115), "6 GHz (6115 MHz)");
        assert_eq!(band(60480), "60480 MHz");
    }
}
