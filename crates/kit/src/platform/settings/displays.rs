//! `displays.json`: what the shell does with the displays beyond niri's own config.

use super::*;

/// The displays: what the shell does to them, beside what niri's own
/// `displays.kdl` keeps.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Displays {
    /// On battery, run each display at its slowest refresh rate (at the
    /// same resolution), and go back on AC.
    pub battery_refresh: bool,
}

settings_file!(
    Displays,
    crate::app_ids::SHELL,
    "displays",
    displays,
    change_displays
);
