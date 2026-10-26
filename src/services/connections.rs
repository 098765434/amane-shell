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

use super::{run_now, state_file};

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
    let profiles = parse_profiles(&profiles);

    let vpns = vpns(&profiles);
    let wired = wired(&parse_rows(&devices));

    // the active Wi-Fi profile in access-point mode is a hotspot
    let mut hotspot = None;
    for profile in profiles
        .iter()
        .filter(|p| p.kind == "802-11-wireless" && p.active)
    {
        let mode = modes.entry(profile.uuid.clone()).or_insert_with(|| {
            nmcli(&[
                "-g",
                "802-11-wireless.mode",
                "connection",
                "show",
                &profile.uuid,
            ])
            .unwrap_or_default()
            .trim()
            .to_string()
        });
        if mode == "ap" {
            hotspot = Some(Hotspot {
                uuid: profile.uuid.clone(),
                ssid: profile.name.clone(),
            });
        }
    }

    Some(Connections {
        vpns,
        wired,
        hotspot,
    })
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
    super::save_state("vpn", &format!("{uuid}\n"));
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
}
