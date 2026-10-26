//! Slider — picks a level from 0 to 100 along a track.
//!
//! Modelled on Material 3's slider: an active track filled up to the value
//! over an inactive one, set by pressing or dragging anywhere along it, and
//! discrete [`STEP`]s for the scroll wheel. Drawn in the macOS look:
//! a thin capsule track with no handle, the accent filling it; a hit area a
//! full control tall, so the thin track is easy to grab.
//!
//! [`Slider::large`] is the macOS Control Center slider instead: a track a
//! full control tall with the glyph inside it, at the start of a fill that
//! never gets narrower than the glyph's circle, so the glyph is the knob.
//! The glyph can mute (`on_icon_press`).

use std::rc::Rc;

use amane::{
    Center, Color, Point, Pointer, Radius, Rectangle, Row, Scroll, Size, Stack, Start, Widget,
};

use crate::buttons::{Actions, interactive, state_fill};
use crate::control_size;
use crate::layout::{boxed, icon, slot};
use crate::material::{self, FillLevel, Level};
use crate::tokens::sys;

/// How far one scroll notch moves a level, the bar's volume button included.
pub const STEP: u8 = 5;

/// `value` moved one `step` by a scroll: up (negative y) raises, down lowers.
pub fn scrolled(value: u8, scroll: Scroll, step: u8) -> u8 {
    if scroll.y < 0.0 {
        value.saturating_add(step).min(100)
    } else if scroll.y > 0.0 {
        value.saturating_sub(step)
    } else {
        // a sideways scroll leaves the level alone
        value
    }
}

pub struct Slider {
    value: u8,
    width: f32,
    dimmed: bool,
    // drawn but taking no input, as in Quick Settings' edit mode
    inert: bool,
    on_value_change: Rc<dyn Fn(u8)>,
    // the large variant's glyph
    large: Option<char>,
    // the thin variant's active colour, in place of the accent
    fill: Option<Color>,
    // the thin variant's track height, in place of `sys.size.slider.height`
    thickness: Option<f32>,
    // the glyph's id and what pressing it does
    icon_press: Option<(String, Rc<dyn Fn()>)>,
    // whether the wheel moves it; off in a scrolled list, where the wheel scrolls the list
    wheel: bool,
}

impl Slider {
    pub fn new(value: u8, width: f32) -> Self {
        Self {
            value: value.min(100),
            width,
            dimmed: false,
            inert: false,
            on_value_change: Rc::new(|_| {}),
            large: None,
            fill: None,
            thickness: None,
            icon_press: None,
            wheel: true,
        }
    }

    /// Greys the active track, as while muted; it still takes input.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    /// Draws it without any handler or cursor, for a surface whose
    /// controls are shown but not for use.
    pub fn inert(mut self, inert: bool) -> Self {
        self.inert = inert;
        self
    }

    /// Whether the scroll wheel moves it (on by default). Turn it off in a
    /// list that scrolls, or scrolling past it changes its value.
    pub fn wheel(mut self, wheel: bool) -> Self {
        self.wheel = wheel;
        self
    }

    pub fn on_value_change(mut self, change: impl Fn(u8) + 'static) -> Self {
        self.on_value_change = Rc::new(change);
        self
    }

    /// The Control Center look: a control-tall track with `glyph` as its knob.
    pub fn large(mut self, glyph: char) -> Self {
        self.large = Some(glyph);
        self
    }

    /// The thin slider's active colour in place of the accent. Quick
    /// Settings' glass cards use the panel's `fg`, a documented departure
    /// from the one-accent rule.
    pub fn fill(mut self, color: Color) -> Self {
        self.fill = Some(color);
        self
    }

    /// The thin slider's track height in place of `sys.size.slider.height`.
    pub fn thickness(mut self, height: f32) -> Self {
        self.thickness = Some(height);
        self
    }

    /// Makes the large slider's glyph a button (mute, say). `id` keys its
    /// hover, so it must be unique and stable. (Quick Settings' Sound card
    /// mutes with a button before its thin track now; the Sound page's
    /// microphone slider mutes with its glyph.)
    pub fn on_icon_press(mut self, id: impl Into<String>, press: impl Fn() + 'static) -> Self {
        self.icon_press = Some((id.into(), Rc::new(press)));
        self
    }

    pub fn build(self) -> Rectangle {
        if let Some(glyph) = self.large {
            return self.build_large(glyph);
        }

        let Self {
            value,
            width,
            dimmed,
            inert,
            on_value_change: set,
            fill,
            thickness,
            wheel,
            ..
        } = self;
        let set = changes_only(value, set);
        let track_h = thickness.unwrap_or_else(sys::size::slider::height);
        let active = if dimmed {
            material::label(Level::Secondary)
        } else {
            fill.unwrap_or_else(sys::color::accent::bg)
        };

        let track = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(track_h))
            .radius(track_h / 2.0)
            .fill(material::fill_role(FillLevel::Secondary))
            .border(
                sys::border::hairline(),
                sys::color::progress::track_stroke(),
            )
            .clip()
            .child(
                Rectangle::new()
                    .width(Size::Fixed(width * f32::from(value) / 100.0))
                    .height(Size::Fixed(track_h))
                    .radius(track_h / 2.0)
                    .fill(active),
            );

        // the pill knob rides the track's end, inside its width; an inert
        // slider is a meter and has none
        let height = sys::size::control::height();
        let (knob_w, knob_h) = control_size::current().slider_knob();
        let knob = Rectangle::new()
            .width(Size::Fixed(knob_w))
            .height(Size::Fixed(knob_h))
            .radius(Radius::Full)
            .fill(sys::color::accent::fg())
            .border(sys::border::hairline(), material::separator())
            .translate(
                (width - knob_w).max(0.0) * f32::from(value) / 100.0,
                (height - knob_h) / 2.0,
            );

        let scroll = set.clone();
        let input = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height))
            .fill(Color::TRANSPARENT)
            .align_child(Start, Center);
        if inert {
            return input.child(track);
        }
        let track = Stack::new(vec![
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(width))
                    .height(Size::Fixed(height))
                    .align_child(Start, Center)
                    .child(track),
            ),
            boxed(knob),
        ]);
        let input = input
            .cursor(Pointer)
            .on_drag(move |p: Point| {
                // the drag keeps reporting past either end, so it is clamped
                let ratio = (p.x / width).clamp(0.0, 1.0);
                set((ratio * 100.0).round() as u8);
            })
            .child(track);
        if !wheel {
            return input;
        }
        input.on_scroll(move |s: Scroll| scroll(scrolled(value, s, STEP)))
    }

    fn build_large(self, glyph: char) -> Rectangle {
        let Self {
            value,
            width,
            dimmed,
            inert,
            on_value_change: set,
            icon_press,
            ..
        } = self;
        let set = changes_only(value, set);
        let height = sys::size::control::height();
        let track = material::fill_role(FillLevel::Secondary);
        let active = if dimmed {
            material::label(Level::Secondary)
        } else {
            sys::color::accent::bg()
        };

        let fill = Rectangle::new()
            .width(Size::Fixed(large_fill(value, width, height)))
            .height(Size::Fixed(height))
            .radius(Radius::Full)
            .fill(active);

        // the glyph's circle, and the rest of the track that the drag maps
        let knob = slot(height).radius(Radius::Full).child(icon(
            glyph,
            sys::size::icon::m(),
            sys::color::accent::fg(),
        ));
        let drag_from = if icon_press.is_some() { height } else { 0.0 };
        let knob = match icon_press {
            Some((id, press)) => {
                let fill = state_fill(&id, Color::TRANSPARENT, false);
                let actions = Actions {
                    press: Some(press),
                    ..Actions::default()
                };
                interactive(knob.fill(fill), id, inert, actions)
            }
            None => knob,
        };

        let drag_set = set.clone();
        let rest = Rectangle::new()
            .width(Size::Fixed((width - drag_from).max(1.0)))
            .height(Size::Fixed(height))
            .fill(Color::TRANSPARENT);
        let rest = if inert {
            rest
        } else {
            rest.cursor(Pointer).on_drag(move |p: Point| {
                drag_set(large_value(p.x + drag_from, width, height));
            })
        };

        // with no glyph button the whole track drags, from under the glyph
        let input: Box<dyn Widget> = if drag_from > 0.0 {
            boxed(Row::new(vec![boxed(knob), boxed(rest)]))
        } else {
            boxed(Stack::new(vec![boxed(knob), boxed(rest)]))
        };

        let slider = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height))
            .radius(Radius::Full)
            .fill(track)
            .clip()
            .child(Stack::new(vec![boxed(fill), input]));
        if inert {
            return slider;
        }
        slider.on_scroll(move |s: Scroll| set(scrolled(value, s, STEP)))
    }
}

/// `set`, passed only a value other than `value`, the one shown: a drag
/// reports every motion and a scroll every step past either end, and each
/// value sent is a round trip (a sink's volume, the backlight) and a redraw.
fn changes_only(value: u8, set: Rc<dyn Fn(u8)>) -> Rc<dyn Fn(u8)> {
    Rc::new(move |next| {
        if next != value {
            set(next);
        }
    })
}

/// How wide the large slider's fill is at `value`: never narrower than the
/// glyph's circle (`height`), the whole track at 100.
fn large_fill(value: u8, width: f32, height: f32) -> f32 {
    height + (width - height).max(0.0) * f32::from(value.min(100)) / 100.0
}

/// The level whose fill's centre-of-knob is under `x` along the track.
fn large_value(x: f32, width: f32, height: f32) -> u8 {
    let span = (width - height).max(1.0);
    let ratio = ((x - height / 2.0) / span).clamp(0.0, 1.0);
    (ratio * 100.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scroll(y: f32) -> Scroll {
        Scroll {
            y,
            ..Scroll::default()
        }
    }

    #[test]
    fn scrolling_steps_within_range() {
        assert_eq!(scrolled(50, scroll(-1.0), STEP), 55);
        assert_eq!(scrolled(50, scroll(1.0), STEP), 45);
        assert_eq!(scrolled(98, scroll(-1.0), STEP), 100);
        assert_eq!(scrolled(3, scroll(1.0), STEP), 0);
    }

    #[test]
    fn the_large_fill_never_gets_narrower_than_the_knob() {
        assert_eq!(large_fill(0, 300.0, 30.0), 30.0);
        assert_eq!(large_fill(50, 300.0, 30.0), 165.0);
        assert_eq!(large_fill(100, 300.0, 30.0), 300.0);
        // a track narrower than its knob is all knob
        assert_eq!(large_fill(100, 20.0, 30.0), 30.0);
    }

    #[test]
    fn the_large_knob_follows_the_pointer() {
        assert_eq!(large_value(15.0, 300.0, 30.0), 0);
        assert_eq!(large_value(150.0, 300.0, 30.0), 50);
        assert_eq!(large_value(285.0, 300.0, 30.0), 100);
        assert_eq!(large_value(-40.0, 300.0, 30.0), 0);
        assert_eq!(large_value(900.0, 300.0, 30.0), 100);
    }
}
