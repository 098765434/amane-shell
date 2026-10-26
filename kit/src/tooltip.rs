//! Tooltip — a short label shown above a hovered control.
//!
//! Modelled on Material 3's plain tooltip: one line of text on a small
//! card, frosted and tinted like the bar's panels, shown after the pointer rests on its control for [`DELAY`]
//! and hidden when it leaves or clicks. It's an xdg-popup without a grab, so
//! it never takes input or focus, hung under the surface the control is on
//! and centred above the pointer, as macOS's are.
//!
//! Only one tooltip shows at a time, so there's one popup slot ([`popup`],
//! registered with `App::popup`). A hovered control [`offer`]s its text every
//! frame it's built, so a value that changes under the pointer stays current,
//! [`suppress`]es it on click (until the pointer leaves) and [`dismiss`]es it
//! on leave. Buttons do this for you (`.tooltip(…)`).

use std::cell::RefCell;
use std::time::{Duration, Instant};

use amane::{ConstraintAdjustment, Gravity, Popup, PopupAnchor, Text, request_frame};

use crate::card::Card;
use crate::layout::{blank, boxed, cursor, text_height, text_width};
use crate::material::{self, Level, Material};
use crate::tokens::{Styled, sys};

/// The tooltip's xdg-popup namespace.
pub const TOOLTIP_NS: &str = "amane.tooltip";

/// How long the pointer rests on a control before its tooltip shows.
/// TODO(token): `sys.motion.duration.tooltip` once Penpot has it.
const DELAY: Duration = Duration::from_millis(500);

/// The tooltip a control offered.
struct Shown {
    id: String,
    text: String,
    parent: &'static str,
    since: Instant,
}

#[derive(Default)]
struct State {
    shown: Option<Shown>,
    /// The control clicked since the pointer entered it, whose offers are
    /// ignored: a click opens something the tooltip shouldn't cover.
    suppressed: Option<String>,
    /// The id and size of the live popup. amane never resizes or moves an
    /// open popup, so a different one is closed for a frame and reopened.
    live: Option<(String, u32, u32)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Control `id`, on the surface `parent`, is hovered and would show `text`.
/// Call it every frame the control is built while hovered.
pub fn offer(id: &str, parent: &'static str, text: impl Into<String>) {
    let text = text.into();
    STATE.with_borrow_mut(|state| {
        if state.suppressed.as_deref() == Some(id) {
            return;
        }
        match &mut state.shown {
            Some(shown) if shown.id == id => {
                shown.text = text;
                shown.parent = parent;
            }
            shown => {
                *shown = Some(Shown {
                    id: id.to_string(),
                    text,
                    parent,
                    since: Instant::now(),
                });
                request_frame();
            }
        }
    });
}

/// The pointer left control `id`: hides its tooltip, if it's the one shown.
pub fn dismiss(id: &str) {
    STATE.with_borrow_mut(|state| {
        if state.suppressed.as_deref() == Some(id) {
            state.suppressed = None;
        }
        hide(state, id);
    });
}

/// Control `id` was clicked: hides its tooltip until the pointer leaves it.
pub fn suppress(id: &str) {
    STATE.with_borrow_mut(|state| {
        state.suppressed = Some(id.to_string());
        hide(state, id);
    });
}

fn hide(state: &mut State, id: &str) {
    if state.shown.as_ref().is_some_and(|shown| shown.id == id) {
        state.shown = None;
        request_frame();
    }
}

/// The tooltip popup. Registered with `App::popup`.
pub fn popup() -> Popup {
    let now = Instant::now();
    let wanted = STATE.with_borrow(|state| {
        let shown = state.shown.as_ref()?;
        if now - shown.since < DELAY {
            // nothing else wakes us when the delay is up
            request_frame();
            return None;
        }
        Some((shown.id.clone(), shown.text.clone(), shown.parent))
    });

    let Some((id, text, parent)) = wanted else {
        return hidden();
    };

    // TODO(token): `tooltip.*` text style and inset once Penpot has them:
    // these are the closest `sys` roles
    let style = sys::r#type::caption();
    let inset = sys::space::inset::s();
    let width = (text_width(&text, style) + inset * 2.0).ceil() as u32;
    let height = (text_height(style) + inset * 2.0).ceil() as u32;

    let reopen = STATE.with_borrow_mut(|state| {
        let geometry = (id, width, height);
        let changed = state.live.as_ref().is_some_and(|live| *live != geometry);
        state.live = if changed { None } else { Some(geometry) };
        changed
    });
    if reopen {
        request_frame();
        return hidden();
    }

    // frosted and tinted like the panels; niri blurs behind it from the
    // parent's `popups {}` rule, with the same corners
    let card = Card::new(vec![boxed(
        Text::new(text)
            .style(style)
            .color(material::label(Level::Primary)),
    )])
    .material(Material::ToolTip)
    .glass(true)
    .padding(inset)
    .build();

    Popup::new(TOOLTIP_NS)
        .size(width, height)
        .anchor_rect(cursor().0, 0, 1, 1)
        .vibrant(material::vibrant())
        .parent(parent)
        .anchor(PopupAnchor::Top)
        .gravity(Gravity::Top)
        .offset(0, -(sys::space::gap::xs() as i32))
        .constraint_adjustment(ConstraintAdjustment::SLIDE_X | ConstraintAdjustment::FLIP_Y)
        .child(card)
}

fn hidden() -> Popup {
    STATE.with_borrow_mut(|state| state.live = None);
    Popup::new(TOOLTIP_NS)
        .size(1, 1)
        .anchor_rect(0, 0, 1, 1)
        // a hidden popup is never opened, so it needs no real parent
        .parent("")
        .visible(false)
        .child(blank())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown() -> Option<(String, String, Instant)> {
        STATE.with_borrow(|state| {
            let shown = state.shown.as_ref()?;
            Some((shown.id.clone(), shown.text.clone(), shown.since))
        })
    }

    #[test]
    fn offering_again_keeps_the_delay_and_updates_the_text() {
        offer("a", "amane.bar", "Volume 40%");
        let (_, _, since) = shown().unwrap();

        offer("a", "amane.bar", "Volume 45%");
        let (id, text, again) = shown().unwrap();
        assert_eq!((id.as_str(), text.as_str()), ("a", "Volume 45%"));
        assert_eq!(since, again);

        offer("b", "amane.bar", "Battery 80%");
        let (id, _, other) = shown().unwrap();
        assert_eq!(id, "b");
        assert!(other >= since);
    }

    #[test]
    fn dismissing_another_control_keeps_the_tooltip() {
        offer("a", "amane.bar", "Volume 40%");
        dismiss("b");
        assert!(shown().is_some());

        dismiss("a");
        assert!(shown().is_none());
    }

    #[test]
    fn a_click_hides_the_tooltip_until_the_pointer_leaves() {
        offer("a", "amane.bar", "Volume 40%");
        suppress("a");
        assert!(shown().is_none());

        offer("a", "amane.bar", "Volume 40%");
        assert!(shown().is_none());

        dismiss("a");
        offer("a", "amane.bar", "Volume 40%");
        assert!(shown().is_some());
    }
}
