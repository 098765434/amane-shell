//! General: the computer's name, its language, and the keyboard layout
//! and variant, each a managed setting; the console keymap follows the
//! keyboard layout (`console.useXkbConfig`).

use amane::Column;

use super::controls;
use crate::{group, groups};

pub fn page() -> Column {
    groups(vec![group().rows(controls::rows("general")).footer(
        "The name and keyboard take effect after Apply; a new language needs a restart. \
         Layouts and variants are the names localectl lists; the console keymap follows \
         the layout.",
    )])
}
