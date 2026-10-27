//! General: the computer's name, its language, and the keyboard layout
//! and variant, each a managed setting; the console keymap follows the
//! keyboard layout (`console.useXkbConfig`).

use amane::{Column, Size, Widget};
use amane_kit::layout::boxed;
use amane_kit::tokens::sys;

use super::controls;
use crate::note;

pub fn page() -> Column {
    let mut parts: Vec<Box<dyn Widget>> = controls::for_page("general");
    parts.push(boxed(
        note(
            "The name and keyboard take effect after Apply; a new language needs a restart. \
             Layouts and variants are the names localectl lists; the console keymap follows \
             the layout.",
        )
        .wrap(),
    ));
    Column::new(parts)
        .gap(sys::space::gap::l())
        .width(Size::Parent)
}
