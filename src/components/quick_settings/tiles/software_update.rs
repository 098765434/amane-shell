//! Software Update — a tile (added from Add Controls, under Utilities)
//! that draws only while the last scheduled check found newer inputs for
//! the NixOS configuration ([`Updates`]), naming them; pressing it opens
//! Shell Settings on Software Update, where the update is built and
//! applied. With nothing waiting its cells stay empty.

use amane::Service;

use crate::components::icons::ICON_UPDATE;
use crate::components::panel::close;
use crate::components::quick_settings::tile::QuickToggle;
use crate::services::installed;
use crate::services::updates::Updates;

const SETTINGS: &str = "amane-settings";

pub(crate) fn available() -> bool {
    installed(SETTINGS)
}

pub(crate) fn toggle() -> Option<QuickToggle> {
    let names = Updates::read().names();
    if names.is_empty() || !available() {
        return None;
    }
    Some(
        QuickToggle::new("amane.qs.software-update", ICON_UPDATE, "Software Update")
            .subtitle(names)
            .checked(true)
            .on_press(|| {
                close();
                amane::spawn_activated(&format!("{SETTINGS} --page update"));
            }),
    )
}
