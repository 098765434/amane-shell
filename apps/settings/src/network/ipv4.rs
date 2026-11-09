//! The IPv4 form on a connection's page: how it gets its address, the
//! address, prefix and gateway set by hand, and its DNS servers. What's
//! typed is checked here before `nmcli` sees it.

use std::net::Ipv4Addr;

use amane::Ipv4Settings;

/// How a connection gets its IPv4 address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// From the network, by DHCP.
    Auto,
    /// Set by hand.
    Manual,
    /// A 169.254 address of its own, for a cable between two machines.
    LinkLocal,
    /// No IPv4 at all.
    Disabled,
}

impl Method {
    /// Every method, as the menu lists them.
    pub const ALL: [Method; 4] = [Self::Auto, Self::Manual, Self::LinkLocal, Self::Disabled];

    /// What the menu calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automatic (DHCP)",
            Self::Manual => "Manual",
            Self::LinkLocal => "Link-Local Only",
            Self::Disabled => "Disabled",
        }
    }

    /// NetworkManager's name for it.
    pub fn nm(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
            Self::LinkLocal => "link-local",
            Self::Disabled => "disabled",
        }
    }

    /// The method NetworkManager's name stands for; a hotspot's `shared`
    /// and anything unknown read as automatic.
    pub fn from_nm(name: &str) -> Self {
        match name {
            "manual" => Self::Manual,
            "link-local" => Self::LinkLocal,
            "disabled" => Self::Disabled,
            _ => Self::Auto,
        }
    }

    /// Whether DNS servers mean anything with it.
    pub fn has_dns(self) -> bool {
        matches!(self, Self::Auto | Self::Manual)
    }
}

/// What the form holds, as typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Form {
    pub method: Method,
    pub address: String,
    pub prefix: String,
    pub gateway: String,
    /// Whether DHCP's DNS servers are used; off uses only [`dns`](Self::dns).
    pub auto_dns: bool,
    /// Servers, split by commas or spaces.
    pub dns: String,
}

impl Form {
    /// The form filled from a profile's saved settings.
    pub fn from_settings(settings: &Ipv4Settings) -> Self {
        let (address, prefix) = settings
            .addresses
            .first()
            .map(|(address, prefix)| (address.clone(), prefix.to_string()))
            .unwrap_or_else(|| (String::new(), String::from("24")));
        Self {
            method: Method::from_nm(&settings.method),
            address,
            prefix,
            gateway: settings.gateway.clone(),
            auto_dns: !settings.ignore_auto_dns,
            dns: settings.dns.join(", "),
        }
    }

    /// The DNS servers typed, each checked.
    fn servers(&self) -> Result<Vec<Ipv4Addr>, String> {
        self.dns
            .split([',', ' '])
            .map(str::trim)
            .filter(|server| !server.is_empty())
            .map(|server| {
                server
                    .parse::<Ipv4Addr>()
                    .map_err(|_| format!("“{server}” isn't a DNS server address, like 1.1.1.1."))
            })
            .collect()
    }

    /// What's wrong with it, in words, or nothing.
    pub fn check(&self) -> Result<(), String> {
        if self.method == Method::Manual {
            let address = self.address.trim();
            if address.parse::<Ipv4Addr>().is_err() {
                return Err(String::from(
                    "The address needs four numbers from 0 to 255, like 192.168.1.20.",
                ));
            }
            let prefix: u8 = self
                .prefix
                .trim()
                .parse()
                .map_err(|_| String::from("The prefix is a number from 1 to 32, like 24."))?;
            if !(1..=32).contains(&prefix) {
                return Err(String::from(
                    "The prefix is a number from 1 to 32, like 24.",
                ));
            }
            let gateway = self.gateway.trim();
            if !gateway.is_empty() && gateway.parse::<Ipv4Addr>().is_err() {
                return Err(String::from(
                    "The gateway needs four numbers from 0 to 255, like 192.168.1.1, or nothing.",
                ));
            }
        }
        let servers = self.servers()?;
        if self.method == Method::Auto && !self.auto_dns && servers.is_empty() {
            return Err(String::from(
                "With automatic DNS off, name at least one DNS server.",
            ));
        }
        Ok(())
    }

    /// `nmcli`'s arguments that save it to the profile with `uuid`; only
    /// after [`check`](Self::check) passed.
    pub fn args(&self, uuid: &str) -> Vec<String> {
        let mut args: Vec<String> = [
            "connection",
            "modify",
            uuid,
            "ipv4.method",
            self.method.nm(),
        ]
        .map(String::from)
        .to_vec();
        let mut set = |key: &str, value: String| {
            args.push(key.to_string());
            args.push(value);
        };
        if self.method == Method::Manual {
            set(
                "ipv4.addresses",
                format!("{}/{}", self.address.trim(), self.prefix.trim()),
            );
            set("ipv4.gateway", self.gateway.trim().to_string());
        } else {
            // a hand-set address left behind would come back with Manual
            set("ipv4.addresses", String::new());
            set("ipv4.gateway", String::new());
        }
        let servers: Vec<String> = self
            .servers()
            .unwrap_or_default()
            .iter()
            .map(Ipv4Addr::to_string)
            .collect();
        if self.method.has_dns() {
            set("ipv4.dns", servers.join(","));
            // DHCP's servers only mean anything with DHCP
            let ignore = self.method == Method::Auto && !self.auto_dns;
            set(
                "ipv4.ignore-auto-dns",
                String::from(if ignore { "yes" } else { "no" }),
            );
        } else {
            set("ipv4.dns", String::new());
            set("ipv4.ignore-auto-dns", String::from("no"));
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manual(address: &str, prefix: &str, gateway: &str) -> Form {
        Form {
            method: Method::Manual,
            address: address.into(),
            prefix: prefix.into(),
            gateway: gateway.into(),
            auto_dns: true,
            dns: String::new(),
        }
    }

    #[test]
    fn a_manual_address_is_checked() {
        assert!(manual("192.168.1.20", "24", "192.168.1.1").check().is_ok());
        assert!(
            manual("192.168.1.20", "24", "").check().is_ok(),
            "no gateway is fine"
        );
        // a gateway outside the subnet is unusual but allowed, as NetworkManager allows it
        assert!(manual("10.0.0.5", "24", "192.168.1.1").check().is_ok());
        assert!(manual("192.168.1.300", "24", "").check().is_err());
        assert!(manual("192.168.1.20", "0", "").check().is_err());
        assert!(manual("192.168.1.20", "33", "").check().is_err());
        assert!(manual("192.168.1.20", "x", "").check().is_err());
        assert!(manual("192.168.1.20", "24", "router").check().is_err());
    }

    #[test]
    fn dns_servers_are_checked_and_needed_without_dhcps() {
        let mut form = Form {
            method: Method::Auto,
            address: String::new(),
            prefix: String::from("24"),
            gateway: String::new(),
            auto_dns: false,
            dns: String::from("1.1.1.1, 9.9.9.9"),
        };
        assert!(form.check().is_ok());
        form.dns = String::from("1.1.1.1, dns.example");
        assert!(form.check().unwrap_err().contains("dns.example"));
        form.dns = String::new();
        assert!(form.check().is_err(), "automatic DNS off needs a server");
        form.auto_dns = true;
        assert!(form.check().is_ok());
    }

    #[test]
    fn manual_writes_its_address_and_dns() {
        let mut form = manual(" 192.168.1.20 ", "24", "192.168.1.1");
        form.dns = String::from("1.1.1.1 9.9.9.9");
        assert_eq!(
            form.args("u-1"),
            [
                "connection",
                "modify",
                "u-1",
                "ipv4.method",
                "manual",
                "ipv4.addresses",
                "192.168.1.20/24",
                "ipv4.gateway",
                "192.168.1.1",
                "ipv4.dns",
                "1.1.1.1,9.9.9.9",
                "ipv4.ignore-auto-dns",
                "no",
            ]
        );
    }

    #[test]
    fn automatic_clears_what_was_set_by_hand() {
        let form = Form {
            method: Method::Auto,
            address: String::from("192.168.1.20"),
            prefix: String::from("24"),
            gateway: String::from("192.168.1.1"),
            auto_dns: false,
            dns: String::from("1.1.1.1"),
        };
        let args = form.args("u-1");
        let value = |key: &str| {
            let at = args.iter().position(|arg| arg == key).expect(key);
            args[at + 1].clone()
        };
        assert_eq!(value("ipv4.method"), "auto");
        assert_eq!(value("ipv4.addresses"), "");
        assert_eq!(value("ipv4.gateway"), "");
        assert_eq!(value("ipv4.dns"), "1.1.1.1");
        assert_eq!(value("ipv4.ignore-auto-dns"), "yes");
    }

    #[test]
    fn the_form_fills_from_saved_settings() {
        let settings = Ipv4Settings {
            method: String::from("manual"),
            addresses: vec![(String::from("10.0.0.5"), 16)],
            gateway: String::from("10.0.0.1"),
            dns: vec![String::from("1.1.1.1"), String::from("8.8.8.8")],
            ignore_auto_dns: false,
        };
        let form = Form::from_settings(&settings);
        assert_eq!(form.method, Method::Manual);
        assert_eq!(
            (form.address.as_str(), form.prefix.as_str()),
            ("10.0.0.5", "16")
        );
        assert_eq!(form.dns, "1.1.1.1, 8.8.8.8");
        assert!(form.auto_dns);
        let empty = Form::from_settings(&Ipv4Settings::default());
        assert_eq!((empty.method, empty.prefix.as_str()), (Method::Auto, "24"));
    }
}
