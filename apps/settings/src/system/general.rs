//! General: the computer's name, its language, and the keyboard layout
//! and variant, each a managed setting; the console keymap follows the
//! keyboard layout (`console.useXkbConfig`).

use amane::Column;

use super::controls;
use crate::{group, groups};

pub fn page() -> Column {
    groups(vec![group().rows(controls::rows("general")).footer(
        "Changes take effect after Apply; a new language needs a restart. A layout is a \
         short name like us or no, and a variant one of its kinds, like dvorak; the text \
         console uses the same layout.",
    )])
}
