//! Battery mode: what changes when the adapter is unplugged, beside each
//! display's refresh rate ([`power_refresh`](super::power_refresh)).
//! `power.json` ([`Power`]) says which of these to do:
//!
//! - **Power Saver.** Unplugging remembers the profile and switches to
//!   Power Saver; plugging in puts the remembered one back. A profile picked
//!   by hand on battery holds until the power source changes again. The
//!   shell starting on battery counts as an unplug.
//! - **HDR off.** Every display that can show HDR is told `hdr off`, which in
//!   the fork also pauses `hdr-auto`, so the panel stays in SDR. A display
//!   that comes back in HDR on battery (a config reload restores `hdr-auto`)
//!   is told again. Plugging in reads niri's config again, which puts back
//!   what `displays.kdl` sets.
//!
//! Checked when amane's `Battery`, `PowerProfile` or `Outputs` change, or
//! `power.json` does; the outputs are only read while HDR is held off or
//! about to be.

use std::collections::BTreeSet;
use std::sync::Once;

use amane::{Battery, Config, Outputs, PowerProfile, Service, niri};
use emaki_kit::settings::{Power, power};

use crate::services::Wake;

const SAVER: &str = "power-saver";
const BALANCED: &str = "balanced";

static WAKE: Wake = Wake::new();

/// Running on battery, as amane's `Battery` says: for views that act
/// differently unplugged (the power-mode button and key never land on
/// Performance then). A read in a view redraws it when that changes.
pub fn on_battery() -> bool {
    let battery = Battery::read();
    battery.present() && !battery.on_ac()
}

/// Starts watching, once.
pub fn watch() {
    static WATCH: Once = Once::new();
    WATCH.call_once(|| {
        Battery::subscribe(|_| WAKE.nudge()).forever();
        PowerProfile::subscribe(|_| WAKE.nudge()).forever();
        Outputs::subscribe(|_| WAKE.nudge()).forever();
        Config::<Power>::subscribe(|_| WAKE.nudge()).forever();
        std::thread::spawn(|| {
            let mut state = State::default();
            loop {
                check(&mut state);
                WAKE.wait(None);
            }
        });
    });
}

/// What battery mode has done, to undo it.
#[derive(Debug, Default)]
struct State {
    /// The power source at the last check; none before the first.
    on_battery: Option<bool>,
    /// `saver_on_battery` at the last check.
    saver_setting: Option<bool>,
    /// The profile from before Power Saver, while battery mode holds it.
    profile_before: Option<String>,
    /// Waiting to switch to Power Saver until the daemon answers.
    saver_pending: bool,
    /// The displays told `hdr off`, by connector.
    hdr_off: BTreeSet<String>,
}

fn check(state: &mut State) {
    let on_battery = on_battery();
    let settings = power().clone();

    let unplugged = on_battery && state.on_battery != Some(true);
    let plugged_in = !on_battery && state.on_battery == Some(true);
    state.on_battery = Some(on_battery);

    profile(state, on_battery, unplugged, plugged_in, &settings);
    hdr(state, on_battery && settings.hdr_off_on_battery);
}

fn profile(
    state: &mut State,
    on_battery: bool,
    unplugged: bool,
    plugged_in: bool,
    settings: &Power,
) {
    // switched on in Settings while on battery acts like an unplug
    let turned_on = settings.saver_on_battery && state.saver_setting == Some(false);
    state.saver_setting = Some(settings.saver_on_battery);
    if (unplugged || (on_battery && turned_on)) && settings.saver_on_battery {
        state.saver_pending = true;
    }
    if !on_battery || !settings.saver_on_battery {
        state.saver_pending = false;
    }

    let profiles = PowerProfile::read();
    if !profiles.present() {
        return;
    }
    let active = profiles.active().to_string();
    let available = profiles.available().to_vec();
    drop(profiles);

    let step = step(
        state.saver_pending,
        state.profile_before.as_deref(),
        plugged_in || (on_battery && !settings.saver_on_battery),
        &active,
        &available,
    );
    match step {
        Step::Stay => {}
        Step::Saver => {
            state.saver_pending = false;
            state.profile_before = Some(active);
            PowerProfile::set(SAVER);
        }
        Step::Back(profile) => {
            state.profile_before = None;
            if profile != active {
                PowerProfile::set(&profile);
            }
        }
        Step::Forget => {
            state.saver_pending = false;
            state.profile_before = None;
        }
    }
}

/// What to do with the power profile.
#[derive(Debug, PartialEq)]
enum Step {
    Stay,
    /// Switch to Power Saver, remembering the active one.
    Saver,
    /// Put this profile back.
    Back(String),
    /// Nothing to switch, nothing to remember.
    Forget,
}

/// `pending`: an unplug wants Power Saver. `before`: what was active before
/// battery mode switched. `undo`: plugged in, or the setting turned off on
/// battery.
fn step(
    pending: bool,
    before: Option<&str>,
    undo: bool,
    active: &str,
    available: &[String],
) -> Step {
    if undo {
        return match before {
            Some(before) => {
                let before = if available.iter().any(|p| p == before) {
                    before
                } else {
                    BALANCED
                };
                Step::Back(before.to_string())
            }
            None => Step::Forget,
        };
    }
    if pending {
        if !available.iter().any(|p| p == SAVER) {
            return Step::Forget;
        }
        if active == SAVER {
            // already there (picked by hand, or the daemon did): nothing to undo
            return Step::Forget;
        }
        return Step::Saver;
    }
    Step::Stay
}

fn hdr(state: &mut State, hold_off: bool) {
    if !hold_off {
        if !state.hdr_off.is_empty() {
            state.hdr_off.clear();
            load_config();
        }
        return;
    }
    // read only here: reading keeps amane's Outputs watched
    let outputs = Outputs::read();
    let wanted: Vec<String> = outputs
        .all()
        .iter()
        .filter(|output| {
            output.hdr_supported && (output.hdr_enabled || !state.hdr_off.contains(&output.name))
        })
        .map(|output| output.name.clone())
        .collect();
    drop(outputs);
    for name in wanted {
        if let Err(err) = niri::set_output(&name, niri::OutputChange::Hdr(false)) {
            log::warn!("couldn't turn {name}'s HDR off: {err}");
            continue;
        }
        state.hdr_off.insert(name);
    }
}

/// niri reads its config again, putting back what `displays.kdl` sets.
fn load_config() {
    let args = ["msg", "action", "load-config-file"].map(String::from);
    let _ = emaki_kit::process::run_now("niri", &args);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<String> {
        ["power-saver", "balanced", "performance"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn unplugging_switches_to_power_saver() {
        assert_eq!(step(true, None, false, "performance", &all()), Step::Saver);
        assert_eq!(step(true, None, false, "balanced", &all()), Step::Saver);
        // already there: nothing to put back later
        assert_eq!(step(true, None, false, "power-saver", &all()), Step::Forget);
    }

    #[test]
    fn plugging_in_puts_the_profile_back() {
        assert_eq!(
            step(false, Some("performance"), true, "power-saver", &all()),
            Step::Back(String::from("performance"))
        );
        // a profile picked by hand on battery is replaced by the one from before
        assert_eq!(
            step(false, Some("balanced"), true, "performance", &all()),
            Step::Back(String::from("balanced"))
        );
        // nothing switched: nothing to do
        assert_eq!(step(false, None, true, "power-saver", &all()), Step::Forget);
    }

    #[test]
    fn a_profile_the_machine_lost_goes_back_to_balanced() {
        let two = ["power-saver", "balanced"].map(String::from).to_vec();
        assert_eq!(
            step(false, Some("performance"), true, "power-saver", &two),
            Step::Back(String::from("balanced"))
        );
    }

    #[test]
    fn no_power_saver_means_nothing_to_switch() {
        let two = ["balanced", "performance"].map(String::from).to_vec();
        assert_eq!(step(true, None, false, "balanced", &two), Step::Forget);
    }

    #[test]
    fn a_pick_on_battery_holds() {
        assert_eq!(
            step(false, Some("balanced"), false, "performance", &all()),
            Step::Stay
        );
    }
}
