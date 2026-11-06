//! Power & Lock: the idle timeouts behind hypridle (`amane.idle.*` in the
//! registry), and whether hypridle is running.

use amane::{Column, Service};
use amane_kit::form::FormRow;

use super::controls;
use super::services::{Units, status_text};
use crate::{group, groups};

pub fn page() -> Column {
    let units = Units::read();

    groups(vec![
        group().rows(controls::rows("power")).footer(
            "hypridle dims the screen, locks it (swaylock) and turns it off after these idle \
             times. Locking before sleep and the lock shortcut stay as they are.",
        ),
        group().row({
            let row = FormRow::new("settings.power.hypridle", "hypridle");
            if units.looked {
                row.value(status_text(units.get("hypridle.service", true)))
            } else {
                row.loading()
            }
        }),
    ])
}
