//! Stepper — a number field with up and down halves: 64×24, radius 6,
//! `fieldFill` in a 1 px `fieldBorder`, the halves in a 20 px column
//! behind a 1 px line. Enter takes a typed number; the wheel over it
//! steps.
//!
//! Holding a half repeats, as in the original: one step at once, the next
//! after 400 ms, then 50 ms sooner each time down to every 50 ms. The
//! steps count up on screen while held and are reported once, on release
//! (amane has no timers, so the hold is played out from the clock as the
//! view redraws). Values are clamped to `from..=to` and rounded to the
//! step.

use std::rc::Rc;
use std::time::Instant;

use amane::{
    Center, Color, Column, Point, Pointer, Rectangle, Row, Scroll, Size, TextInput, request_frame,
};

use crate::buttons::DISABLED_OPACITY;
use crate::buttons::{hover, with_state};
use crate::layout::boxed;
use crate::look::hover::hover_fill;
use crate::style::{self, px};
use crate::symbol::{Symbol, symbol};

/// A half held down: which way, since when, and the value it started at.
#[derive(Clone, Copy)]
struct Hold {
    dir: f64,
    since: Instant,
    from: f64,
}

#[derive(Default)]
struct State {
    hold: Option<Hold>,
    /// The value last put in the field.
    written: Option<f64>,
}

/// How many steps a hold of `ms` milliseconds has made.
fn steps_after(ms: u128) -> u32 {
    let mut steps = 1;
    let mut at: u128 = 400;
    let mut interval: u128 = 400;
    while at <= ms {
        steps += 1;
        interval = interval.saturating_sub(50).max(50);
        at += interval;
    }
    steps
}

/// How many decimals a step needs shown.
fn decimals(step: f64) -> usize {
    if step < 1.0 {
        (-step.log10()).ceil().max(1.0) as usize
    } else {
        0
    }
}

/// A number field with up and down halves: a click steps, holding repeats, the wheel steps,
/// and typing a number and Enter sets it. The change is reported once the press ends.
pub struct Stepper {
    id: &'static str,
    value: f64,
    from: f64,
    to: f64,
    step: f64,
    width: f32,
    disabled: bool,
    on_change: Rc<dyn Fn(f64)>,
}

impl Stepper {
    /// A stepper showing `value`; `id` keys its field and its hold.
    pub fn new(id: impl AsRef<str>, value: f64) -> Self {
        Self {
            id: crate::intern::intern(id.as_ref()),
            value,
            from: 0.0,
            to: 99.0,
            step: 1.0,
            width: px(64.0),
            disabled: false,
            on_change: Rc::new(|_| {}),
        }
    }

    /// The lowest and highest it goes.
    pub fn range(mut self, from: f64, to: f64) -> Self {
        self.from = from;
        self.to = to;
        self
    }

    /// How much one step changes it; 1 unless set.
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// How wide the field is.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Shown at half strength, taking no input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// What a new value does.
    pub fn on_change(mut self, change: impl Fn(f64) + 'static) -> Self {
        self.on_change = Rc::new(change);
        self
    }

    /// The field and its halves.
    pub fn build(self) -> Rectangle {
        let Self {
            id,
            value,
            from,
            to,
            step,
            width,
            disabled,
            on_change,
        } = self;
        let fix = move |v: f64| ((v / step).round() * step).clamp(from, to);
        let places = decimals(step);
        let key = format!("{id}.stepper");

        // the value on screen: a held half's steps over the given one
        let shown = with_state(&key, State::default, |state: &mut State| {
            let shown = match state.hold {
                Some(hold) => {
                    request_frame();
                    let steps = f64::from(steps_after(hold.since.elapsed().as_millis()));
                    fix(hold.from + hold.dir * steps * step)
                }
                None => value,
            };
            if state.written != Some(shown) && !TextInput::has_focus(id) {
                TextInput::set_text(id, &format!("{shown:.places$}"));
                state.written = Some(shown);
            }
            shown
        });

        let height = style::control_height();
        let radius = style::field_radius();
        let fg = style::label_primary();
        let column_w = px(20.0);

        let submit = on_change.clone();
        let input = TextInput::new(id)
            .size(style::control_font().size)
            .color(fg)
            .selection_color(crate::color::with_alpha(style::tint(), 0.4))
            .on_submit(move |text| {
                let next = text.trim().parse::<f64>().map_or(value, fix);
                // the field shows the clamped value even if it didn't change
                TextInput::set_text(id, &format!("{next:.places$}"));
                if next != value {
                    submit(next);
                }
            });

        let field = Rectangle::new()
            .width(Size::Fixed(width - column_w - 1.0))
            .height(Size::Fixed(height))
            .padding(crate::layout::pad_x(px(8.0), px(4.0)))
            .align_child(amane::Start, Center)
            .clip()
            .child(input);

        let half = |dir: f64, glyph: Symbol| {
            let half_id = format!("{id}.{}", if dir > 0.0 { "up" } else { "down" });
            let base = Rectangle::new()
                .width(Size::Fixed(column_w))
                .height(Size::Fixed(height / 2.0))
                .fill(hover_fill(&half_id, Color::TRANSPARENT, fg, disabled))
                .align_child(Center, Center)
                .child(symbol(glyph, px(10.0), fg));
            if disabled {
                return base;
            }
            let (pressed, released) = (key.clone(), key.clone());
            let commit = on_change.clone();
            let hovered = half_id.clone();
            base.cursor(Pointer)
                .on_hover(move |inside| hover(&hovered, inside))
                .on_drag(move |_: Point| {
                    with_state(&pressed, State::default, |state: &mut State| {
                        if state.hold.is_none() {
                            state.hold = Some(Hold {
                                dir,
                                since: Instant::now(),
                                from: value,
                            });
                        }
                    });
                    request_frame();
                })
                .on_release(move |_: Point| {
                    let hold = with_state(&released, State::default, |state: &mut State| {
                        state.hold.take()
                    });
                    if let Some(hold) = hold {
                        let steps = f64::from(steps_after(hold.since.elapsed().as_millis()));
                        let next = fix(hold.from + hold.dir * steps * step);
                        if next != hold.from {
                            commit(next);
                        }
                    }
                })
        };

        let column = Column::new(vec![
            boxed(half(1.0, Symbol::ChevronUp)),
            boxed(half(-1.0, Symbol::ChevronDown)),
        ]);

        let line = Rectangle::new()
            .width(Size::Fixed(1.0))
            .height(Size::Fixed(height - px(6.0)))
            .fill(style::field_border());

        let base = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height))
            .radius(radius)
            .fill(style::field_fill())
            .border(1.0, style::field_border())
            .clip()
            .child(Row::new(vec![boxed(field), boxed(line), boxed(column)]).align(Center));

        if disabled {
            return base.opacity(DISABLED_OPACITY);
        }
        let wheel = on_change;
        base.on_scroll(move |s: Scroll| {
            let next = if s.y < 0.0 {
                fix(shown + step)
            } else if s.y > 0.0 {
                fix(shown - step)
            } else {
                return;
            };
            if next != shown {
                wheel(next);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hold_speeds_up_to_every_50_ms() {
        assert_eq!(steps_after(0), 1);
        assert_eq!(steps_after(399), 1);
        assert_eq!(steps_after(400), 2);
        // 400, then 350, 300, 250, 200, 150, 100 and 50 apart
        assert_eq!(steps_after(749), 2);
        assert_eq!(steps_after(750), 3);
        assert_eq!(steps_after(1800), 9);
        assert_eq!(steps_after(1850), 10);
        assert_eq!(steps_after(1900), 11);
    }

    #[test]
    fn decimals_follow_the_step() {
        assert_eq!(decimals(1.0), 0);
        assert_eq!(decimals(0.5), 1);
        assert_eq!(decimals(0.1), 1);
        assert_eq!(decimals(0.05), 2);
    }
}
