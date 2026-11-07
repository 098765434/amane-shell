//! Reusable buttons, drawn from `sys.*` tokens only so any component can use
//! them:
//!
//! - [`PushButton`] — a label, optionally led by an icon.
//! - [`IconButton`] — a lone glyph, optionally a toggle.
//! - [`Switch`] — a capsule track and knob for an on/off setting.
//! - [`SegmentedButton`] — a row of 2–5 related options on one track, single-
//!   or multi-select.
//! - [`DrawnButton`] — content of its own, drawn in the button's colour.
//! - [`Checkbox`] — an on/off choice among independent ones.
//! - [`Radio`] — one choice out of a few, all in sight.
//!
//! The push, icon and drawn buttons are one [`ButtonBuilder`] with a different
//! kind: they share a [`Style`] and a [`Shape`], and show a
//! `sys.color.state.hover` layer under the pointer. amane rebuilds the view
//! every frame, so that hover, and the switch's and segmented button's
//! motion, live here in tables keyed by each control's `id`. Ids must
//! therefore be unique and stable, like a `ScrollArea`'s.

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
use crate::control_size;
use crate::layout::{boxed, icon};
use crate::material::{self, FillLevel, Level};
use crate::tokens::{Styled, TextStyle, sys};
use crate::{motion, tooltip};

/// How faint a disabled control is, every control alike. TODO(token):
/// `sys.opacity.disabled` once Penpot has it (manual §1.1).
pub const DISABLED_OPACITY: f32 = 0.4;

/// How long a click's new state is shown before the service has to agree.
const PENDING_TIMEOUT: Duration = Duration::from_millis(1500);

/// How far a pressed button shrinks before springing back. TODO(token):
/// `sys.motion.press-scale` once Penpot has it (manual §1.1); Quick
/// Settings' `quick.press.scale` holds the same value for its tiles.
const PRESS_SCALE: f32 = 0.96;

/// How much a button stands out: filled, tonal or text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The accent fill: the one main action of a surface ("Connect").
    Filled,
    /// A `surface.raised` fill, like the bar's pills: most buttons.
    Tonal,
    /// No fill until hovered: bar items, and plain icon buttons.
    Plain,
    /// A faint red fill under a red label: an action that destroys or
    /// can't be undone ("Delete", "Log Out"), as in the shell's alerts.
    Destructive,
}

impl Style {
    fn fill(self) -> Color {
        match self {
            Self::Filled => sys::color::accent::bg(),
            Self::Tonal => material::fill_role(FillLevel::Secondary),
            Self::Plain => Color::TRANSPARENT,
            Self::Destructive => sys::color::status::critical::hover(),
        }
    }

    /// The content colour when none is given: `accent.fg` on the accent
    /// fill, else the panel's `fg`.
    fn fg(self) -> Color {
        match self {
            Self::Filled => sys::color::accent::fg(),
            Self::Destructive => sys::color::status::critical::text(),
            _ => material::label(Level::Primary),
        }
    }
}

/// The two button shapes, in the shell's radii.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A capsule, or a circle when square.
    Round,
    /// The rounded rectangle of the current control size, a capsule from
    /// Large up.
    Square,
}

impl Shape {
    fn radius(self) -> Radius {
        match self {
            Self::Round => Radius::Full,
            Self::Square => control_size::current().button_radius(),
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

    /// How much it stands out: filled, tonal, plain or destructive.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Round (a capsule) or square (the control size's corners).
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

    /// Its height; `sys.size.control.height` by default.
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
        fade(&format!("{}.highlight", self.id), on, motion::HOVER)
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
            .width(width)
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
/// `PENDING_TIMEOUT` (1.5 seconds) passes.
#[derive(Clone, Copy)]
pub struct Pending<T>(Option<(T, Instant)>);

impl<T: Copy + PartialEq> Pending<T> {
    /// Nothing pending: show what the service says.
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

    /// A click asked for `wanted` at `now`: show it until the service agrees or the wait runs
    /// out.
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

// each control's state by id, with when it was last asked for
type States = HashMap<String, (Instant, Box<dyn Any>)>;

thread_local! {
    // the buttons under the pointer, which show the hover layer: more than
    // one when a control sits inside another (a tile's disc in its tile)
    static HOVERED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };

    // each switch's and segmented button's motion, by id, with when it was last asked for
    static STATES: RefCell<States> = RefCell::new(HashMap::new());

    // the button held down and when it last reported: the drag that
    // reports a press goes on reporting every motion after it
    static HELD: RefCell<Option<(String, Instant)>> = const { RefCell::new(None) };
}

/// Runs `f` on control `id`'s state, made with `init` the first time (or
/// when the control changed what it keeps).
pub fn with_state<S: 'static, R>(
    id: &str,
    init: impl FnOnce() -> S,
    f: impl FnOnce(&mut S) -> R,
) -> R {
    let now = Instant::now();
    STATES.with_borrow_mut(|states| {
        if !states.get(id).is_some_and(|(_, state)| state.is::<S>()) {
            prune(states, now);
            states.insert(id.to_string(), (now, Box::new(init())));
        }
        let (used, state) = states.get_mut(id).expect("just inserted");
        *used = now;
        f(state.downcast_mut::<S>().expect("just inserted"))
    })
}

/// Runs `f` on control `id`'s state, if it has one yet.
pub fn with_existing<S: 'static, R>(id: &str, f: impl FnOnce(&mut S) -> R) -> Option<R> {
    STATES.with_borrow_mut(|states| {
        let (used, state) = states.get_mut(id)?;
        *used = Instant::now();
        state.downcast_mut::<S>().map(f)
    })
}

// controls come and go with their ids (a list's rows, a page's switches): once there are many,
// the ones not asked for in a while are let go, and start fresh if they come back
fn prune(states: &mut States, now: Instant) {
    const MANY: usize = 512;
    const UNUSED: std::time::Duration = std::time::Duration::from_secs(5 * 60);
    if states.len() >= MANY {
        states.retain(|_, (used, _)| now.duration_since(*used) < UNUSED);
    }
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

/// Whether the pointer is over the button `id`, as [`hover`] last recorded.
pub fn hovered(id: &str) -> bool {
    HOVERED.with_borrow(|hovered| hovered.iter().any(|h| h == id))
}

/// Whether the button `id` is held down now: pressed and not yet let go.
/// A hold that hasn't reported for [`HOLD_LAPSE`] counts as let go, since a
/// release can go unseen.
pub fn held(id: &str) -> bool {
    HELD.with_borrow(|held| {
        held.as_ref()
            .is_some_and(|(was, at)| was == id && at.elapsed() < HOLD_LAPSE)
    })
}

/// `fill` with the hover layer fading over it while `id` is hovered.
pub fn state_fill(id: &str, fill: Color, disabled: bool) -> Color {
    hover_layer(id, fill, sys::color::state::hover(), disabled)
}

/// `fill` with `layer` fading over it while `id` is hovered: the one hover
/// every control shows, in the colour that suits it.
pub(crate) fn hover_layer(id: &str, fill: Color, layer: Color, disabled: bool) -> Color {
    let amount = fade(
        &format!("{id}.hover"),
        !disabled && hovered(id),
        motion::HOVER,
    );
    Color::blend(fill, over(layer, fill), amount)
}

/// The fade keyed `key`, 1 while `on` and 0 while not, over `duration`.
/// It starts where it is, so a control that appears under the pointer
/// doesn't fade in its hover.
pub(crate) fn fade(key: &str, on: bool, duration: Duration) -> f32 {
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

/// Control `id` was pressed: its pulse starts over, once a press, and not
/// at all where motion is reduced.
fn pressed(id: &str) {
    // a release can go unseen (the window closed under it), so a press
    // that comes a second after the last report is a new one
    let again = HELD.with_borrow_mut(|held| {
        let before = held.replace((id.to_string(), Instant::now()));
        before.is_some_and(|(was, at)| was == id && at.elapsed() < REPRESS)
    });
    if again || crate::theme::reduced_motion() {
        return;
    }
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

/// How long a hold lasts without a report from the pointer before it
/// counts as let go: a release can go unseen.
const HOLD_LAPSE: Duration = Duration::from_secs(3);

/// How long after the last report of a hold a press counts as a new one,
/// and pulses again.
const REPRESS: Duration = Duration::from_secs(1);

/// How soon after a drop its click comes, at most.
const SWALLOW_WINDOW: Duration = Duration::from_millis(250);

thread_local! {
    // when a drag was dropped, so the click that ends it does nothing
    static SWALLOW: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
}

/// The click that comes with this release does nothing: a drag that moved
/// something, like a status bar item, ends where it was dropped. amane
/// hands out the drop before the click.
pub fn swallow_click() {
    SWALLOW.set(Some(Instant::now()));
}

/// Whether the click now is the one a drop swallowed.
fn swallowed() -> bool {
    SWALLOW
        .take()
        .is_some_and(|at| at.elapsed() < SWALLOW_WINDOW)
}

/// What a click asks for: the left button presses, the right one is the
/// secondary action (usually a menu), the middle one a shortcut. Scrolling
/// is optional too.
#[derive(Clone, Default)]
pub struct Actions {
    /// The left button's click.
    pub press: Option<Rc<dyn Fn()>>,
    /// The right button's click, usually a menu.
    pub secondary: Option<Rc<dyn Fn()>>,
    /// The middle button's click, a shortcut.
    pub middle: Option<Rc<dyn Fn()>>,
    /// A wheel or touchpad scroll over it.
    pub scroll: Option<Rc<dyn Fn(Scroll)>>,
}

impl Actions {
    /// A left click doing `press`, and nothing else.
    pub fn press(press: impl Fn() + 'static) -> Self {
        Self {
            press: Some(Rc::new(press)),
            ..Self::default()
        }
    }

    /// A right click doing `secondary` too, usually a menu.
    #[must_use]
    pub fn secondary(mut self, secondary: impl Fn() + 'static) -> Self {
        self.secondary = Some(Rc::new(secondary));
        self
    }
}

/// Makes `base` a button: pointer cursor, hover tracking and the clicks.
/// Leaving or clicking it hides its tooltip. A disabled one is dimmed and
/// takes none of them.
pub fn interactive(base: Rectangle, id: String, disabled: bool, actions: Actions) -> Rectangle {
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
        .on_release(|_: Point| HELD.set(None))
        .on_hover(move |inside| {
            hover(&id, inside);
            if !inside {
                tooltip::dismiss(&id);
            }
        })
        .on_click(move |button| {
            if button == Button::Left && swallowed() {
                return;
            }
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
    fn radios_and_checkboxes_build_in_every_state() {
        for on in [false, true] {
            for disabled in [false, true] {
                let _ = Radio::new("test.radio", on)
                    .label("Option")
                    .disabled(disabled)
                    .build();
                let _ = Checkbox::new("test.checkbox", on)
                    .disabled(disabled)
                    .build();
            }
        }
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
