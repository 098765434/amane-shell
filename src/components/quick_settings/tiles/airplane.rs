//! Airplane Mode — GNOME's toggle: every radio off, or back on.

use amane::Service;

use super::{GridCell, on_off};
use crate::components::icons::ICON_AIRPLANE;
use crate::components::layout::boxed;
use crate::components::quick_settings::grid::Cell;
use crate::components::quick_settings::tile::QuickToggle;
use crate::services::rfkill::Rfkill;

pub(crate) fn cell() -> Option<GridCell> {
    let rfkill = Rfkill::read();
    if !rfkill.present() {
        return None;
    }
    let on = rfkill.airplane();
    drop(rfkill);

    let tile = QuickToggle::new("amane.qs.airplane", ICON_AIRPLANE, "Airplane Mode")
        .subtitle(on_off(on))
        .checked(on)
        .on_toggle(Rfkill::set_airplane)
        .build();
    Some(Cell::Half(boxed(tile)))
}
