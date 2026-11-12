//! `power.json`: what the shell changes on battery beside the refresh rate
//! (that one is each display's, in [`displays.json`](super::Displays)).

use super::*;

/// What changes when the power adapter is unplugged, and changes back when
/// it's plugged in again. Shown on Settings' Power & Lock page.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Power {
    /// Power Saver while on battery. Unplugging switches to it; plugging in
    /// again goes back to the profile picked on the adapter (Balanced if
    /// none was). A profile picked by hand on battery stays until the
    /// power source changes.
    pub saver_on_battery: bool,

    /// HDR off while on battery: displays in HDR, or set to `hdr-auto`,
    /// stay in SDR until the adapter is back.
    pub hdr_off_on_battery: bool,
}

impl Default for Power {
    fn default() -> Self {
        Self {
            saver_on_battery: true,
            hdr_off_on_battery: true,
        }
    }
}

settings_file!(Power, crate::app_ids::SHELL, "power", power, change_power);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_are_on_unless_turned_off() {
        let none: Power = serde_json::from_str("{}").unwrap();
        assert_eq!(none, Power::default());
        assert!(none.saver_on_battery && none.hdr_off_on_battery);
        let one: Power = serde_json::from_str(r#"{"hdr_off_on_battery":false}"#).unwrap();
        assert!(one.saver_on_battery && !one.hdr_off_on_battery);
    }
}
