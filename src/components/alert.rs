//! The shell's alert: a [`Dialog`] in the middle of the screen, on a
//! surface of its own in thick glass, as macOS asks before it ends the
//! session. One shows at a time, and it holds the keyboard until answered:
//! Return answers with its default button, Escape with its cancel button.
//!
//! The surface is only as big as the card, which frosts itself with one
//! blur shape (like a toast); its layer rule rounds it and gives it niri's
//! shadow.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use amane::{Animation, Blend, Horizontal, Keyboard, LayerWindow, Rectangle, Vertical};

use crate::components::dialog::Dialog;
use crate::components::glass_region;
use crate::components::layout::blank;
use crate::components::material::Material;
use crate::components::motion;
use crate::components::panel::overlay;
use crate::tokens::dialog;

amane_kit::marker! {
    /// The alert showed or closed: only its window draws again.
    pub Alert
}

/// Layer-surface namespace; the niri `layer-rule` matches it.
pub const ALERT_NS: &str = "amane.alert";

/// How small the card swells in from, as the launcher's glass does.
const OPEN_SCALE: f32 = 0.96;

thread_local! {
    // the alert shown, kept while it fades out
    static SHOWN: RefCell<Option<Dialog>> = const { RefCell::new(None) };

    // whether it's open, and how far it has swelled in
    static OPEN: Cell<bool> = const { Cell::new(false) };
    static UNFOLD: RefCell<Animation> = RefCell::new(motion::pop(0.0));
    static OPENING: Cell<bool> = const { Cell::new(false) };
}

/// Shows `alert`, in place of any alert already showing.
pub fn show(alert: Dialog) {
    SHOWN.set(Some(alert));
    OPEN.set(true);
    Alert::changed();
}

/// Whether the alert showing is the one named `name`.
pub fn showing(name: &str) -> bool {
    OPEN.get() && SHOWN.with_borrow(|shown| shown.as_ref().is_some_and(|a| a.name() == name))
}

/// Puts the alert away without an answer.
pub fn close() {
    OPEN.set(false);
    Alert::changed();
}

/// Steps the card's opening towards `open` and gives how far it is, a
/// little past 1 on the spring. Closing is quicker than opening.
fn unfold(open: bool) -> f32 {
    UNFOLD.with_borrow_mut(|anim| {
        if OPENING.replace(open) != open {
            anim.retime(if open { motion::POP } else { motion::CLOSE });
            anim.to(if open { 1.0 } else { 0.0 });
        }
        anim.value()
    })
}

/// The surface: hidden while no alert shows, else the card, swelling in.
pub fn surface() -> LayerWindow {
    // a device asking to pair asks here; reading Bluetooth for it wakes this view
    crate::components::bluetooth_prompt::sync();
    Alert::watch();
    let open = OPEN.get();
    let unfolded = unfold(open);

    let shown = SHOWN.with_borrow(Clone::clone);
    let Some(alert) = shown.filter(|_| open || unfolded > 0.001) else {
        if !open {
            SHOWN.set(None);
        }
        return window(1.0)
            .visible(false)
            .blur_shapes(Vec::new())
            .child(blank());
    };

    let width = Dialog::width();
    let height = alert.height();
    let radius = dialog::radius();

    // a compositor that can't fade frost only shows or hides it, so there
    // the card arrives whole rather than fading in over frost at full strength
    let fades = amane::blur_fades();
    let scale = f32::blend(motion::scale(OPEN_SCALE), 1.0, unfolded);
    let opacity = if open && !fades {
        1.0
    } else {
        unfolded.clamp(0.0, 1.0)
    };

    let close: Rc<dyn Fn()> = Rc::new(close);
    let card = Rectangle::new()
        .width(width)
        .height(height)
        .radius(radius)
        .fill(Material::Sheet.fill())
        .child(alert.content(close));
    let card = Material::Sheet
        .lit(card, radius)
        .opacity(opacity)
        .scale(scale);

    // the frost shrinks with the card, about the surface's centre
    let frost = if open || fades {
        let (shown_w, shown_h) = (width * scale, height * scale);
        vec![
            Material::Sheet
                .shape(
                    (width - shown_w) / 2.0,
                    (height - shown_h) / 2.0,
                    shown_w,
                    shown_h,
                    radius * scale,
                    opacity,
                )
                .id(glass_region::shape_id(ALERT_NS)),
        ]
    } else {
        Vec::new()
    };

    window(height).visible(true).blur_shapes(frost).child(card)
}

/// The surface, as big as a card `height` tall, in the middle of the output,
/// taking every key while it shows.
fn window(height: f32) -> LayerWindow {
    let base = LayerWindow::new()
        .width(Dialog::width())
        .height(height)
        .keyboard(Keyboard::Exclusive)
        .on_key(|key| {
            // the answer may show another alert, so nothing stays borrowed
            let shown = SHOWN.with_borrow(Clone::clone);
            if let Some(alert) = shown.filter(|_| OPEN.get()) {
                let close: Rc<dyn Fn()> = Rc::new(close);
                alert.key(key, &close);
            }
        });
    overlay(base, ALERT_NS, Vertical::Middle, Horizontal::Middle)
}
