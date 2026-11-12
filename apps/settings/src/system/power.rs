//! Power & Lock: how long the computer waits, idle, before it dims the
//! screen, locks it and turns it off (`emaki.idle.*` in the registry), and
//! whether the idle service that does it is running.

use amane::{Column, Service};
use emaki_kit::form::{FormRow, Group};

use emaki_kit::settings::{change_power, power};

use super::battery;
use super::controls;
use super::nixos::{Managed, Snapshot};
use super::services::{Units, status_text};
use crate::{group, groups, switch_row};

/// The waits, in the order they come.
const STEPS: [(&str, &str); 3] = [
    ("dimAfter", "dims"),
    ("lockAfter", "locks"),
    ("screenOffAfter", "turns off"),
];

/// A wait set to happen before one that should come first: "The screen
/// locks before it dims."
fn out_of_order(waits: &[(&str, u64)]) -> Option<String> {
    // 0 is never, so it can't be early
    let set: Vec<&(&str, u64)> = waits.iter().filter(|(_, wait)| *wait > 0).collect();
    set.windows(2).find_map(|pair| {
        let ((earlier, first), (later, second)) = (pair[0], pair[1]);
        (second < first).then(|| format!("The screen {later} before it {earlier}."))
    })
}

pub fn page() -> Column {
    let units = Units::read();
    let snapshot = Snapshot::read();
    let managed = Managed::read();

    let waits: Vec<(&str, u64)> = STEPS
        .iter()
        .filter_map(|(id, verb)| {
            let entry = snapshot.entry(id)?;
            Some((*verb, managed.value(id, &entry.value).as_u64()?))
        })
        .collect();
    let warning = out_of_order(&waits)
        .map(|why| format!("{why} Each step waits for the one before it.\n"))
        .unwrap_or_default();

    let idle = FormRow::new("settings.power.hypridle", "Idle service")
        .subtitle("What dims, locks and turns off the screen");
    let idle = if units.looked {
        idle.value(
            match status_text(units.get("hypridle.service", true)).as_str() {
                "Off" => String::from("Not running"),
                text => text.to_string(),
            },
        )
    } else {
        idle.loading()
    };

    groups(vec![
        battery::section(),
        on_battery(),
        group().rows(controls::rows("power")).footer(format!(
            "{warning}After this long without input the screen dims, then locks, then turns \
             off. Locking when the lid closes and the lock shortcut work as before."
        )),
        group().row(idle),
    ])
}

/// What the shell changes while unplugged (`power.json`), undone when the
/// adapter is back. Live: no Apply.
fn on_battery() -> Group {
    let settings = power().clone();
    group()
        .header("On Battery")
        .row(switch_row(
            "settings.power.saver-on-battery",
            "Use Power Saver",
            settings.saver_on_battery,
            |on| change_power(|now| now.saver_on_battery = on),
        ))
        .row(switch_row(
            "settings.power.hdr-off-on-battery",
            "Turn HDR off",
            settings.hdr_off_on_battery,
            |on| change_power(|now| now.hdr_off_on_battery = on),
        ))
        .footer(
            "Unplugging switches to Power Saver and keeps displays in SDR; plugging in puts \
             back the power mode and HDR from before. A power mode picked on battery stays \
             until you plug in. Each display's refresh rate on battery is on Displays.",
        )
}

#[cfg(test)]
mod tests {
    use super::out_of_order;

    #[test]
    fn a_step_before_the_one_it_follows_is_said() {
        assert_eq!(
            out_of_order(&[("dims", 180), ("locks", 240), ("turns off", 300)]),
            None
        );
        assert_eq!(
            out_of_order(&[("dims", 300), ("locks", 240), ("turns off", 0)]).as_deref(),
            Some("The screen locks before it dims.")
        );
        assert_eq!(out_of_order(&[("dims", 0), ("locks", 60)]), None);
    }
}
