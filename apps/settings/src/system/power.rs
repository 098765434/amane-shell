//! Power & Lock: the idle timeouts behind hypridle (`amane.idle.*` in the
//! registry), and whether hypridle is running.

use amane::{Column, Service, Size, Widget};
use amane_kit::layout::boxed;
use amane_kit::tokens::sys;

use super::controls;
use super::services::{Units, status_text};
use crate::note;

pub fn page() -> Column {
    let units = Units::read();

    let mut parts: Vec<Box<dyn Widget>> = controls::for_page("power");
    parts.push(boxed(
        note(
            "hypridle dims the screen, locks it (swaylock) and turns it off after these idle \
             times. Locking before sleep and the lock shortcut stay as they are.",
        )
        .wrap(),
    ));
    parts.push(boxed(note(format!(
        "hypridle: {}",
        status_text(units.get("hypridle.service", true))
    ))));

    Column::new(parts)
        .gap(sys::space::gap::l())
        .width(Size::Parent)
}
