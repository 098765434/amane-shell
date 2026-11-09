//! What the Network page asks `nmcli` to do: change a saved profile,
//! import a VPN, run a hotspot. Reading goes through amane's `Network`
//! over D-Bus. These go through nmcli because it merges a change into a
//! profile with its stored secrets kept, and it carries the VPN plugins'
//! import parsers.

use std::path::{Path, PathBuf};

/// What `nmcli connection import` calls a file's kind, by its extension.
pub fn import_kind(file: &Path) -> Option<&'static str> {
    match file.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "ovpn" => Some("openvpn"),
        "conf" => Some("wireguard"),
        _ => None,
    }
}

/// Imports `file` as a VPN.
pub fn import_args(kind: &str, file: &Path) -> Vec<String> {
    vec![
        String::from("connection"),
        String::from("import"),
        String::from("type"),
        kind.to_string(),
        String::from("file"),
        file.to_string_lossy().into_owned(),
    ]
}

/// Whether NetworkManager brings the profile up by itself.
pub fn autoconnect_args(uuid: &str, on: bool) -> Vec<String> {
    ["connection", "modify", uuid, "connection.autoconnect"]
        .into_iter()
        .chain([if on { "yes" } else { "no" }])
        .map(String::from)
        .collect()
}

/// Brings a profile up again, so a changed one takes its new settings.
pub fn up_args(uuid: &str) -> Vec<String> {
    ["connection", "up", uuid].map(String::from).to_vec()
}

/// The hotspot's profile, made or changed by `nmcli device wifi hotspot`.
pub const HOTSPOT: &str = "Hotspot";

/// Starts the hotspot named `ssid`; an empty password lets nmcli make one.
pub fn hotspot_args(ssid: &str, password: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "device", "wifi", "hotspot", "con-name", HOTSPOT, "ssid", ssid,
    ]
    .map(String::from)
    .to_vec();
    if !password.is_empty() {
        args.push(String::from("password"));
        args.push(password.to_string());
    }
    args
}

/// The hotspot profile's name and password, one a line.
pub fn hotspot_show_args() -> Vec<String> {
    [
        "-s",
        "-g",
        "802-11-wireless.ssid,802-11-wireless-security.psk",
        "connection",
        "show",
        HOTSPOT,
    ]
    .map(String::from)
    .to_vec()
}

/// The hotspot's name and password from [`hotspot_show_args`]'s answer.
pub fn hotspot_values(out: &str) -> (String, String) {
    let mut lines = out.lines().map(|line| line.trim().to_string());
    let ssid = lines.next().unwrap_or_default();
    let password = lines.next().unwrap_or_default();
    (ssid, password)
}

/// Whether NetworkManager has its OpenVPN plugin, which importing an
/// `.ovpn` file needs; WireGuard is built in.
pub fn openvpn_available() -> bool {
    plugin_dirs()
        .iter()
        .any(|dir| dir.join("nm-openvpn-service.name").is_file())
}

fn plugin_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/etc/NetworkManager/VPN"),
        PathBuf::from("/run/current-system/sw/lib/NetworkManager/VPN"),
        PathBuf::from("/usr/lib/NetworkManager/VPN"),
    ];
    if let Ok(data) = std::env::var("XDG_DATA_DIRS") {
        dirs.extend(
            std::env::split_paths(&data)
                .filter_map(|dir| dir.parent().map(|lib| lib.join("lib/NetworkManager/VPN"))),
        );
    }
    dirs
}

/// What to add for OpenVPN, said where the import fails without it.
pub const OPENVPN_HINT: &str = "Importing an OpenVPN file needs NetworkManager's OpenVPN \
     plugin. Add networkmanager-openvpn to networking.networkmanager.plugins in \
     your NixOS configuration and rebuild.";

/// Runs `nmcli` with `args` to the end: its answer, or what went wrong.
/// Blocks, so it runs on a thread of its own (see `super::Job`).
pub fn run(args: &[String]) -> Result<String, String> {
    emaki_kit::process::Cmd::new("nmcli")
        .args(args)
        .stdout()
        .map(|out| out.trim().to_string())
}

/// The useful line of nmcli's error, without its `Error:` prefix.
pub fn said(error: &str) -> String {
    let line = error
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("Error:"))
        .or_else(|| error.lines().map(str::trim).find(|line| !line.is_empty()))
        .unwrap_or("nmcli failed");
    line.trim_start_matches("Error:").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_imports_by_its_extension() {
        assert_eq!(import_kind(Path::new("/x/office.ovpn")), Some("openvpn"));
        assert_eq!(import_kind(Path::new("/x/wg0.CONF")), Some("wireguard"));
        assert_eq!(import_kind(Path::new("/x/notes.txt")), None);
        assert_eq!(import_kind(Path::new("/x/wg0")), None);
        assert_eq!(
            import_args("wireguard", Path::new("/x/wg0.conf")),
            [
                "connection",
                "import",
                "type",
                "wireguard",
                "file",
                "/x/wg0.conf"
            ]
        );
    }

    #[test]
    fn a_hotspot_without_a_password_gets_one_made() {
        assert_eq!(
            hotspot_args("Desk", ""),
            [
                "device", "wifi", "hotspot", "con-name", "Hotspot", "ssid", "Desk"
            ]
        );
        assert_eq!(
            hotspot_args("Desk", "secret12").last().map(String::as_str),
            Some("secret12")
        );
        assert_eq!(
            hotspot_values("Desk\nsecret12\n"),
            (String::from("Desk"), String::from("secret12"))
        );
        assert_eq!(hotspot_values(""), (String::new(), String::new()));
    }

    #[test]
    fn autoconnect_says_yes_or_no() {
        assert_eq!(
            autoconnect_args("u-1", false),
            [
                "connection",
                "modify",
                "u-1",
                "connection.autoconnect",
                "no"
            ]
        );
    }

    #[test]
    fn nmcli_errors_read_without_their_prefix() {
        let out = "Warning: something\nError: failed to import: the plugin is missing.\n";
        assert_eq!(said(out), "failed to import: the plugin is missing.");
        assert_eq!(said("plain words"), "plain words");
        assert_eq!(said(""), "nmcli failed");
    }
}
