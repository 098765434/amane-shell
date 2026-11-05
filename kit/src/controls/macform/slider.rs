//! MacSlider — a 4 px track (radius 2, `controlFill`), the accent filling
//! it to the knob, and a 20×16 white knob (radius 5.6) in a 1 px
//! `fieldBorder` with a soft shadow. The value is 0.0–1.0; a press jumps
//! to it, a drag follows, the wheel steps.
//!
//! Two of the original's slips are fixed: the fill ends under the knob's
//! centre, and disabled fades the colours rather than replacing their
//! alpha (which made the disabled track brighter).

use std::rc::Rc;

use amane::{Center, Color, Point, Pointer, Rectangle, Scroll, Shadow, Size, Stack, Start};

use super::DISABLED_OPACITY;
use super::theme::{self, px};
use crate::color::faded;
use crate::layout::boxed;

pub struct MacSlider {
    value: f32,
    width: f32,
    step: f32,
    disabled: bool,
    on_change: Rc<dyn Fn(f32)>,
}

impl MacSlider {
    pub fn new(value: f32, width: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            width,
            step: 0.05,
            disabled: false,
            on_change: Rc::new(|_| {}),
        }
    }

    /// How far one wheel notch moves it (0.05 by default).
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(mut self, change: impl Fn(f32) + 'static) -> Self {
        self.on_change = Rc::new(change);
        self
    }

    pub fn build(self) -> Rectangle {
        let width = self.width;
        let height = px(24.0);
        let (knob_w, knob_h) = (px(20.0), px(16.0));
        let track_h = px(4.0);
        let inset = px(1.0);
        let travel = (width - knob_w).max(1.0);
        let fade = |c: Color, amount: f32| if self.disabled { faded(c, amount) } else { c };

        let knob_x = self.value * travel;
        let fill_w = (knob_x + knob_w / 2.0 - inset).max(track_h);

        let track = Rectangle::new()
            .width(Size::Fixed(width - inset * 2.0))
            .height(Size::Fixed(track_h))
            .radius(track_h / 2.0)
            .fill(fade(theme::control_fill(), 0.5))
            .align_child(Start, Center)
            .child(
                Rectangle::new()
                    .width(Size::Fixed(fill_w))
                    .height(Size::Fixed(track_h))
                    .radius(track_h / 2.0)
                    .fill(fade(theme::tint(), 0.4)),
            );

        let knob = Rectangle::new()
            .width(Size::Fixed(knob_w))
            .height(Size::Fixed(knob_h))
            .radius(knob_h.min(knob_w) * 0.35)
            .fill(theme::knob())
            .border(1.0, theme::field_border())
            .shadow(
                // the original's elevation 1
                Shadow::drop(theme::shadow_color())
                    .blur(3.09)
                    .spread(-0.29)
                    .offset(0.0, 0.5),
            )
            .opacity(if self.disabled { DISABLED_OPACITY } else { 1.0 })
            .translate(knob_x, (height - knob_h) / 2.0);

        let layer = |child: Rectangle| {
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(width))
                    .height(Size::Fixed(height))
                    .padding(crate::layout::pad(inset, 0.0))
                    .align_child(Start, Center)
                    .child(child),
            )
        };

        let base = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height))
            .fill(Color::TRANSPARENT)
            .child(Stack::new(vec![layer(track), boxed(knob)]));

        if self.disabled {
            return base;
        }

        let value = self.value;
        let step = self.step;
        let set = self.on_change;
        let scroll = set.clone();
        base.cursor(Pointer)
            .on_drag(move |p: Point| {
                let next = ((p.x - knob_w / 2.0) / travel).clamp(0.0, 1.0);
                if (next - value).abs() > f32::EPSILON {
                    set(next);
                }
            })
            .on_scroll(move |s: Scroll| {
                let next = if s.y < 0.0 {
                    value + step
                } else if s.y > 0.0 {
                    value - step
                } else {
                    return;
                };
                scroll(next.clamp(0.0, 1.0));
            })
    }
}
