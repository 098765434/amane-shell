//! `displays.json`: what the shell does with the displays beyond niri's own config.

use super::*;

/// The displays: what the shell does to them, beside what niri's own
/// `displays.kdl` keeps. `displays.kdl` holds each display's rate on the
/// power adapter; this holds what changes on battery.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Displays {
    /// The older single switch: every display at its slowest rate on
    /// battery. Still read, as the choice of a display [`on_battery`]
    /// names nothing for; Settings no longer writes it.
    ///
    /// [`on_battery`]: Self::on_battery
    pub battery_refresh: bool,

    /// Each display's refresh rate on battery, by [`output_key`].
    pub on_battery: BTreeMap<String, BatteryRefresh>,
}

/// What a display's refresh rate does on battery.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "kebab-case", tag = "kind")]
pub enum BatteryRefresh {
    /// As on the power adapter.
    #[default]
    Same,
    /// The slowest rate the display has at its resolution.
    Slowest,
    /// The fastest rate at the display's resolution that isn't above
    /// `mhz` (millihertz, like 60000), else the slowest.
    Rate {
        /// The highest rate wanted, in millihertz.
        mhz: u32,
    },
    /// Variable refresh, always on, at the mode [`Rate`](Self::Rate) would
    /// pick for `mhz`: the panel then runs between its lowest rate and that.
    Variable {
        /// The highest rate wanted, in millihertz.
        mhz: u32,
    },
}

impl Displays {
    /// What the display `key` ([`output_key`]) does on battery.
    pub fn battery(&self, key: &str) -> BatteryRefresh {
        self.on_battery
            .get(key)
            .copied()
            .unwrap_or(if self.battery_refresh {
                BatteryRefresh::Slowest
            } else {
                BatteryRefresh::Same
            })
    }
}

/// The name a display's choices are kept under, the one `displays.kdl`
/// matches it by: the connector for a built-in panel or a display without
/// a serial, else `make model serial`, which stays the same on another port.
pub fn output_key(output: &amane::niri::Output) -> String {
    let built_in = ["eDP", "LVDS", "DSI"]
        .iter()
        .any(|kind| output.name.starts_with(kind));
    match &output.serial {
        Some(serial) if !built_in => format!("{} {} {serial}", output.make, output.model),
        _ => output.name.clone(),
    }
}

settings_file!(
    Displays,
    crate::app_ids::SHELL,
    "displays",
    displays,
    change_displays
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_switch_reads_as_slowest_for_unnamed_displays() {
        let mut displays = Displays {
            battery_refresh: true,
            ..Displays::default()
        };
        assert_eq!(displays.battery("eDP-1"), BatteryRefresh::Slowest);
        displays
            .on_battery
            .insert(String::from("eDP-1"), BatteryRefresh::Rate { mhz: 60_000 });
        assert_eq!(
            displays.battery("eDP-1"),
            BatteryRefresh::Rate { mhz: 60_000 }
        );
        assert_eq!(Displays::default().battery("eDP-1"), BatteryRefresh::Same);
    }

    #[test]
    fn choices_round_trip_through_json() {
        let mut displays = Displays::default();
        displays.on_battery.insert(
            String::from("eDP-1"),
            BatteryRefresh::Variable { mhz: 60_000 },
        );
        displays
            .on_battery
            .insert(String::from("DP-1"), BatteryRefresh::Same);
        let json = serde_json::to_string(&displays).unwrap();
        assert!(json.contains(r#""kind":"variable","mhz":60000"#), "{json}");
        let back: Displays = serde_json::from_str(&json).unwrap();
        assert_eq!(back, displays);
        // a file from before the choices
        let old: Displays = serde_json::from_str(r#"{"battery_refresh":true}"#).unwrap();
        assert!(old.battery_refresh && old.on_battery.is_empty());
    }
}
