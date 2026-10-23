//! Reusable buttons, drawn from `sys.*` tokens only so any component can use
//! them. Each follows its Material 3 counterpart in structure (variants,
//! shapes, state layers) but is drawn in the shell's macOS-style look:
//!
//! - [`PushButton`] — a label, optionally led by an icon (M3's buttons).
//! - [`IconButton`] — a lone glyph, optionally a toggle (M3's icon buttons).
//! - [`Switch`] — an iOS-style track and knob for an on/off setting (M3's
//!   switch).
//! - [`SegmentedButton`] — a row of 2–5 related options on one track, single-
//!   or multi-select (M3's segmented buttons).
//!
//! The push and icon buttons are one [`ButtonBuilder`] with a different
//! kind: they share a [`Style`] and a [`Shape`], and show a
//! `sys.color.state.hover` layer under the pointer. amane rebuilds the view
//! every frame, so that hover, and the switch's and segmented button's
//! motion, live here in tables keyed by each control's `id`. Ids must
//! therefore be unique and stable, like a `ScrollArea`'s.

// The public API (styles, shapes, disabled, …) is broader than the bar uses
// yet, so dead_code is allowed module-wide; unused_imports is not.
#![allow(dead_code)]

mod icon_button;
mod push_button;
mod segmented;
mod switch;

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use amane::{Button, Center, Color, Pointer, Radius, Rectangle, Row, Size, Text, Widget};

pub use icon_button::IconButton;
pub use push_button::PushButton;
pub use segmented::{Segment, SegmentedButton};
pub use switch::Switch;

use crate::components::color::over;
use crate::components::layout::{boxed, icon};
use crate::tokens::{Styled, TextStyle, sys};

/// M3's disabled-content opacity. TODO(token): `sys.opacity.disabled` once
/// Penpot has it (manual §1.1).
const DISABLED_OPACITY: f32 = 0.38;

/// How long a click's new state is shown before the service has to agree.
const PENDING_TIMEOUT: Duration = Duration::from_millis(1500);

/// How much a button stands out, after M3's filled, tonal and text buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The accent fill: the one main action of a surface ("Connect").
    Filled,
    /// A `surface.raised` fill, like the bar's pills: most buttons.
    Tonal,
    /// No fill until hovered: bar items, and M3's "standard" icon buttons.
    Plain,
}

impl Style {
    fn fill(self) -> Color {
        match self {
            Self::Filled => sys::color::accent::bg(),
            Self::Tonal => sys::color::surface::raised::bg(),
            Self::Plain => Color::TRANSPARENT,
        }
    }

    /// The content colour when none is given: `accent.fg` on the accent
    /// fill, else the panel's `fg`.
    fn fg(self) -> Color {
        match self {
            Self::Filled => sys::color::accent::fg(),
            _ => sys::color::surface::panel::fg(),
        }
    }
}

/// M3's two button shapes, in the shell's radii.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A capsule, or a circle when square.
    Round,
    /// The rounded rectangle chips and rows use (`sys.radius.raised`).
    Square,
}

impl Shape {
    fn radius(self) -> Radius {
        match self {
            Self::Round => Radius::Full,
            Self::Square => Radius::Fixed(sys::radius::raised()),
        }
    }
}

/// A push or icon button: what both have, and its `kind`'s own parts.
/// `id` keys its hover state, so it must be unique and stable.
pub struct ButtonBuilder<K> {
    id: String,
    style: Style,
    shape: Shape,
    color: Option<Color>,
    height: Size,
    disabled: bool,
    actions: Actions,
    kind: K,
}

impl<K> ButtonBuilder<K> {
    fn with(id: impl Into<String>, style: Style, shape: Shape, kind: K) -> Self {
        Self {
            id: id.into(),
            style,
            shape,
            color: None,
            height: Size::Fixed(sys::size::control::height()),
            disabled: false,
            actions: Actions::default(),
            kind,
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    /// The content colour. Defaults to `accent.fg` on a filled button and
    /// the panel's `fg` otherwise, so pass the `fg` of any other surface the
    /// button sits on.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn height(mut self, height: impl Into<Size>) -> Self {
        self.height = height.into();
        self
    }

    /// Dimmed and unclickable.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// What a left click does.
    pub fn on_press(mut self, press: impl Fn() + 'static) -> Self {
        self.actions.press = Some(Rc::new(press));
        self
    }

    /// What a right click does, such as opening a menu.
    pub fn on_secondary(mut self, secondary: impl Fn() + 'static) -> Self {
        self.actions.secondary = Some(Rc::new(secondary));
        self
    }

    /// The content colour this button draws in.
    fn fg(&self) -> Color {
        self.color.unwrap_or(self.style.fg())
    }

    /// The button around `content`, `width` wide.
    fn finish(self, width: f32, content: impl Widget + 'static) -> Rectangle {
        let base = Rectangle::new()
            .width(Size::Fixed(width))
            .height(self.height)
            .radius(self.shape.radius())
            .fill(state_fill(&self.id, self.style.fill(), self.disabled))
            .align_child(Center, Center)
            .clip()
            .child(content);

        interactive(base, self.id, self.disabled, self.actions)
    }
}

/// An optional glyph then a label, the content of push buttons and segments.
fn label_row(
    glyph: Option<char>,
    glyph_size: f32,
    label: String,
    style: TextStyle,
    color: Color,
) -> Row {
    let mut content: Vec<Box<dyn Widget>> = Vec::new();
    if let Some(glyph) = glyph {
        content.push(boxed(icon(glyph, glyph_size, color)));
    }
    // not elided, which would stretch it and lose the centering;
    // the control clips a label too long for it
    content.push(boxed(Text::new(label).style(style).color(color)));
    Row::new(content).gap(sys::space::gap::xs()).align(Center)
}

/// A click's new state, shown before the service agrees, until it does or
/// [`PENDING_TIMEOUT`] passes.
#[derive(Clone, Copy)]
struct Pending<T>(Option<(T, Instant)>);

impl<T: Copy + PartialEq> Pending<T> {
    const NONE: Self = Self(None);

    /// What to show while the service says `actual`.
    fn shown(&mut self, actual: T, now: Instant) -> T {
        if let Some((wanted, at)) = self.0
            && (actual == wanted || now - at > PENDING_TIMEOUT)
        {
            self.0 = None;
        }
        self.0.map_or(actual, |(wanted, _)| wanted)
    }

    fn set(&mut self, wanted: T, now: Instant) {
        self.0 = Some((wanted, now));
    }
}

thread_local! {
    // the button under the pointer, which shows the hover layer
    static HOVERED: RefCell<Option<String>> = const { RefCell::new(None) };

    // each switch's and segmented button's motion, by id
    static STATES: RefCell<HashMap<String, Box<dyn Any>>> = RefCell::new(HashMap::new());
}

/// Runs `f` on control `id`'s state, made with `init` the first time (or
/// when the control changed what it keeps).
fn with_state<S: 'static, R>(id: &str, init: impl FnOnce() -> S, f: impl FnOnce(&mut S) -> R) -> R {
    STATES.with_borrow_mut(|states| {
        if !states.get(id).is_some_and(|state| state.is::<S>()) {
            states.insert(id.to_string(), Box::new(init()));
        }
        let state = states
            .get_mut(id)
            .and_then(|state| state.downcast_mut::<S>());
        f(state.expect("just inserted"))
    })
}

/// Runs `f` on control `id`'s state, if it has one yet.
fn with_existing<S: 'static, R>(id: &str, f: impl FnOnce(&mut S) -> R) -> Option<R> {
    STATES.with_borrow_mut(|states| states.get_mut(id)?.downcast_mut::<S>().map(f))
}

/// The pointer entered or left the button `id`. amane redraws after any
/// hover change, so this only records it.
pub(crate) fn hover(id: &str, inside: bool) {
    HOVERED.with_borrow_mut(|hovered| {
        if inside {
            *hovered = Some(id.to_string());
        } else if hovered.as_deref() == Some(id) {
            *hovered = None;
        }
    });
}

pub(crate) fn hovered(id: &str) -> bool {
    HOVERED.with_borrow(|hovered| hovered.as_deref() == Some(id))
}

/// `fill` with the hover layer laid over it while `id` is hovered.
pub(crate) fn state_fill(id: &str, fill: Color, disabled: bool) -> Color {
    if !disabled && hovered(id) {
        over(sys::color::state::hover(), fill)
    } else {
        fill
    }
}

/// What a click asks for: the left button presses, the right one is the
/// secondary action (usually a menu).
#[derive(Clone, Default)]
pub(crate) struct Actions {
    pub(crate) press: Option<Rc<dyn Fn()>>,
    pub(crate) secondary: Option<Rc<dyn Fn()>>,
}

/// Makes `base` a button: pointer cursor, hover tracking and the clicks.
/// A disabled one is dimmed and takes none of them.
pub(crate) fn interactive(
    base: Rectangle,
    id: String,
    disabled: bool,
    actions: Actions,
) -> Rectangle {
    if disabled {
        return base.opacity(DISABLED_OPACITY);
    }

    let Actions { press, secondary } = actions;
    base.cursor(Pointer)
        .on_hover(move |inside| hover(&id, inside))
        .on_click(move |button| {
            let action = match button {
                Button::Left => &press,
                Button::Right => &secondary,
                _ => return,
            };
            if let Some(action) = action {
                action();
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pending_click_shows_until_the_service_agrees() {
        let now = Instant::now();
        let mut pending = Pending::NONE;
        assert!(!pending.shown(false, now));

        pending.set(true, now);
        assert!(pending.shown(false, now));
        // the service agreed: nothing is pending any more
        assert!(pending.shown(true, now));
        assert!(!pending.shown(false, now));

        pending.set(true, now);
        assert!(!pending.shown(false, now + PENDING_TIMEOUT * 2));
    }

    #[test]
    fn leaving_another_button_keeps_the_hover() {
        hover("a", true);
        hover("b", true);
        hover("a", false);
        assert!(hovered("b"));

        hover("b", false);
        assert!(!hovered("b"));
    }
}
