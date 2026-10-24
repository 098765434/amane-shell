//! Shared scaffolding for the bar's popover panels — Wi-Fi, Bluetooth, Sound
//! and power — and the overlay surfaces the notification center and toasts
//! share with them.
//!
//! Only one panel is open at a time: each is a frosted
//! [`Card`](crate::components::card::Card) on its own bottom-right
//! `LayerWindow` (`Keyboard::OnDemand`, so a panel's text field can take keys
//! while the bar never steals focus), toggled by its bar indicator and
//! dismissed with Escape or by toggling again. The notification center
//! shares the one-open-at-a-time state.

use std::cell::Cell;

use amane::{
    Horizontal, Key, Keyboard, Layer, LayerWindow, Margin, Rectangle, Vertical, Widget, Zone,
    mark_all,
};

use super::{bluetooth, wifi};
use crate::components::card::{Card, Surface};
use crate::components::layout::blank;
use crate::components::{context_menu, notifications};
use crate::tokens::sys;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Wifi,
    Bluetooth,
    Audio,
    Power,
    /// The notification center: not one of the bottom-right cards, but
    /// sharing their one-open-at-a-time state.
    Notifications,
}

thread_local! {
    static OPEN: Cell<Option<Panel>> = const { Cell::new(None) };
}

// ───── runtime ───────────────────────────────────────────────────────────────

/// Opens `panel`, closing whichever other one is open, or closes it if it is.
pub fn toggle(panel: Panel) {
    set(if is_open(panel) { None } else { Some(panel) });
}

pub fn close() {
    set(None);
}

pub fn is_open(panel: Panel) -> bool {
    OPEN.with(Cell::get) == Some(panel)
}

fn set(next: Option<Panel>) {
    let previous = OPEN.with(|open| open.replace(next));
    if previous == next {
        return;
    }

    match previous {
        Some(Panel::Wifi) => wifi::reset(),
        Some(Panel::Bluetooth) => bluetooth::closed(),
        Some(Panel::Notifications) => notifications::center::closed(),
        _ => {}
    }
    if next == Some(Panel::Notifications) {
        notifications::center::opened();
    }
    // a device menu belongs to the card it was opened from
    if previous.is_some() {
        context_menu::close();
    }

    mark_all();
}

// ───── view ──────────────────────────────────────────────────────────────────

/// An overlay-layer surface anchored to `vertical` and `horizontal` edges,
/// clear of what other surfaces reserve. Callers size it first.
pub(crate) fn overlay(
    base: LayerWindow,
    namespace: &'static str,
    vertical: Vertical,
    horizontal: Horizontal,
) -> LayerWindow {
    base.anchor_vertical(vertical)
        .anchor_horizontal(horizontal)
        .layer(Layer::Overlay)
        .space(Zone::Respect)
        .namespace(namespace)
}

/// Takes keys while clicked into, and closes the open panel on Escape.
pub(crate) fn dismissable(base: LayerWindow) -> LayerWindow {
    base.keyboard(Keyboard::OnDemand).on_key(|key| {
        if matches!(key, Key::Escape) {
            close();
        }
    })
}

/// The panel's surface: hidden and empty while closed, else `card()`.
pub fn surface(
    namespace: &'static str,
    panel: Panel,
    card: impl FnOnce() -> Rectangle,
) -> LayerWindow {
    let open = is_open(panel);

    let base = LayerWindow::new()
        .width(sys::size::popover::width())
        .height(sys::size::popover::height())
        .margin(Margin {
            top: 0,
            right: sys::space::gap::m() as i32,
            bottom: (sys::size::panel::height() + sys::space::gap::m()) as i32,
            left: 0,
        })
        .visible(open);
    let base = dismissable(overlay(
        base,
        namespace,
        Vertical::Bottom,
        Horizontal::Right,
    ));

    if !open {
        return base.child(blank());
    }

    base.child(card())
}

/// The frosted card every panel draws, its sections `gap` apart.
pub fn card(body: Vec<Box<dyn Widget>>, gap: f32) -> Rectangle {
    Card::new(body)
        .surface(Surface::Panel)
        .blur(true)
        .glass(true)
        .gap(gap)
        .build()
}
