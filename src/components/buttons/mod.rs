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
//! The push and icon buttons share a [`Style`] and a [`Shape`], and show a
//! `sys.color.state.hover` layer under the pointer. amane rebuilds the view
//! every frame, so that hover lives here, keyed by each button's `id`.
//! Ids must therefore be unique and stable, like a `ScrollArea`'s.

// The public API (styles, shapes, disabled, …) is broader than the bar uses
// yet, so dead_code is allowed module-wide; unused_imports is not.
#![allow(dead_code)]

mod icon_button;
mod push_button;
mod segmented;
mod switch;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use amane::{Button, Color, Pointer, Radius, Rectangle};

pub use icon_button::IconButton;
pub use push_button::PushButton;
pub use segmented::{Segment, SegmentedButton};
pub use switch::Switch;

use crate::tokens::sys;

/// M3's disabled-content opacity; becomes `sys.state.disabled` once the
/// token exists in Penpot.
const DISABLED_OPACITY: f32 = 0.38;

// iOS-like motion; become `sys.motion.*` tokens once they exist in Penpot
const SLIDE: Duration = Duration::from_millis(450);
const SLIDE_BOUNCE: f32 = 0.15;
const FADE: Duration = Duration::from_millis(200);

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

thread_local! {
    // the button under the pointer, which shows the hover layer
    static HOVERED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The pointer entered or left the button `id`. amane redraws after any
/// hover change, so this only records it.
fn hover(id: &str, inside: bool) {
    HOVERED.with_borrow_mut(|hovered| {
        if inside {
            *hovered = Some(id.to_string());
        } else if hovered.as_deref() == Some(id) {
            *hovered = None;
        }
    });
}

fn hovered(id: &str) -> bool {
    HOVERED.with_borrow(|hovered| hovered.as_deref() == Some(id))
}

/// `fill` with the hover layer laid over it while `id` is hovered.
fn state_fill(id: &str, fill: Color, disabled: bool) -> Color {
    if !disabled && hovered(id) {
        over(sys::color::state::hover(), fill)
    } else {
        fill
    }
}

/// `top` painted over `bottom` (source-over), so a translucent state layer
/// can be one fill instead of another rectangle.
fn over(top: Color, bottom: Color) -> Color {
    let top_a = f32::from(top.alpha()) / 255.0;
    let bottom_a = f32::from(bottom.alpha()) / 255.0 * (1.0 - top_a);
    let alpha = top_a + bottom_a;
    if alpha <= 0.0 {
        return Color::TRANSPARENT;
    }

    let channel =
        |t: u8, b: u8| ((f32::from(t) * top_a + f32::from(b) * bottom_a) / alpha).round() as u8;
    Color::rgba(
        channel(top.red(), bottom.red()),
        channel(top.green(), bottom.green()),
        channel(top.blue(), bottom.blue()),
        (alpha * 255.0).round() as u8,
    )
}

/// `color` at `amount` of its own opacity.
fn faded(color: Color, amount: f32) -> Color {
    let alpha = (f32::from(color.alpha()) * amount.clamp(0.0, 1.0)).round() as u8;
    Color::rgba(color.red(), color.green(), color.blue(), alpha)
}

/// What a click asks for: the left button presses, the right one is the
/// secondary action (usually a menu).
#[derive(Clone, Default)]
struct Actions {
    press: Option<Rc<dyn Fn()>>,
    secondary: Option<Rc<dyn Fn()>>,
}

/// Makes `base` a button: pointer cursor, hover tracking and the clicks.
/// A disabled one is dimmed and takes none of them.
fn interactive(base: Rectangle, id: String, disabled: bool, actions: Actions) -> Rectangle {
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
    fn over_an_opaque_fill_stays_opaque() {
        let mixed = over(Color::rgba(255, 255, 255, 51), Color::rgb(0, 0, 0));
        assert_eq!(mixed, Color::rgba(51, 51, 51, 255));
    }

    #[test]
    fn over_transparent_is_the_top_layer() {
        let top = Color::rgba(255, 255, 255, 26);
        assert_eq!(over(top, Color::TRANSPARENT), top);
    }

    #[test]
    fn translucent_layers_add_up() {
        // 20% over 20% covers 36%
        let mixed = over(Color::rgba(0, 0, 0, 51), Color::rgba(0, 0, 0, 51));
        assert_eq!(mixed.alpha(), 92);
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
