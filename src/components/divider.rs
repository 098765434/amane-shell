//! Divider — a hairline between groups of content.
//!
//! Modelled on Material 3's full-width divider: a `sys.border.hairline`
//! line in `sys.color.separator`, with `gap.xs` of room above and below it.

use amane::{Color, Parent, Rectangle};

use crate::components::layout::pad;
use crate::tokens::sys;

pub struct Divider;

impl Divider {
    /// How much height a divider takes, its room included.
    pub fn height() -> f32 {
        sys::border::hairline() + 2.0 * sys::space::gap::xs()
    }

    pub fn build() -> Rectangle {
        Rectangle::new()
            .width(Parent)
            .height(Self::height())
            .fill(Color::TRANSPARENT)
            .padding(pad(0.0, sys::space::gap::xs()))
            .child(
                Rectangle::new()
                    .width(Parent)
                    .height(sys::border::hairline())
                    .fill(sys::color::separator()),
            )
    }
}
