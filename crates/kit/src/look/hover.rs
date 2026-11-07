//! How a form control answers the pointer: a hover layer of its content's colour over its fill,
//! and colour crossfades, each fading on the kit's state tables under the control's id.

use amane::{Blend, Color};

use crate::buttons::{fade, hover_layer};
use crate::motion;
use crate::style;

/// `fill` with an 8% layer of `content` over it while `id` is hovered.
pub(crate) fn hover_fill(id: &str, fill: Color, content: Color, disabled: bool) -> Color {
    hover_layer(id, fill, style::hover_of(content), disabled)
}

/// A colour crossfade between `off_color` and `on_color`.
pub(crate) fn crossfade(key: &str, on: bool, off_color: Color, on_color: Color) -> Color {
    Color::blend(off_color, on_color, fade(key, on, motion::CROSSFADE))
}
