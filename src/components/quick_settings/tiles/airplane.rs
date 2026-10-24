//! Airplane Mode — GNOME's toggle: every radio off, or back on. A row of
//! a round button or a tile.

use amane::Service;

use super::on_off;
use crate::components::icons::ICON_AIRPLANE;
use crate::components::quick_settings::tile::QuickToggle;
use crate::services::rfkill::Rfkill;

pub(crate) fn toggle() -> Option<QuickToggle> {
    let rfkill = Rfkill::read();
    if !rfkill.present() {
        return None;
    }
    let on = rfkill.airplane();
    drop(rfkill);

    Some(
        QuickToggle::new("amane.qs.airplane", ICON_AIRPLANE, "Airplane Mode")
            .subtitle(on_off(on))
            .checked(on)
            .on_toggle(Rfkill::set_airplane),
    )
}
