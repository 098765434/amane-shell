//! What a reading says and how far back its graph goes: temperatures in
//! the unit a widget is set to, rates in bytes or bits, sensors and
//! interfaces by names a person knows, and a series over the range picked.

use crate::services::desktop::system_stats::{HISTORY, Series};
use crate::surfaces::widgets::store::Settings;

use super::SPARK_POINTS;

/// `celsius` as a widget set to `settings` shows it: "52°", or "126°F".
pub fn degrees(celsius: f32, settings: &Settings) -> String {
    if settings.get("unit") == Some("f") {
        format!("{:.0}°F", celsius * 9.0 / 5.0 + 32.0)
    } else {
        format!("{celsius:.0}°")
    }
}

/// `per_second` bytes a second: "1.2 MB/s", or with `bits`, "9.9 Mb/s".
pub fn rate(per_second: f32, bits: bool) -> String {
    let per_second = per_second.max(0.0);
    if !bits {
        return format!("{}/s", emaki_kit::format::bytes(per_second as u64));
    }
    let bits = per_second * 8.0;
    let (value, unit) = [(1e9, "Gb/s"), (1e6, "Mb/s"), (1e3, "kb/s")]
        .into_iter()
        .find(|(step, _)| bits >= *step)
        .map_or((bits, "b/s"), |(step, unit)| (bits / step, unit));
    if value >= 100.0 || unit == "b/s" {
        format!("{value:.0} {unit}")
    } else {
        format!("{value:.1} {unit}")
    }
}

/// Whether a network widget counts in bits.
pub fn in_bits(settings: &Settings) -> bool {
    settings.get("unit") == Some("bits")
}

/// A sensor's label ("k10temp Tctl", "nvme Composite 2") as a person
/// would say it ("CPU Tctl", "NVMe 2"): the driver by what it watches, and
/// a label that only repeats it left out.
pub fn sensor_name(label: &str) -> String {
    let (driver, rest) = label.split_once(' ').unwrap_or((label, ""));
    let what = match driver {
        "k10temp" | "coretemp" | "zenpower" | "cpu_thermal" => "CPU",
        "amdgpu" | "nouveau" | "radeon" | "i915" | "xe" => "GPU",
        "nvme" => "NVMe",
        "acpitz" => "ACPI",
        "drivetemp" => "Drive",
        "mt7921_phy0" | "iwlwifi_1" | "ath11k_hwmon" => "Wi-Fi",
        "spd5118" | "jc42" => "Memory",
        _ => return label.to_string(),
    };
    // "Composite" says nothing a person reads; a number after it does
    let rest = rest.strip_prefix("Composite").map_or(rest, str::trim_start);
    if rest.is_empty() {
        what.to_string()
    } else {
        format!("{what} {rest}")
    }
}

/// What kind of link an interface is, by its kernel name.
pub fn interface_kind(name: &str) -> &'static str {
    let starts = |prefixes: &[&str]| prefixes.iter().any(|p| name.starts_with(p));
    if starts(&["wl", "wlan", "wifi"]) {
        "Wi-Fi"
    } else if starts(&["tun", "tap", "wg", "ppp", "tailscale", "nordlynx", "proton"]) {
        "VPN"
    } else if starts(&["usb", "enx", "rndis"]) {
        "USB Ethernet"
    } else if starts(&["en", "eth"]) {
        "Ethernet"
    } else if starts(&["ww"]) {
        "Mobile"
    } else {
        "Network"
    }
}

/// An interface as its menu names it: "Wi-Fi (wlp194s0)".
pub fn interface_name(name: &str) -> String {
    format!("{} ({name})", interface_kind(name))
}

/// The readings a graph of `series` shows, oldest first: the last two
/// minutes, or with the range set to `15`, the last quarter hour, every
/// few readings averaged into one, so it spans as many points either way.
pub fn graph(series: &Series, settings: &Settings) -> Vec<f32> {
    if settings.get("range") == Some("15") {
        downsample(&series.recent(HISTORY).collect::<Vec<_>>(), HISTORY)
    } else {
        series.recent(SPARK_POINTS).collect()
    }
}

/// `values` from a window of `span` readings, as [`SPARK_POINTS`] points
/// across it: each the mean of its share (`span / SPARK_POINTS` readings,
/// a share's edges falling between readings where that isn't whole),
/// counted back from the newest. A window not yet full gives fewer, so the
/// graph still fills from the right.
pub fn downsample(values: &[f32], span: usize) -> Vec<f32> {
    let share = span as f32 / SPARK_POINTS as f32;
    let len = values.len();
    let points = ((len as f32 / share).ceil() as usize).min(SPARK_POINTS);
    let mut out: Vec<f32> = (0..points)
        .filter_map(|k| {
            // the k-th share back from the newest
            let end = len as f32 - share * k as f32;
            let start = (end - share).max(0.0);
            let (start, end) = (start.round() as usize, (end.round() as usize).min(len));
            let part = values.get(start..end).filter(|p| !p.is_empty())?;
            Some(part.iter().sum::<f32>() / part.len() as f32)
        })
        .collect();
    out.reverse();
    out
}

/// What `find` gives for `wanted`; when that's gone (a drive taken out, a
/// VPN down), what it gives for the automatic choice, said so with `true`.
pub fn or_automatic<'a, T>(
    wanted: Option<&'a str>,
    find: impl Fn(Option<&'a str>) -> Option<T>,
) -> (Option<T>, bool) {
    match find(wanted) {
        Some(found) => (Some(found), false),
        None if wanted.is_some() => (find(None), true),
        None => (None, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_say_what_they_are_set_to() {
        let plain = Settings::default();
        let fahrenheit = Settings::default().with("unit", Some(String::from("f")));
        assert_eq!(degrees(52.4, &plain), "52°");
        assert_eq!(degrees(100.0, &fahrenheit), "212°F");
        assert_eq!(rate(1_234_567.0, false), "1.2 MB/s");
        assert_eq!(rate(1_234_567.0, true), "9.9 Mb/s");
        assert_eq!(rate(20_000_000.0, true), "160 Mb/s");
        assert_eq!(rate(10.0, true), "80 b/s");
    }

    #[test]
    fn sensors_and_interfaces_have_names_a_person_knows() {
        assert_eq!(sensor_name("k10temp Tctl"), "CPU Tctl");
        assert_eq!(sensor_name("nvme Composite"), "NVMe");
        assert_eq!(sensor_name("nvme Composite 2"), "NVMe 2");
        assert_eq!(sensor_name("amdgpu edge"), "GPU edge");
        assert_eq!(sensor_name("thinkpad CPU"), "thinkpad CPU");
        assert_eq!(interface_name("wlp194s0"), "Wi-Fi (wlp194s0)");
        assert_eq!(interface_kind("enp5s0"), "Ethernet");
        assert_eq!(interface_kind("enx001122334455"), "USB Ethernet");
        assert_eq!(interface_kind("wg0"), "VPN");
        assert_eq!(interface_kind("docker0"), "Network");
    }

    #[test]
    fn a_quarter_hour_spans_as_many_points_as_two_minutes() {
        let full: Vec<f32> = (0..HISTORY).map(|n| n as f32).collect();
        let points = downsample(&full, HISTORY);
        assert_eq!(points.len(), SPARK_POINTS);
        // in order, oldest first
        assert!(points.windows(2).all(|w| w[0] < w[1]));
        // the newest share ends on the newest reading
        assert!(*points.last().unwrap() > (HISTORY - 10) as f32);
        // a window not yet full gives fewer
        // 16 readings: two shares of 7.5 and what came before them
        assert_eq!(downsample(&full[..16], HISTORY).len(), 3);
        assert!(downsample(&[], HISTORY).is_empty());
    }

    #[test]
    fn a_choice_gone_falls_back_to_the_automatic_one() {
        let find = |name: Option<&str>| match name {
            None => Some(String::from("eth0")),
            Some("wg0") => None,
            Some(other) => Some(other.to_string()),
        };
        let found = |name: &str| Some(name.to_string());
        assert_eq!(or_automatic(Some("wlan0"), find), (found("wlan0"), false));
        assert_eq!(or_automatic(Some("wg0"), find), (found("eth0"), true));
        assert_eq!(or_automatic(None, find), (found("eth0"), false));
    }
}
