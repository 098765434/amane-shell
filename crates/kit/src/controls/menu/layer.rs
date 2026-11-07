//! The shell's host: a menu on its own layer surface, opened over the
//! desktop or hung under one of the shell's surfaces, with submenus each a
//! popup of their own (up to [`MAX_DEPTH`] deep). Its panel is a glass card
//! niri frosts; its rows are [`rows`](super::rows)' with [`Look::Layer`].
//! See `docs/shell/context-menu.md`.
//!
//! The shell registers [`anchor`] as a window and every one of [`LEVELS`] as
//! a popup, once; then [`show_under`] opens a menu built for what was
//! clicked.

use amane::{
    Button, ConstraintAdjustment, Gravity, Horizontal, Key, Keyboard, Layer, LayerWindow, Parent,
    Point, Pointer, Popup, Rectangle, Vertical, Widget, WindowSize, request_frame,
};

pub use super::stack::{
    arm, close, is_open_under, open_at, open_named, open_under, register, show_under,
};

use super::keys::{self, Move};
#[cfg(test)]
use super::model::ContextMenu;
use super::model::{Entry, MenuItem};
use super::rows::{Columns, Look, RowState};
use super::stack::{self, FocusSource, with, with_mut};
use crate::card::{Card, Variant};
use crate::divider::Divider;
use crate::layout::{blank, boxed, pad};
use crate::material::{self, Material};
use crate::tokens::menu;

/// Maximum submenu nesting depth. Each level is a distinct popup slot with
/// its own compile-time namespace; bump this (and `NS`) to allow deeper.
pub const MAX_DEPTH: usize = 8;

/// The anchor layer-surface namespace. The niri `layer-rule` matches this and
/// its `popups { }` block styles every descendant popup (all menu levels).
pub const ANCHOR_NS: &str = "amane.context-menu-anchor";

/// One namespace per depth level. Level `k`'s popup is parented to level
/// `k-1` (level 0 to the anchor).
const NS: [&str; MAX_DEPTH] = [
    "amane.context-menu.0",
    "amane.context-menu.1",
    "amane.context-menu.2",
    "amane.context-menu.3",
    "amane.context-menu.4",
    "amane.context-menu.5",
    "amane.context-menu.6",
    "amane.context-menu.7",
];

/// The popup namespace of menu level `k`.
pub fn level_ns(k: usize) -> &'static str {
    NS[k]
}

fn parent_ns(k: usize) -> &'static str {
    if k == 0 {
        // level 0 hangs under a caller-supplied surface when one is set
        // (e.g. a dock icon via `open_under`), else the desktop anchor
        stack::root_parent().unwrap_or(ANCHOR_NS)
    } else {
        NS[k - 1]
    }
}

// ───── View entry points ─────────────────────────────────────────────────

/// Overlay anchor layer-surface. Two modes:
///
/// - **Idle / Open**: 1×1 top-left, click-through (`input_region` empty).
///   Does not block any pointer input to apps.
/// - **Arming**: fullscreen, catches right-click (captures a fresh input
///   serial for `xdg_popup.grab()`), Esc cancels.
///
/// `Keyboard::OnDemand` is required for popup keyboard inheritance —
/// wlr-layer-shell says child popups inherit the parent's keyboard mode,
/// and `xdg_popup.grab()` can only route keyboard to a popup whose
/// inherited mode is not None.
pub fn anchor() -> LayerWindow {
    let arming = stack::is_arming();

    let size: WindowSize = if arming {
        WindowSize::Full
    } else {
        WindowSize::Fixed(1.0)
    };

    let mut lw = LayerWindow::new()
        .width(size)
        .height(size)
        .layer(Layer::Overlay)
        .keyboard(Keyboard::OnDemand)
        .namespace(ANCHOR_NS)
        .visible(true);

    if !arming {
        lw = lw
            .anchor_vertical(Vertical::Top)
            .anchor_horizontal(Horizontal::Left)
            .input_region(Vec::new());
    }

    let mut rect = blank();

    if arming {
        rect = rect
            .on_track(|p: Point| stack::set_cursor(p.x as i32, p.y as i32))
            .on_click(|btn: Button| {
                if btn == Button::Right {
                    stack::open_at_cursor();
                } else {
                    stack::close();
                }
            });
    }

    lw.on_key(|key| {
        if matches!(key, Key::Escape) {
            stack::close();
        }
    })
    .child(rect)
}

/// The fixed array of per-depth popup view fns. Register all of them on the
/// `App` (`for f in LEVELS { app = app.popup(f); }`). Each renders the menu
/// level at its depth, reading the open stack.
pub const LEVELS: [fn() -> Popup; MAX_DEPTH] = [
    || render(0),
    || render(1),
    || render(2),
    || render(3),
    || render(4),
    || render(5),
    || render(6),
    || render(7),
];

/// Render the popup for depth level `k`. Returns an invisible placeholder
/// when the stack isn't that deep, so the slot stays registered but mapped
/// only while its level is open.
fn render(k: usize) -> Popup {
    let Some((entries, anchor_rect)) = stack::level(k, |l| (l.entries.clone(), l.anchor_rect))
    else {
        return Popup::new(NS[k])
            .size(1, 1)
            .anchor_rect(0, 0, 1, 1)
            .parent(parent_ns(k))
            .visible(false);
    };

    let (width, height) = size_for(&entries);
    let card = card_widget(&entries, k);
    let (ax, ay, aw, ah) = anchor_rect;

    // Level 0 can slide on both axes; deeper levels keep their x (they flip
    // to the parent's other side instead) and sit just left of the row edge.
    let constraint = if k == 0 {
        ConstraintAdjustment::SLIDE_X
            | ConstraintAdjustment::SLIDE_Y
            | ConstraintAdjustment::FLIP_X
            | ConstraintAdjustment::FLIP_Y
    } else {
        ConstraintAdjustment::SLIDE_Y | ConstraintAdjustment::FLIP_X | ConstraintAdjustment::FLIP_Y
    };

    let mut popup = Popup::new(NS[k])
        .size(width as u32, height as u32)
        .anchor_rect(ax, ay, aw, ah)
        .vibrant(material::vibrant())
        .parent(parent_ns(k))
        .gravity(Gravity::BottomRight)
        .constraint_adjustment(constraint)
        .grab()
        .visible(stack::visible_at(k))
        .on_key(move |key| handle_key(k, key))
        .child(card);

    if k > 0 {
        popup = popup.offset(-menu::submenu::offset() as i32, 0);
    } else {
        popup = popup.on_dismiss(stack::close);
    }

    popup
}

// ───── Card / row rendering ──────────────────────────────────────────────

fn card_widget(entries: &[Entry], level: usize) -> Rectangle {
    let columns = Columns::of_entries(entries);
    let rows: Vec<Box<dyn Widget>> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| build_entry(i, entry, level, columns))
        .collect();

    Card::new(rows)
        .material(Material::Menu)
        .variant(Variant::Outlined)
        .glass(true)
        .padding(menu::card::padding())
        .build()
}

fn build_entry(index: usize, entry: &Entry, level: usize, columns: Columns) -> Box<dyn Widget> {
    match entry {
        Entry::Separator => boxed(Divider::build()),
        Entry::Header(title) => boxed(Look::Layer.header(title)),
        Entry::Item(item) => boxed(row(index, item, level, columns)),
    }
}

fn row(index: usize, item: &MenuItem, level: usize, columns: Columns) -> Rectangle {
    let (focused, source) =
        stack::level(level, |l| (l.focus == Some(index), l.focus_source)).unwrap_or((false, None));
    let state = RowState {
        focused,
        keyboard: source == Some(FocusSource::Keyboard),
        active: false,
    };
    let colors = Look::Layer.colors(item, state);

    let has_submenu = item.has_submenu();
    let on_select = item.on_select.clone();

    let mut rect = Rectangle::new()
        .width(Parent)
        .height(Look::Layer.row_height())
        .fill(colors.fill)
        .radius(menu::row::radius())
        .padding(pad(Look::Layer.row_padding(), 0.0))
        .child(Look::Layer.content(item, columns, colors));

    if item.enabled {
        rect = rect
            .cursor(Pointer)
            .on_hover(move |inside| set_focus(level, index, inside, has_submenu))
            .on_click(move |_button| {
                if has_submenu {
                    stack::push_level(level, index);
                } else {
                    if let Some(f) = &on_select {
                        f();
                    }
                    stack::close();
                }
            });
    }

    rect
}

// ───── Focus / keyboard ──────────────────────────────────────────────────

/// Pointer hover on a row at `level`. Updates the highlight and, for a
/// submenu row, opens its child level; for a leaf row, closes any deeper
/// levels.
fn set_focus(level: usize, index: usize, inside: bool, has_submenu: bool) {
    if !inside {
        with_mut(|r| {
            let is_deepest = r.stack.len() == level + 1;
            if let Some(l) = r.stack.get_mut(level)
                && l.focus == Some(index)
                && is_deepest
            {
                l.focus = None;
                l.focus_source = None;
            }
        });
        request_frame();
        return;
    }

    let already_open = with(|r| {
        r.stack.len() > level + 1 && r.stack.get(level).map(|l| l.focus) == Some(Some(index))
    });

    with_mut(|r| {
        if let Some(l) = r.stack.get_mut(level) {
            l.focus = Some(index);
            l.focus_source = Some(FocusSource::Pointer);
        }
    });

    if has_submenu {
        if !already_open {
            stack::push_level(level, index);
        }
    } else {
        stack::pop_to(level);
    }
    request_frame();
}

/// Keyboard handler for the popup at `level`. Only the deepest (topmost
/// grabbing) popup receives keys, so `level` is the active level.
fn handle_key(level: usize, key: Key) {
    let (selectable, focus) = stack::level(level, |l| {
        (
            l.entries.iter().map(Entry::selectable).collect::<Vec<_>>(),
            l.focus,
        )
    })
    .unwrap_or_default();
    match keys::press(key, &selectable, focus, None) {
        Move::Close => stack::close(),
        Move::To(row) => {
            with_mut(|r| {
                if let Some(l) = r.stack.get_mut(level) {
                    l.focus = Some(row);
                    l.focus_source = Some(FocusSource::Keyboard);
                }
            });
            request_frame();
        }
        Move::Open => {
            descend(level);
        }
        Move::Back => {
            if level > 0 {
                stack::pop_to(level - 1);
            }
        }
        Move::Pick if !descend(level) => invoke_focused(level),
        _ => {}
    }
}

/// If the focused row at `level` has a submenu, open it and focus its first
/// item. Returns whether it descended.
fn descend(level: usize) -> bool {
    let focus = stack::level(level, |l| l.focus).flatten();
    let Some(i) = focus else {
        return false;
    };
    let has = stack::level(
        level,
        |l| matches!(l.entries.get(i), Some(Entry::Item(it)) if it.has_submenu()),
    )
    .unwrap_or(false);

    if has {
        stack::push_level(level, i);
        focus_first(level + 1);
        true
    } else {
        false
    }
}

fn focus_first(level: usize) {
    with_mut(|r| {
        if let Some(l) = r.stack.get_mut(level) {
            l.focus = l.entries.iter().position(Entry::selectable);
            l.focus_source = Some(FocusSource::Keyboard);
        }
    });
    request_frame();
}

fn invoke_focused(level: usize) {
    let on_select = with(|r| {
        let l = r.stack.get(level)?;
        let i = l.focus?;
        match l.entries.get(i)? {
            Entry::Item(it) if it.enabled => it.on_select.clone(),
            _ => None,
        }
    });

    if let Some(f) = on_select {
        f();
        stack::close();
    }
}

// ───── Measurement ───────────────────────────────────────────────────────

/// How big the card is for `entries`.
pub(crate) fn size_for(entries: &[Entry]) -> (f32, f32) {
    let columns = Columns::of_entries(entries);
    let widest = entries
        .iter()
        .map(|e| match e {
            Entry::Item(item) => Look::Layer.row_width(item, columns),
            Entry::Header(title) => Look::Layer.header_width(title),
            Entry::Separator => 0.0,
        })
        .fold(0.0_f32, f32::max);

    let width = (widest + 2.0 * menu::card::padding()).clamp(menu::min_width(), menu::max_width());
    let height = 2.0 * menu::card::padding()
        + entries
            .iter()
            .map(|e| Look::Layer.entry_height(e))
            .sum::<f32>();

    (width, height)
}

/// How far down the card's content entry `index` starts.
pub(crate) fn row_offset(entries: &[Entry], index: usize) -> f32 {
    entries
        .iter()
        .take(index)
        .map(|e| Look::Layer.entry_height(e))
        .sum()
}

/// Runs `f` with a test menu open at level 0.
#[cfg(test)]
fn with_open(menu: ContextMenu, f: impl FnOnce()) {
    register("test", menu);
    open_at("test", 10, 10);
    f();
    close();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::Check;

    #[test]
    fn a_menu_with_every_kind_of_line_draws_and_measures() {
        let menu = ContextMenu::new()
            .header("Apps")
            .item(MenuItem::new("Open").shortcut("Ctrl+O"))
            .item(MenuItem::new("Mode").check(Check::Radio(true)))
            .separator()
            .item(MenuItem::new("More").submenu(ContextMenu::new().item(MenuItem::new("A"))))
            .item(MenuItem::new("Delete").destructive());
        let (width, height) = size_for(menu.entries());
        assert!(width >= menu::min_width() && height > 0.0);
        assert!(row_offset(menu.entries(), 2) > row_offset(menu.entries(), 1));
        with_open(menu, || {
            let _ = render(0);
            handle_key(0, Key::Down);
            assert_eq!(stack::level(0, |l| l.focus), Some(Some(1)));
            handle_key(0, Key::Up);
            assert_eq!(stack::level(0, |l| l.focus), Some(Some(5)));
        });
    }
}
