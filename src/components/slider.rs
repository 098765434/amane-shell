//! Slider — picks a level from 0 to 100 along a track.
//!
//! Modelled on Material 3's slider: an active track filled up to the value
//! over an inactive one, set by pressing or dragging anywhere along it, and
//! discrete [`STEP`]s for the scroll wheel. Drawn in the macOS look:
//! a thin capsule track with no handle, the accent filling it; a hit area a
//! full control tall, so the thin track is easy to grab.

use amane::{Center, Color, Point, Pointer, Rectangle, Scroll, Size, Start};

use crate::tokens::sys;

/// How far one scroll notch moves a level, the bar's volume button included.
pub const STEP: u8 = 5;

/// `value` moved one `step` by a scroll: up (negative y) raises, down lowers.
pub fn scrolled(value: u8, scroll: Scroll, step: u8) -> u8 {
    if scroll.y < 0.0 {
        value.saturating_add(step).min(100)
    } else {
        value.saturating_sub(step)
    }
}

pub struct Slider {
    value: u8,
    width: f32,
    dimmed: bool,
    on_value_change: fn(u8),
}

impl Slider {
    pub fn new(value: u8, width: f32) -> Self {
        Self {
            value: value.min(100),
            width,
            dimmed: false,
            on_value_change: |_| {},
        }
    }

    /// Greys the active track, as while muted; it still takes input.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    pub fn on_value_change(mut self, change: fn(u8)) -> Self {
        self.on_value_change = change;
        self
    }

    pub fn build(self) -> Rectangle {
        let Self {
            value,
            width,
            dimmed,
            on_value_change: set,
        } = self;
        let track_h = sys::size::slider::height();
        let active = if dimmed {
            sys::color::content::secondary()
        } else {
            sys::color::accent::bg()
        };

        let track = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(track_h))
            .radius(track_h / 2.0)
            .fill(sys::color::surface::raised::bg())
            .clip()
            .child(
                Rectangle::new()
                    .width(Size::Fixed(width * f32::from(value) / 100.0))
                    .height(Size::Fixed(track_h))
                    .radius(track_h / 2.0)
                    .fill(active),
            );

        Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(sys::size::control::height()))
            .fill(Color::TRANSPARENT)
            .align_child(Start, Center)
            .cursor(Pointer)
            .on_drag(move |p: Point| {
                // the drag keeps reporting past either end, so it is clamped
                let ratio = (p.x / width).clamp(0.0, 1.0);
                set((ratio * 100.0).round() as u8);
            })
            .on_scroll(move |scroll: Scroll| set(scrolled(value, scroll, STEP)))
            .child(track)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scroll(y: f32) -> Scroll {
        Scroll { x: 0.0, y }
    }

    #[test]
    fn scrolling_steps_within_range() {
        assert_eq!(scrolled(50, scroll(-1.0), STEP), 55);
        assert_eq!(scrolled(50, scroll(1.0), STEP), 45);
        assert_eq!(scrolled(98, scroll(-1.0), STEP), 100);
        assert_eq!(scrolled(3, scroll(1.0), STEP), 0);
    }
}
