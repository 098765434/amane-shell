//! Launcher — search for the whole desktop: a glass search pill high on the
//! screen with four round buttons beside it, for its modes (Applications,
//! Files, Actions and Clipboard, [`Mode`]). Typing grows the pill down into
//! a panel listing what the search finds ([`provider`]): an answer in a
//! card over the list, chips to narrow it to an app, and the rows, ranked
//! by how well they match and how much they've been used ([`rank`],
//! [`usage`]).
//!
//! Applications is the app grid it grew from: a row of category
//! pills over the apps five across, the ones used most in a suggested row
//! on top. Two fingers move it as they would a page: up and down they
//! scroll the grid, which coasts on after a flick ([`Kinetic`]); sideways
//! they slide the next category's apps in under them ([`Pager`]).
//!
//! The launcher draws the search text itself rather than focusing a
//! `TextInput`, so every key reaches it: Tab narrows the search to the row
//! picked (an app's own actions) or moves between the search field, the
//! pills and the apps; Up brings back the searches run before; Ctrl+1 … 4
//! pick a mode; holding Super shows each row's fuller line, like an app's
//! command (`text_edit::Field`, amane's `modifiers()`).
//!
//! The surface covers the output, clear outside the glass, so a click
//! anywhere else closes it. The compositor frosts behind the sheet and the
//! buttons where the surface says
//! ([`glass_region`](emaki_kit::glass_region), [`shape`]); one spring
//! carries both between the pill and the panel, the frost with them.
//!
//! It shares the notification center's and Quick Settings'
//! one-open-at-a-time state ([`Panel::Launcher`]), opened over IPC:
//! `launcher [toggle | open | close] [apps | files | actions | clipboard]`,
//! which a niri keybind sends. See `docs/user/features/launcher.md`.

mod actions;
mod answers;
mod app_menu;
mod assistant;
mod buttons;
mod choreo;
mod entries;
mod grid;
mod header;
mod input;
mod ipc;
mod keys;
mod layout;
mod mode;
mod model;
mod phone_apps;
mod provider;
mod rank;
mod results;
mod shape;
#[cfg(test)]
mod tests;
mod usage;
mod view;

use input::*;
pub use ipc::{run, run_assistant};
use keys::*;
use layout::*;
use view::*;

pub(crate) use actions::profile_name;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use amane::{
    Animation, Apps, Button, Column, DesktopApp, Easing, Horizontal, Key, Keyboard, LayerWindow,
    Parent, Rectangle, Row, Scroll, ScrollSource, Service, Stack, Start, Vertical, Widget,
    WindowSize, modifiers, window_size,
};

use crate::chrome::panel::{self, Panel, close, is_open, overlay};
use crate::services::assistant as assistant_service;
use crate::services::launcher::clipboard_history;
use crate::services::launcher::file_search::{self, FileKind};
use emaki_kit::divider::Divider;
use emaki_kit::kinetic::{Axis, AxisLock, Kinetic};
use emaki_kit::layout::{boxed, pad, track_cursor};
use emaki_kit::material::Material;
use emaki_kit::motion;
use emaki_kit::pager::{Pager, PagerMotion};
use emaki_kit::reveal::{self, Reveal, Swell};
use emaki_kit::scroll_view::ScrollbarFade;
use emaki_kit::settings::{AppsLayout, QuickKey, change_launcher, launcher};
use emaki_kit::text_edit::{self, Field};

use emaki_kit::shell_ipc::{self, LauncherAction as Action};
use entries::{Entry, Scope};
pub use mode::Mode;
use provider::{Found, Query};
use shape::{Geometry, PILL_H, PILL_W};

emaki_kit::marker! {
    /// The launcher's own state changed (what's picked, the mode, the
    /// page, a capture): only its window draws again.
    pub Launcher
}

/// Layer-surface namespace; the niri `layer-rule` matches it (a clear
/// surface that says where to frost).
pub const LAUNCHER_NS: &str = "emaki.launcher";

// TODO(token): `launcher.grid.*`, `launcher.panel.*`, `launcher.result.*`,
// `launcher.open.scale` and `launcher.motion.*` once Penpot has them
// (manual §1.1).
/// Apps across the grid, and rows of them in view.
const COLS: usize = 5;
const ROWS: usize = 3;
/// An app's square cell, its icon, and the highlight around the icon.
const CELL: f32 = 150.0;
const ICON: f32 = 64.0;
const ICON_BOX: f32 = 84.0;
/// The room between the panel's edge and what's on it, and between the
/// parts on it.
const INSET: f32 = 15.0;
const GAP: f32 = 10.0;
/// How tall the pill bar is.
const PILLS: f32 = 40.0;
/// A result row, the gap under it, and how many show before the list
/// scrolls.
const ROW: f32 = 52.0;
const ROW_GAP: f32 = 4.0;
const MAX_ROWS: usize = 8;
/// A section's heading over its rows, with the gap under it.
const HEADING: f32 = 36.0;
/// The answer card over the results, and the chips under it.
const ANSWER_H: f32 = 88.0;
const CHIP_H: f32 = 32.0;
/// The kinds Files offers as chips.
const FILE_CHIPS: [FileKind; 4] = [
    FileKind::Pdf,
    FileKind::Image,
    FileKind::Document,
    FileKind::Folder,
];
/// How wide Clipboard's and Files' list is, beside the preview of the row
/// picked.
const CLIP_LIST_W: f32 = 420.0;
/// How tall Clipboard is under its search row while it asks to keep copies.
const EMPTY_H: f32 = 200.0;
/// How the categories turn under the fingers (`launcher.paging.*`): the
/// dock's paging, a touch calmer for a page as wide as the panel.
fn paging_motion() -> PagerMotion {
    use crate::tokens::launcher::paging;
    PagerMotion {
        duration: emaki_kit::tokens::ms(paging::duration()),
        easing: Easing::Spring {
            bounce: paging::bounce(),
        },
        flick: paging::flick(),
        rubber_band: Some(paging::rubber_band()),
    }
}
/// How long the scrollbar stays after the body last moved.
/// How strongly the highlight shows while the focus is elsewhere.
const UNFOCUSED: f32 = 0.45;

/// How wide the grid is, and how tall it is in view.
const GRID_W: f32 = CELL * COLS as f32;
const BODY: f32 = CELL * ROWS as f32;
/// How wide the panel is in Applications, round the grid.
const APPS_W: f32 = GRID_W + INSET * 2.0;
/// Where the highlight sits in a cell: centred, at the cell's top padding.
const BOX_X: f32 = (CELL - ICON_BOX) / 2.0;
const BOX_Y: f32 = 10.0;

/// Where the keys go: Tab moves between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Zone {
    Search,
    Pills,
    Body,
}

impl Zone {
    /// The next zone Tab goes to (`back` for Shift+Tab), through the
    /// pills only where there are some.
    fn next(self, back: bool, pills: bool) -> Self {
        let order: &[Self] = if pills {
            &[Self::Search, Self::Pills, Self::Body]
        } else {
            &[Self::Search, Self::Body]
        };
        let at = order.iter().position(|zone| *zone == self).unwrap_or(0);
        let len = order.len();
        order[if back {
            (at + len - 1) % len
        } else {
            (at + 1) % len
        }]
    }
}

struct State {
    field: Field,
    mode: Mode,
    // what the search is narrowed to
    scope: Option<Scope>,
    // the app or entry highlighted
    selected: usize,
    focus: Zone,
    // which of the searches run before is in the field, from Up
    history_at: Option<usize>,
    // Super was up since the launcher opened: it's held from Mod+Space, and
    // only a press after that shows the rows' fuller lines
    logo_armed: bool,
    // the mode button under the pointer, and whether the back button is
    hover_mode: Option<Mode>,
    back_hover: bool,
    // an action's quick keys being typed: every key goes to them
    capture: Option<Capture>,
    // what the last frame showed: a change snaps the highlight and the
    // scroll rather than slide them between lists that have nothing to do
    // with each other
    shown: (Kind, usize),
    // how many categories there were to turn through last frame
    pages: usize,
    // the category the pill bar last scrolled to show
    pills_for: Option<usize>,
    // a finger gesture over the body, scrolling or turning the page
    gesture: AxisLock,
    // when the body last moved, and when the scrollbar is next due to fade
}

impl State {
    const fn new() -> Self {
        Self {
            field: Field::new(),
            mode: Mode::Home,
            scope: None,
            selected: 0,
            focus: Zone::Search,
            history_at: None,
            logo_armed: false,
            hover_mode: None,
            back_hover: false,
            capture: None,
            shown: (Kind::Idle, 0),
            pages: 0,
            pills_for: None,
            gesture: AxisLock::DEFAULT,
        }
    }
}

/// Quick keys being typed for an action: its key and name, the keys so
/// far, and why the last try couldn't be saved.
#[derive(Clone, Debug, PartialEq)]
struct Capture {
    action: String,
    title: String,
    keys: String,
    error: Option<String>,
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::new()) };

    // 0 closed … 1 open: the glass fades and swells in
    static OPEN: RefCell<Reveal> = RefCell::new(Reveal::new());

    // the glass, between the pill and the panel; it starts where it
    // belongs each time the launcher opens (`FRESH`)
    static GEOMETRY: RefCell<Animation<Geometry>> = RefCell::new(motion::sheet(Geometry::default()));
    static FRESH: Cell<bool> = const { Cell::new(true) };

    // each mode button's lift under the pointer
    static LIFT: RefCell<[Animation; 4]> = RefCell::new(std::array::from_fn(|_| motion::spring(1.0)));

    // the body's scroll, and the pill bar's sideways
    static SCROLL: RefCell<Kinetic> = RefCell::new(Kinetic::new());
    static PILL_SCROLL: RefCell<Kinetic> = RefCell::new(Kinetic::new());

    // the category showing, as an index into the categories shown
    static PAGER: RefCell<Pager> = RefCell::new(Pager::new(Axis::Horizontal, &paging_motion()));

    // where the highlight is gliding to
    static HIGHLIGHT_X: RefCell<Animation> = RefCell::new(glide(BOX_X));
    static HIGHLIGHT_Y: RefCell<Animation> = RefCell::new(glide(BOX_Y));
    // how strongly it shows: less while the focus is on the search or pills
    static HIGHLIGHT_ALPHA: RefCell<Animation> = RefCell::new(motion::fade(1.0, motion::duration::hover()));

    // 0 … 1: the scrollbar, showing while the body moves
    static SCROLLBAR: RefCell<ScrollbarFade> = RefCell::new(ScrollbarFade::new());
}

/// How the highlight moves between rows or apps: quickly, settling with
/// a breath of overshoot, so holding an arrow key keeps up.
fn glide(value: f32) -> Animation {
    motion::eased(
        value,
        motion::duration::press(),
        Easing::Spring {
            bounce: crate::tokens::sys::motion::pop::bounce(),
        },
    )
}

fn with<R>(change: impl FnOnce(&mut State) -> R) -> R {
    STATE.with_borrow_mut(change)
}

// ───── runtime ───────────────────────────────────────────────────────────────

/// Gone from the screen: the next opening starts afresh, on the pill.
fn hidden() {
    // listening ends with the launcher
    assistant_service::listen_cancel();
    let left = with(|state| std::mem::replace(state, State::new()).scope);
    left_scope(left);
    FRESH.set(true);
    choreo::reset();
    header::reset();
    LIFT.with_borrow_mut(|lifts| *lifts = std::array::from_fn(|_| motion::spring(1.0)));
    SCROLL.with_borrow_mut(|scroll| *scroll = Kinetic::new());
    PILL_SCROLL.with_borrow_mut(|scroll| *scroll = Kinetic::new());
    PAGER.with_borrow_mut(|pager| pager.jump(0, &paging_motion()));
    SCROLLBAR.with_borrow_mut(ScrollbarFade::reset);
}

/// `emaki` IPC: `launcher [toggle | open | close] [mode]`.
/// Opens the launcher on `mode`, as `launcher open <mode>` does.
pub fn open(mode: Mode) {
    switch_mode(mode);
    if !is_open(Panel::Launcher) {
        panel::toggle(Panel::Launcher);
    }
    Launcher::changed();
}

// ───── view ──────────────────────────────────────────────────────────────────

/// The surface: hidden while closed, else the glass high on a clear
/// surface that closes the launcher when clicked.
pub fn surface() -> LayerWindow {
    Launcher::watch();
    // the clipboard's history follows its setting, whether or not the
    // launcher is open
    clipboard_history::sync(&launcher().clipboard);
    heard();
    let open = is_open(Panel::Launcher);
    let unfolded = unfold(open);

    if !reveal::showing(open, unfolded) {
        hidden();
        return panel::empty(window());
    }

    let apps = Apps::read();
    let view = View::new(apps.list());

    // the glass heads for where this moment's view puts it: opening, from
    // a drop at the pill's centre that stretches into the pill, holding as
    // the pill a moment before growing into a mode's panel; closing, the
    // pill draws back towards its centre
    let screen = window_size();
    let pill = shape::target(screen, (0.0, 0.0), true);
    let compact = view.kind == Kind::Idle || choreo::holding();
    let mut target = shape::target(screen, (view.width(), view.height()), compact);
    if !open && view.kind == Kind::Idle {
        target = shape::seed(pill);
    }
    let geometry = GEOMETRY.with_borrow_mut(|anim| {
        if FRESH.replace(false) {
            *anim = motion::sheet(if choreo::from_drop() {
                shape::seed(pill)
            } else {
                target
            });
        }
        anim.to(target);
        anim.value()
    });

    // the mode buttons bud off the pill's end one after another
    let budded = std::array::from_fn(|index| choreo::bud(index, open));
    let buds = geometry.buds(
        budded,
        motion::scale(choreo::BUD_SCALE),
        motion::travel(crate::tokens::launcher::mode_button::slide()),
        !emaki_kit::theme::reduced_motion(),
    );

    // the glass swells in about the pill's centre, the frost with it
    let swell = Swell::new(open, unfolded, crate::tokens::launcher::open::scale());
    let (sheet, _) = geometry.drawn();
    let centre = (sheet.x + sheet.width / 2.0, sheet.y + PILL_H / 2.0);
    let frost = swell.frost(geometry.frost(&buds), centre);

    let mut layers: Vec<Box<dyn Widget>> = vec![boxed(glass(&view, geometry, open))];
    layers.extend(buttons::mode_buttons(&buds, view.mode));
    drop(view);
    drop(apps);

    // a layer scales about its own centre, the surface's: moved by how far
    // that is from the pill's, scaled down, it swells from the pill
    let (screen_w, screen_h) = screen;
    let shift = (
        (centre.0 - screen_w / 2.0) * (1.0 - swell.scale),
        (centre.1 - screen_h / 2.0) * (1.0 - swell.scale),
    );
    let group = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .child(Stack::new(layers).width(Parent).height(Parent))
        .opacity(swell.opacity)
        .scale(swell.scale)
        .translate(shift.0, shift.1);

    // a click outside the glass closes it; the glass keeps its own clicks
    let outside = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .on_click(|_| close())
        .child(group);

    window()
        .visible(true)
        .blur_shapes(frost)
        .child(track_cursor(outside))
}

/// The surface over the output, keeping clear of what the bar reserves.
fn window() -> LayerWindow {
    let base = LayerWindow::new()
        .width(WindowSize::Full)
        .height(WindowSize::Full)
        // modal while open: every key is the launcher's from the first moment
        .keyboard(Keyboard::Exclusive)
        .on_key(on_key);
    overlay(base, LAUNCHER_NS, Vertical::Middle, Horizontal::Middle).output(panel::on_output())
}
