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

impl Settings for Displays {
    const APP: &'static str = crate::app_ids::SHELL;
    const FILE: &'static str = "displays";
}

/// The display settings as they are now; a view that reads them redraws
/// when they change.
pub fn displays() -> RwLockReadGuard<'static, Config<Displays>> {
    Config::<Displays>::read()
}

/// Changes the display settings and saves them, for input handlers.
pub fn change_displays(edit: impl FnOnce(&mut Displays)) {
    Config::<Displays>::change(edit);
}
