//! General: the computer's name, its language and its time zone, each a
//! managed setting. The keyboard layout is on Keyboard.

use amane::Column;

use super::controls;
use crate::{group, groups};

pub fn page() -> Column {
    groups(vec![group().rows(controls::rows("general")).footer(
        "Changes take effect after Apply; a new language needs a restart.",
    )])
}
