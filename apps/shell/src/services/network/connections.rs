//! Connections — the wired link, VPNs and a Wi-Fi hotspot, as the bar and Quick Settings show
//! them. amane's `Network` follows NetworkManager's signals and lists the profiles; this keeps
//! the shell's view of them (and the VPN used last, for the toggle) and changes them through
//! amane, which leaves secrets and VPN plugins to NetworkManager. A VPN is any `vpn` or
//! `wireguard` profile, like GNOME's; a tunnel NetworkManager doesn't manage (tailscale) isn't
//! one. Without NetworkManager there's nothing, and the tiles stay hidden.

use amane::{Network, NetworkProfile, Service};

use crate::services::{save_state, state_file};

pub use amane::Wired;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vpn {
    pub uuid: String,
    pub name: String,
    pub active: bool,
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
        from_network(&Network::read())
    }

    fn listen() {
        Network::subscribe(|network| {
            let fresh = from_network(network);
            let mut connections = Connections::write();
            if *connections == fresh {
                connections.quiet();
                return;
            }
            *connections = fresh;
        })
        .forever();
    }
}

fn from_network(network: &Network) -> Connections {
    assemble(network.profiles(), network.wired().cloned())
}

fn assemble(profiles: &[NetworkProfile], wired: Option<Wired>) -> Connections {
    Connections {
        vpns: profiles
            .iter()
            .filter(|profile| profile.is_vpn())
            .map(|profile| Vpn {
                uuid: profile.uuid.clone(),
                name: profile.name.clone(),
                active: profile.active,
            })
            .collect(),
        wired,
        hotspot: profiles
            .iter()
            .rfind(|profile| profile.is_hotspot())
            .map(|profile| Hotspot {
                uuid: profile.uuid.clone(),
                ssid: profile.name.clone(),
            }),
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
            save_state("vpn", &format!("{uuid}\n"));
            Network::activate(uuid);
        } else {
            Network::deactivate(uuid);
        }
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
        Network::deactivate(uuid);
    }

    pub fn set_wired(device: &str, on: bool) {
        Network::set_device(device, on);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(
        name: &str,
        uuid: &str,
        kind: &str,
        mode: Option<&str>,
        active: bool,
    ) -> NetworkProfile {
        NetworkProfile {
            uuid: uuid.into(),
            name: name.into(),
            kind: kind.into(),
            wifi_mode: mode.map(String::from),
            active,
        }
    }

    // what tests/fixtures/nmcli/connection.txt listed, as amane hands it over
    fn recorded(home_mode: &str) -> Vec<NetworkProfile> {
        vec![
            profile(
                "Home Wi-Fi",
                "1111",
                "802-11-wireless",
                Some(home_mode),
                true,
            ),
            profile("tailscale0", "2222", "tun", None, true),
            profile("lo", "3333", "loopback", None, true),
            profile(
                "Office: 5 GHz",
                "4444",
                "802-11-wireless",
                Some("ap"),
                false,
            ),
            profile("Work VPN", "5555", "vpn", None, false),
            profile("wg-home", "6666", "wireguard", None, true),
            profile("Wired connection 1", "7777", "802-3-ethernet", None, true),
        ]
    }

    #[test]
    fn the_recorded_profiles_give_the_vpns_and_no_hotspot() {
        let wired = Wired {
            device: "enp2s0".into(),
            connected: true,
            name: "Wired connection 1".into(),
        };
        let c = assemble(&recorded("infrastructure"), Some(wired.clone()));
        let vpns: Vec<(&str, bool)> = c.vpns.iter().map(|v| (v.name.as_str(), v.active)).collect();
        assert_eq!(vpns, [("Work VPN", false), ("wg-home", true)]);
        assert_eq!(c.wired, Some(wired));
        // an access-point profile that isn't up isn't a hotspot
        assert_eq!(c.hotspot, None);
    }

    #[test]
    fn an_active_wifi_profile_in_ap_mode_is_the_hotspot() {
        let c = assemble(&recorded("ap"), None);
        assert_eq!(
            c.hotspot,
            Some(Hotspot {
                uuid: "1111".into(),
                ssid: "Home Wi-Fi".into(),
            })
        );
        assert_eq!(c.wired, None);
    }
}
