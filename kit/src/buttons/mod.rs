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
//! - [`DrawnButton`] — content of its own, drawn in the button's colour.
//! - [`Checkbox`] — an on/off choice among independent ones (M3's checkbox).
//! - [`Radio`] — one choice out of a few, all in sight (M3's radio button).
//!
//! The push, icon and drawn buttons are one [`ButtonBuilder`] with a different
//! kind: they share a [`Style`] and a [`Shape`], and show a
//! `sys.color.state.hover` layer under the pointer. amane rebuilds the view
//! every frame, so that hover, and the switch's and segmented button's
//! motion, live here in tables keyed by each control's `id`. Ids must
//! therefore be unique and stable, like a `ScrollArea`'s.

// The public API (styles, shapes, disabled, …) is broader than the bar uses
// yet, so dead_code is allowed module-wide; unused_imports is not.
#![allow(dead_code)]

mod checkbox;
mod drawn_button;
mod icon_button;
mod push_button;
mod radio;
mod segmented;
mod switch;

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use amane::{
    Animation, Blend, Button, Center, Color, Point, Pointer, Radius, Rectangle, Row, Scroll, Size,
    Text, Widget, request_frame,
};

pub use checkbox::Checkbox;
pub use drawn_button::DrawnButton;
pub use icon_button::IconButton;
pub use push_button::PushButton;
pub use radio::Radio;
pub use segmented::{Segment, SegmentedButton};
pub use switch::Switch;

use crate::color::over;
use crate::layout::{boxed, icon};
use crate::{motion, tooltip};
use crate::tokens::{Styled, TextStyle, sys};

/// M3's disabled-content opacity. TODO(token): `sys.opacity.disabled` once
/// Penpot has it (manual §1.1).
const DISABLED_OPACITY: f32 = 0.38;

/// How long a click's new state is shown before the service has to agree.
const PENDING_TIMEOUT: Duration = Duration::from_millis(1500);

/// How far a pressed button shrinks before springing back. TODO(token):
/// `sys.motion.press-scale` once Penpot has it (manual §1.1); Quick
/// Settings' `quick.press.scale` holds the same value for its tiles.
const PRESS_SCALE: f32 = 0.96;

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
    /// The surface it's on and what its tooltip says.
    tooltip: Option<(&'static str, String)>,
    /// The fill and content colours it takes while hovered or `active`.
    highlight: Option<(Color, Color)>,
    active: bool,
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
            tooltip: None,
            highlight: None,
            active: false,
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

    /// What scrolling over it does, such as changing a level.
    pub fn on_scroll(mut self, scroll: impl Fn(Scroll) + 'static) -> Self {
        self.actions.scroll = Some(Rc::new(scroll));
        self
    }

    /// A [`tooltip`] with `text` while hovered. `parent` is the namespace of
    /// the surface the button is on, which the tooltip hangs under.
    pub fn tooltip(mut self, parent: &'static str, text: impl Into<String>) -> Self {
        self.tooltip = Some((parent, text.into()));
        self
    }

    /// While hovered or [`active`](Self::active), `bg` fills it and its
    /// content turns `fg`, in place of the hover layer: the status bar's
    /// items.
    pub fn highlight(mut self, bg: Color, fg: Color) -> Self {
        self.highlight = Some((bg, fg));
        self
    }

    /// Held highlighted, such as while what it opens is open.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// What a middle click does, such as muting.
    pub fn on_middle(mut self, middle: impl Fn() + 'static) -> Self {
        self.actions.middle = Some(Rc::new(middle));
        self
    }

    /// The content colour this button draws in.
    fn fg(&self) -> Color {
        let fg = self.color.unwrap_or(self.style.fg());
        match self.highlight {
            Some((_, on)) => Color::blend(fg, on, self.highlighted()),
            None => fg,
        }
    }

    /// The fill under the content: the highlight's, or the hover layer.
    fn fill(&self) -> Color {
        let fill = self.style.fill();
        match self.highlight {
            Some((bg, _)) => Color::blend(fill, bg, self.highlighted()),
            None => state_fill(&self.id, fill, self.disabled),
        }
    }

    /// How far the highlight has faded in, 0 to 1.
    fn highlighted(&self) -> f32 {
        let on = !self.disabled && (self.active || hovered(&self.id));
        fade(&format!("{}.highlight", self.id), on)
    }

    /// The button around `content`, `width` wide.
    fn finish(mut self, width: f32, content: impl Widget + 'static) -> Rectangle {
        if let Some((parent, text)) = self.tooltip.take()
            && !self.disabled
            && hovered(&self.id)
        {
            tooltip::offer(&self.id, parent, text);
        }

        let base = Rectangle::new()
            .width(Size::Fixed(width))
            .height(self.height)
            .radius(self.shape.radius())
            .fill(self.fill())
            .scale(press_scale(&self.id, PRESS_SCALE))
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
pub struct Pending<T>(Option<(T, Instant)>);

impl<T: Copy + PartialEq> Pending<T> {
    pub const NONE: Self = Self(None);

    /// What to show while the service says `actual`.
    pub fn shown(&mut self, actual: T, now: Instant) -> T {
        if let Some((wanted, at)) = self.0
            && (actual == wanted || now - at > PENDING_TIMEOUT)
        {
            self.0 = None;
        }
        self.0.map_or(actual, |(wanted, _)| wanted)
    }

    pub fn set(&mut self, wanted: T, now: Instant) {
        self.0 = Some((wanted, now));
    }
}

/// What a toggle `id` should show while the service says `actual`: the
/// state a click asked for ([`expect`]), until the service agrees or
/// gives up waiting.
pub fn optimistic(id: &str, actual: bool) -> bool {
    with_state(
        &format!("{id}.pending"),
        || Pending::<bool>::NONE,
        |pending: &mut Pending<bool>| pending.shown(actual, Instant::now()),
    )
}

/// A click on toggle `id` asked for `wanted`: show it right away.
pub fn expect(id: &str, wanted: bool) {
    with_state(
        &format!("{id}.pending"),
        || Pending::<bool>::NONE,
        |pending: &mut Pending<bool>| pending.set(wanted, Instant::now()),
    );
    request_frame();
}

thread_local! {
    // the buttons under the pointer, which show the hover layer: more than
    // one when a control sits inside another (a tile's disc in its tile)
    static HOVERED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };

    // each switch's and segmented button's motion, by id
    static STATES: RefCell<HashMap<String, Box<dyn Any>>> = RefCell::new(HashMap::new());
}

/// Runs `f` on control `id`'s state, made with `init` the first time (or
/// when the control changed what it keeps).
pub fn with_state<S: 'static, R>(
    id: &str,
    init: impl FnOnce() -> S,
    f: impl FnOnce(&mut S) -> R,
) -> R {
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
pub fn with_existing<S: 'static, R>(id: &str, f: impl FnOnce(&mut S) -> R) -> Option<R> {
    STATES.with_borrow_mut(|states| states.get_mut(id)?.downcast_mut::<S>().map(f))
}

/// The pointer entered or left the button `id`. amane redraws after any
/// hover change, so this only records it.
pub fn hover(id: &str, inside: bool) {
    HOVERED.with_borrow_mut(|hovered| {
        if inside {
            if !hovered.iter().any(|h| h == id) {
                hovered.push(id.to_string());
            }
        } else {
            hovered.retain(|h| h != id);
        }
    });
}

pub fn hovered(id: &str) -> bool {
    HOVERED.with_borrow(|hovered| hovered.iter().any(|h| h == id))
}

/// `fill` with the hover layer fading over it while `id` is hovered.
pub fn state_fill(id: &str, fill: Color, disabled: bool) -> Color {
    let amount = fade(&format!("{id}.hover"), !disabled && hovered(id));
    Color::blend(fill, over(sys::color::state::hover(), fill), amount)
}

/// The fade keyed `key`, 1 while `on` and 0 while not, at hover speed.
/// It starts where it is, so a control that appears under the pointer
/// doesn't fade in its hover.
fn fade(key: &str, on: bool) -> f32 {
    let target = if on { 1.0 } else { 0.0 };
    with_state(
        key,
        || motion::fade(target, motion::HOVER),
        |fade: &mut Animation| {
            fade.to(target);
            fade.value().clamp(0.0, 1.0)
        },
    )
}

/// A press's pulse: 0 the moment the button is pressed, 1 once it has
/// swelled back. A button that was never pressed is at rest.
struct Press(Animation);

/// The scale to draw control `id` at, shrinking to `scale` on a press and
/// springing back. amane reports a press as the start of a drag, not its
/// release, so the pulse is timed rather than held.
pub fn press_scale(id: &str, scale: f32) -> f32 {
    let progress = with_state(
        &format!("{id}.press"),
        || Press(motion::spring(1.0)),
        |press: &mut Press| press.0.value().clamp(0.0, 1.0),
    );
    f32::blend(scale, 1.0, progress)
}

/// Control `id` was pressed: its pulse starts over.
fn pressed(id: &str) {
    with_state(
        &format!("{id}.press"),
        || Press(motion::spring(1.0)),
        |press: &mut Press| {
            let mut pulse = Animation::new(0.0)
                .duration(motion::PRESS)
                .easing(amane::Easing::Out);
            pulse.to(1.0);
            press.0 = pulse;
        },
    );
    request_frame();
}

/// What a click asks for: the left button presses, the right one is the
/// secondary action (usually a menu), the middle one a shortcut. Scrolling
/// is optional too.
#[derive(Clone, Default)]
pub struct Actions {
    pub press: Option<Rc<dyn Fn()>>,
    pub secondary: Option<Rc<dyn Fn()>>,
    pub middle: Option<Rc<dyn Fn()>>,
    pub scroll: Option<Rc<dyn Fn(Scroll)>>,
}

/// Makes `base` a button: pointer cursor, hover tracking and the clicks.
/// Leaving or clicking it hides its tooltip. A disabled one is dimmed and
/// takes none of them.
pub fn interactive(
    base: Rectangle,
    id: String,
    disabled: bool,
    actions: Actions,
) -> Rectangle {
    if disabled {
        return base.opacity(DISABLED_OPACITY);
    }

    let Actions {
        press,
        secondary,
        middle,
        scroll,
    } = actions;
    let clicked = id.clone();
    let pressed_id = id.clone();
    let base = base
        .cursor(Pointer)
        // a left press starts a drag, which is the only press amane reports
        .on_drag(move |_: Point| pressed(&pressed_id))
        .on_hover(move |inside| {
            hover(&id, inside);
            if !inside {
                tooltip::dismiss(&id);
            }
        })
        .on_click(move |button| {
            let action = match button {
                Button::Left => &press,
                Button::Right => &secondary,
                Button::Middle => &middle,
                // side buttons, once amane reports them
                #[allow(unreachable_patterns)]
                _ => return,
            };
            tooltip::suppress(&clicked);
            if let Some(action) = action {
                action();
            }
        });

    match scroll {
        Some(scroll) => base.on_scroll(move |delta| scroll(delta)),
        None => base,
    }
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
