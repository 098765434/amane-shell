use std::cell::RefCell;
use std::collections::HashMap;

use amane::request_frame;

use super::{ContextMenu, Entry, row_offset, size_for};
use crate::tokens::menu;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Idle,
    /// Anchor is temporarily fullscreen, waiting for the user to right-click
    /// (or press Esc to cancel). The right-click gives us a fresh input
    /// serial so `xdg_popup.grab()` is accepted by the compositor.
    Arming,
    Open,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FocusSource {
    Pointer,
    Keyboard,
}

/// One open level of the menu stack. Level 0 is the root menu; each deeper
/// level is the submenu of the focused item in the level above it.
pub(crate) struct Level {
    pub(crate) entries: Vec<Entry>,
    pub(crate) focus: Option<usize>,
    pub(crate) focus_source: Option<FocusSource>,
    /// Anchor rect for this level's popup, in its parent surface's coords.
    pub(crate) anchor_rect: (i32, i32, i32, i32),
}

pub(crate) struct Runtime {
    /// All registered menus, keyed by a caller-chosen name. Only one is open
    /// at a time (set by `open_at`); the rest are dormant definitions.
    pub(crate) registry: HashMap<&'static str, ContextMenu>,
    /// Menu to open on the next arm→right-click, resolved from `arm(name)`.
    pub(crate) pending: Option<&'static str>,
    /// Last pointer position on the (armed) anchor, so a right-click — which
    /// only carries a button — can open the menu at the cursor.
    pub(crate) cursor: (i32, i32),
    pub(crate) phase: Phase,
    /// The open levels, outermost first. Empty unless `phase == Open`.
    pub(crate) stack: Vec<Level>,
    /// Parent surface namespace for level 0. `None` → the desktop anchor
    /// (`NAMESPACE_ANCHOR`); `Some` → a caller-supplied surface the menu
    /// should hang under (set by `open_under`, e.g. a taskbar pill).
    pub(crate) root_parent: Option<&'static str>,
}

impl Runtime {
    fn new() -> Self {
        Self {
            registry: HashMap::new(),
            pending: None,
            cursor: (0, 0),
            phase: Phase::Idle,
            stack: Vec::new(),
            root_parent: None,
        }
    }
}

thread_local! {
    pub(crate) static RUNTIME: RefCell<Runtime> = RefCell::new(Runtime::new());
}

pub(crate) fn with<R>(f: impl FnOnce(&Runtime) -> R) -> R {
    RUNTIME.with(|r| f(&r.borrow()))
}

pub(crate) fn with_mut<R>(f: impl FnOnce(&mut Runtime) -> R) -> R {
    RUNTIME.with(|r| f(&mut r.borrow_mut()))
}

// ───── Registration + open/close ──────────────────────────────────────────

/// Register a menu under a name so it can be opened later. Call once per
/// distinct menu at startup.
pub fn register(key: &'static str, menu: ContextMenu) {
    with_mut(|r| {
        r.registry.insert(key, menu);
    });
}

/// Open a registered menu at the given screen coordinates.
pub fn open_at(key: &'static str, x: i32, y: i32) {
    open_root(key, (x, y, 1, 1), None);
}

/// Open a registered menu hung under a specific parent surface (namespace),
/// with level 0 anchored to the given rect in that surface's coordinates.
/// For a bottom-anchored parent (e.g. a taskbar), the popup's `FLIP_Y`
/// constraint makes the menu open upward automatically. The parent surface
/// must have non-`None` keyboard interactivity for keyboard nav to work.
pub fn open_under(key: &'static str, parent_ns: &'static str, x: i32, y: i32, w: i32, h: i32) {
    open_root(key, (x, y, w, h), Some(parent_ns));
}

/// Register `menu` under `key` and open it hung under `parent_ns` at the
/// point `(x, y)` of that surface: a menu built fresh for what was clicked.
pub fn show_under(
    key: &'static str,
    menu: ContextMenu,
    parent_ns: &'static str,
    (x, y): (i32, i32),
) {
    register(key, menu);
    open_under(key, parent_ns, x, y, 1, 1);
}

/// Opens `key` as the only level, at `anchor_rect` in its parent surface
/// (`None` is the desktop anchor).
fn open_root(
    key: &'static str,
    anchor_rect: (i32, i32, i32, i32),
    root_parent: Option<&'static str>,
) {
    with_mut(|r| {
        let Some(menu) = r.registry.get(key) else {
            return;
        };
        let level = Level {
            entries: menu.entries.clone(),
            focus: None,
            focus_source: None,
            anchor_rect,
        };
        r.stack.clear();
        r.stack.push(level);
        r.pending = Some(key);
        r.root_parent = root_parent;
        r.phase = Phase::Open;
    });
    request_frame();
}

/// Open a menu by its string name (for IPC / dynamic callers).
pub fn open_named(name: &str, x: i32, y: i32) {
    let key = with(|r| r.registry.keys().copied().find(|k| *k == name));
    if let Some(key) = key {
        open_at(key, x, y);
    }
}

/// Close the whole menu stack.
pub fn close() {
    with_mut(|r| {
        r.stack.clear();
        r.phase = Phase::Idle;
        r.root_parent = None;
    });
    request_frame();
}

/// The parent surface namespace for level 0 (see `open_under`).
pub(crate) fn root_parent() -> Option<&'static str> {
    with(|r| r.root_parent)
}

// ───── Arming (fullscreen hit-catcher) ──────────────────────────────────────

/// Arm the menu named `name`: expand the anchor fullscreen so the next
/// right-click captures a fresh input serial and opens it with a working
/// grab. Called from the niri keybind via IPC.
pub fn arm(name: &str) {
    with_mut(|r| {
        let key = r.registry.keys().copied().find(|k| *k == name);
        if let Some(key) = key {
            r.pending = Some(key);
            r.phase = Phase::Arming;
        }
    });
    request_frame();
}

/// Open the armed menu at the last tracked cursor position.
pub fn open_at_cursor() {
    let (key, x, y) = with(|r| (r.pending, r.cursor.0, r.cursor.1));
    if let Some(key) = key {
        open_at(key, x, y);
    }
}

/// Update the tracked cursor position (from the anchor's on_move).
pub fn set_cursor(x: i32, y: i32) {
    with_mut(|r| r.cursor = (x, y));
}

pub(crate) fn is_arming() -> bool {
    with(|r| matches!(r.phase, Phase::Arming))
}

// ───── Stack navigation ─────────────────────────────────────────────────────

/// Whether level `k` should currently render its popup.
pub(crate) fn visible_at(k: usize) -> bool {
    with(|r| matches!(r.phase, Phase::Open) && r.stack.len() > k)
}

/// Read a level's data, if it exists.
pub(crate) fn level<R>(k: usize, f: impl FnOnce(&Level) -> R) -> Option<R> {
    with(|r| r.stack.get(k).map(f))
}

/// Open the submenu of item `parent_index` at level `parent_k`, discarding
/// any levels deeper than `parent_k`. No-op if that item has no submenu.
pub(crate) fn push_level(parent_k: usize, parent_index: usize) {
    with_mut(|r| {
        let built = {
            let Some(parent) = r.stack.get(parent_k) else {
                return;
            };
            let Some(Entry::Item(item)) = parent.entries.get(parent_index) else {
                return;
            };
            let Some(sub) = item.submenu.clone() else {
                return;
            };
            let parent_width = size_for(&parent.entries).0 as i32;
            let row_y =
                menu::card::padding() as i32 + row_offset(&parent.entries, parent_index) as i32;
            Level {
                entries: sub,
                focus: None,
                focus_source: None,
                anchor_rect: (parent_width - 1, row_y, 1, menu::row::height() as i32),
            }
        };

        r.stack.truncate(parent_k + 1);
        if let Some(parent) = r.stack.get_mut(parent_k) {
            parent.focus = Some(parent_index);
        }
        r.stack.push(built);
    });
    request_frame();
}

/// Discard every level deeper than `k` (keeping levels `0..=k`).
pub(crate) fn pop_to(k: usize) {
    with_mut(|r| r.stack.truncate(k + 1));
    request_frame();
}
