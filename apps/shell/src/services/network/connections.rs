//! Connections — what amane's `Network` doesn't cover: the wired link, VPNs
//! and a Wi-Fi hotspot, as NetworkManager has them.
//!
//! Read with `nmcli` every few seconds (its terse output, `-t`), and changed
//! with it too, which leaves secrets and VPN plugins to NetworkManager. A
//! VPN is any `vpn` or `wireguard` connection profile, like GNOME's; a
//! tunnel NetworkManager doesn't manage (tailscale) isn't one. Without
//! `nmcli` there's nothing, and the tiles stay hidden.

use std::collections::HashMap;
use std::time::Duration;

use amane::Service;

use crate::services::{run_now, state_file};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vpn {
    pub uuid: String,
    pub name: String,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wired {
    pub device: String,
    pub connected: bool,
    /// The connection on it, or the device's name.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hotspot {
    pub uuid: String,
    pub ssid: String,
}

#[derive(Default, PartialEq)]
pub struct Connections {
    vpns: Vec<Vpn>,
    wired: Option<Wired>,
    hotspot: Option<Hotspot>,
}

impl Service for Connections {
    fn new() -> Self {
        fetch(&mut HashMap::new()).unwrap_or_default()
    }

    fn interval() -> Duration {
        Duration::from_secs(3)
    }

    /*
     * nmcli is asked with no lock held, so the bar never waits on it while
     * it draws, and nothing is written when nothing changed
     */
    fn listen() {
        // each Wi-Fi profile's mode, asked once: it only changes when edited
        let mut modes = HashMap::new();
        loop {
            std::thread::sleep(Self::interval());
            let Some(fresh) = fetch(&mut modes) else {
                continue;
            };
            if *Self::read() == fresh {
                continue;
            }
            *Self::write() = fresh;
        }
    }
}

/// What NetworkManager says now; none if nmcli didn't answer.
fn fetch(modes: &mut HashMap<String, String>) -> Option<Connections> {
    let profiles = nmcli(&["-t", "-f", "NAME,UUID,TYPE,DEVICE,ACTIVE", "connection"])?;
    let devices =
        nmcli(&["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"]).unwrap_or_default();
    // a profile's mode doesn't change while it's up, so it's asked once
    Some(assemble(&profiles, &devices, |uuid| {
        modes
            .entry(uuid.to_string())
            .or_insert_with(|| {
                nmcli(&["-g", "802-11-wireless.mode", "connection", "show", uuid])
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            })
            .clone()
    }))
}

/// The model from `nmcli -t` connection and device lists, with `mode_of`
/// giving an active Wi-Fi profile's `802-11-wireless.mode` by UUID.
fn assemble(profiles: &str, devices: &str, mut mode_of: impl FnMut(&str) -> String) -> Connections {
    let profiles = parse_profiles(profiles);

    // the active Wi-Fi profile in access-point mode is a hotspot
    let hotspot = profiles
        .iter()
        .filter(|p| p.kind == "802-11-wireless" && p.active)
        .filter(|p| mode_of(&p.uuid) == "ap")
        .last()
        .map(|p| Hotspot {
            uuid: p.uuid.clone(),
            ssid: p.name.clone(),
        });

    Connections {
        vpns: vpns(&profiles),
        wired: wired(&parse_rows(devices)),
        hotspot,
    }
}

impl Connections {
    pub fn vpns(&self) -> &[Vpn] {
        &self.vpns
    }

    pub fn wired(&self) -> Option<&Wired> {
        self.wired.as_ref()
    }

    pub fn hotspot(&self) -> Option<&Hotspot> {
        self.hotspot.as_ref()
    }

    pub fn set_vpn(uuid: &str, on: bool) {
        if on {
            remember_vpn(uuid);
        }
        act(&["connection", if on { "up" } else { "down" }, "uuid", uuid]);
    }

    /// The VPN the toggle turns on: the last one used, else the first.
    pub fn preferred_vpn(&self) -> Option<&Vpn> {
        let last = std::fs::read_to_string(state_file("vpn")).unwrap_or_default();
        self.vpns
            .iter()
            .find(|vpn| vpn.uuid == last.trim())
            .or(self.vpns.first())
    }

    pub fn stop_hotspot(uuid: &str) {
        act(&["connection", "down", "uuid", uuid]);
    }

    pub fn set_wired(device: &str, on: bool) {
        act(&["device", if on { "connect" } else { "disconnect" }, device]);
    }
}

fn remember_vpn(uuid: &str) {
    crate::services::save_state("vpn", &format!("{uuid}\n"));
}

/// Runs an `nmcli` change off the UI thread, then reads again.
fn act(args: &[&str]) {
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    std::thread::spawn(move || {
        let _ = run_now("nmcli", &args);
        if let Some(fresh) = fetch(&mut HashMap::new())
            && *Connections::read() != fresh
        {
            *Connections::write() = fresh;
        }
    });
}

fn nmcli(args: &[&str]) -> Option<String> {
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    run_now("nmcli", &args)
}

#[derive(Debug, PartialEq)]
struct Profile {
    name: String,
    uuid: String,
    kind: String,
    active: bool,
}

fn parse_profiles(text: &str) -> Vec<Profile> {
    parse_rows(text)
        .into_iter()
        .filter_map(|row| match row.as_slice() {
            [name, uuid, kind, _device, active] => Some(Profile {
                name: name.clone(),
                uuid: uuid.clone(),
                kind: kind.clone(),
                active: active == "yes",
            }),
            _ => None,
        })
        .collect()
}

fn vpns(profiles: &[Profile]) -> Vec<Vpn> {
    profiles
        .iter()
        .filter(|p| p.kind == "vpn" || p.kind == "wireguard")
        .map(|p| Vpn {
            uuid: p.uuid.clone(),
            name: p.name.clone(),
            active: p.active,
        })
        .collect()
}

/// The first Ethernet device with a cable in: GNOME hides one without.
fn wired(devices: &[Vec<String>]) -> Option<Wired> {
    devices.iter().find_map(|row| match row.as_slice() {
        [device, kind, state, connection]
            if kind == "ethernet" && state != "unavailable" && state != "unmanaged" =>
        {
            Some(Wired {
                device: device.clone(),
                connected: state == "connected",
                name: if connection.is_empty() {
                    device.clone()
                } else {
                    connection.clone()
                },
            })
        }
        _ => None,
    })
}

/// `nmcli -t` lines: fields split at `:`, with `\:` and `\\` escaped.
fn parse_rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut fields = vec![String::new()];
            let mut chars = line.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => {
                        if let Some(next) = chars.next() {
                            fields.last_mut().expect("never empty").push(next);
                        }
                    }
                    ':' => fields.push(String::new()),
                    c => fields.last_mut().expect("never empty").push(c),
                }
            }
            fields
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terse_lines_unescape_colons() {
        assert_eq!(
            parse_rows("a\\:b:c\\\\d:\n\nx:y\n"),
            [
                vec!["a:b".to_string(), "c\\d".into(), "".into()],
                vec!["x".into(), "y".into()]
            ]
        );
    }

    #[test]
    fn only_vpn_and_wireguard_profiles_are_vpns() {
        let profiles = parse_profiles(
            "Home:1111:802-11-wireless:wlan0:yes\n\
             tailscale0:2222:tun:tailscale0:yes\n\
             Work:3333:vpn::no\n\
             wg0:4444:wireguard:wg0:yes\n",
        );
        assert_eq!(
            vpns(&profiles),
            [
                Vpn {
                    uuid: "3333".into(),
                    name: "Work".into(),
                    active: false
                },
                Vpn {
                    uuid: "4444".into(),
                    name: "wg0".into(),
                    active: true
                },
            ]
        );
    }

    #[test]
    fn a_wired_device_shows_only_with_a_cable_in() {
        let rows = parse_rows(
            "wlp1s0:wifi:connected:Home\n\
             enp2s0:ethernet:unavailable:\n",
        );
        assert_eq!(wired(&rows), None);

        let rows = parse_rows("enp2s0:ethernet:disconnected:\n");
        assert_eq!(
            wired(&rows),
            Some(Wired {
                device: "enp2s0".into(),
                connected: false,
                name: "enp2s0".into()
            })
        );

        let rows = parse_rows("enp2s0:ethernet:connected:Wired connection 1\n");
        assert_eq!(
            wired(&rows).map(|w| (w.connected, w.name)),
            Some((true, "Wired connection 1".into()))
        );
    }

    const CONNECTION: &str = include_str!("../../../tests/fixtures/nmcli/connection.txt");
    const DEVICE: &str = include_str!("../../../tests/fixtures/nmcli/device.txt");
    const DEVICE_UNPLUGGED: &str =
        include_str!("../../../tests/fixtures/nmcli/device-unplugged.txt");

    fn vpn(uuid: &str, name: &str, active: bool) -> Vpn {
        Vpn {
            uuid: uuid.into(),
            name: name.into(),
            active,
        }
    }

    #[test]
    fn recorded_lists_give_the_vpns_and_the_wired_link() {
        let c = assemble(CONNECTION, DEVICE, |_| String::from("infrastructure"));
        assert_eq!(
            c.vpns,
            [
                vpn("55555555-5555-4555-8555-555555555555", "Work VPN", false),
                vpn("66666666-6666-4666-8666-666666666666", "wg-home", true),
            ]
        );
        assert_eq!(
            c.wired,
            Some(Wired {
                device: "enp2s0".into(),
                connected: true,
                name: "Wired connection 1".into(),
            })
        );
        assert_eq!(c.hotspot, None);
    }

    #[test]
    fn an_unplugged_cable_hides_the_wired_link() {
        let c = assemble(CONNECTION, DEVICE_UNPLUGGED, |_| String::new());
        assert_eq!(c.wired, None);
    }

    #[test]
    fn an_active_wifi_profile_in_ap_mode_is_the_hotspot() {
        let mut asked = Vec::new();
        let c = assemble(CONNECTION, DEVICE, |uuid| {
            asked.push(uuid.to_string());
            String::from("ap")
        });
        // only the active Wi-Fi profile is asked about, not "Office: 5 GHz"
        assert_eq!(asked, ["11111111-1111-4111-8111-111111111111"]);
        assert_eq!(
            c.hotspot,
            Some(Hotspot {
                uuid: "11111111-1111-4111-8111-111111111111".into(),
                ssid: "Home Wi-Fi".into(),
            })
        );
    }

    #[test]
    fn an_escaped_colon_stays_in_the_name() {
        let names: Vec<String> = parse_profiles(CONNECTION)
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert!(names.contains(&String::from("Office: 5 GHz")), "{names:?}");
    }
}
