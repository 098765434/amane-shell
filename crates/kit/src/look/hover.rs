//! How a form control answers the pointer: a hover layer of its content's colour over its fill,
//! and colour crossfades, each fading on the kit's state tables under the control's id.

use amane::{Animation, Blend, Color};

use crate::buttons::{hovered, with_state};
use crate::color::over;
use crate::motion;
use crate::style;

/// `fill` with an 8% layer of `content` over it while `id` is hovered,
/// fading in and out at hover speed.
pub(crate) fn hover_fill(id: &str, fill: Color, content: Color, disabled: bool) -> Color {
    let on = !disabled && hovered(id);
    let amount = fade(&format!("{id}.mac-hover"), on, motion::HOVER);
    Color::blend(fill, over(style::hover_of(content), fill), amount)
}

/// A value fading to 1 while `on` and to 0 while not, over `duration`.
pub(crate) fn fade(key: &str, on: bool, duration: std::time::Duration) -> f32 {
    let target = if on { 1.0 } else { 0.0 };
    with_state(
        key,
        || motion::fade(target, duration),
        |fade: &mut Animation| {
            fade.to(target);
            fade.value().clamp(0.0, 1.0)
        },
    )
}

/// A colour crossfade (300 ms) between `off_color` and `on_color`.
pub(crate) fn crossfade(key: &str, on: bool, off_color: Color, on_color: Color) -> Color {
    let amount = fade(key, on, std::time::Duration::from_millis(300));
    Color::blend(off_color, on_color, amount)
}
