//! A text field's frame: a 24 px box, radius 6, `fieldFill` in a 1 px edge, the text 8 px in.
//! The edge takes the accent while the field has the keys. [`app`](crate::app)'s text fields
//! and [`field_box`](crate::app::field_box) draw with it.

use amane::{Center, Rectangle, Size, TextInput};

use crate::style::{self, px};

/// The edge of field `id`: the accent while it has the keys, else the
/// hairline `fieldBorder`.
pub(crate) fn edge(id: &'static str) -> amane::Color {
    if TextInput::has_focus(id) {
        style::tint()
    } else {
        style::field_border()
    }
}

/// The box a text input sits in: 24 px tall, radius 6, `fieldFill` in a
/// 1 px `edge`, the text 8 px in. [`field_box`](crate::app::field_box)
/// draws with it too.
pub(crate) fn frame(input: TextInput, width: impl Into<Size>, edge: amane::Color) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(style::control_height())
        .radius(style::field_radius())
        .fill(style::field_fill())
        .border(1.0, edge)
        .padding(crate::layout::pad_x(px(8.0), px(4.0)))
        .align_child(amane::Start, Center)
        .clip()
        .child(input)
}
