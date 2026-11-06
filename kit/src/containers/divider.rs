//! Divider — a hairline between groups of content.
//!
//! Modelled on Material 3's full-width divider: a `sys.border.hairline`
//! line in `sys.color.separator`, with `gap.xs` of room above and below it,
//! or M3's vertical divider between groups in a row ([`Divider::vertical`]).

use amane::{Color, Parent, Rectangle};

use crate::layout::pad;
use crate::material;
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
                    .fill(material::separator()),
            )
    }

    /// A hairline `width` wide with no room above or below it, `leading`
    /// in from the start and `trailing` from the end, in `color`: M3's
    /// inset and middle-inset dividers between list items.
    pub fn inset(
        width: impl Into<amane::Size>,
        leading: f32,
        trailing: f32,
        color: Color,
    ) -> Rectangle {
        Rectangle::new()
            .width(width)
            .height(sys::border::hairline())
            .fill(Color::TRANSPARENT)
            .padding(amane::Padding {
                top: 0.0,
                right: trailing,
                bottom: 0.0,
                left: leading,
            })
            .child(
                Rectangle::new()
                    .width(Parent)
                    .height(sys::border::hairline())
                    .fill(color),
            )
    }

    /// How much width a vertical divider takes, its room included.
    pub fn width() -> f32 {
        Self::height()
    }

    /// A vertical hairline `height` tall, with `gap.xs` of room either side.
    pub fn vertical(height: f32) -> Rectangle {
        Rectangle::new()
            .width(Self::width())
            .height(height)
            .fill(Color::TRANSPARENT)
            .padding(pad(sys::space::gap::xs(), 0.0))
            .child(
                Rectangle::new()
                    .width(sys::border::hairline())
                    .height(Parent)
                    .fill(material::separator()),
            )
    }
}
