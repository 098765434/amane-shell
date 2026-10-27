//! The one-open-at-a-time state Quick Settings, the notification center
//! and the launcher share, and the overlay-surface helpers they and the
//! toasts are built on.
//!
//! Opening one closes the others; Escape or toggling again closes it.

use std::cell::Cell;

use amane::{Horizontal, Key, Keyboard, Layer, LayerWindow, Margin, Vertical, Zone, mark_all};

use crate::components::material;
use crate::components::{context_menu, launcher, notifications, quick_settings, status_bar};
use crate::tokens::sys;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    /// The notification center, a sidebar at the right edge.
    Notifications,
    /// Quick Settings, a bottom-right card, see [`quick_settings`].
    QuickSettings,
    /// The app launcher, a panel in the middle, see [`launcher`].
    Launcher,
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
        Some(Panel::Notifications) => notifications::center::closed(),
        Some(Panel::QuickSettings) => quick_settings::closed(),
        Some(Panel::Launcher) => launcher::closed(),
        _ => {}
    }
    match next {
        Some(Panel::Notifications) => notifications::center::opened(),
        Some(Panel::QuickSettings) => quick_settings::opened(),
        Some(Panel::Launcher) => launcher::opened(),
        _ => {}
    }
    // a menu belongs to the surface it was opened from
    if previous.is_some() {
        context_menu::close();
    }

    mark_all();
}

// ───── view ──────────────────────────────────────────────────────────────────

/// An overlay-layer surface anchored to `vertical` and `horizontal` edges,
/// clear of what other surfaces reserve, its labels vibrant where vibrancy
/// is on (`material::vibrant`). Callers size it first.
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
        .vibrant(material::vibrant())
}

/// Takes keys as `keyboard` says, and closes the open panel on Escape.
pub(crate) fn dismissable(base: LayerWindow, keyboard: Keyboard) -> LayerWindow {
    base.keyboard(keyboard).on_key(|key| {
        if matches!(key, Key::Escape) {
            close();
        }
    })
}

/// Where a card in the bar's right-hand corner sits: `gap.m` in from the
/// right edge and clear of the bar, above it or below it.
pub(crate) fn popover_margin() -> Margin {
    let clear = (sys::size::panel::height() + sys::space::gap::m()) as i32;
    let top = status_bar::edge() == Vertical::Top;

    Margin {
        top: if top { clear } else { 0 },
        right: sys::space::gap::m() as i32,
        bottom: if top { 0 } else { clear },
        left: 0,
    }
}
