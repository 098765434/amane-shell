//! Launcher — macOS Tahoe's Spotlight: a glass search pill high on the
//! screen with four round buttons beside it, for its modes (Applications,
//! Files, Actions and Clipboard, [`Mode`]). Typing grows the pill down into
//! a panel listing what the search finds ([`provider`]): an answer in a
//! card over the list, chips to narrow it to an app, and the rows, ranked
//! by how well they match and how much they've been used ([`rank`],
//! [`usage`]).
//!
//! Applications is the Launchpad grid it grew from: a row of category
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
//! buttons where the surface says ([`glass_region`], [`shape`]); one spring
//! carries both between the pill and the panel, the frost with them.
//!
//! It shares the notification center's and Quick Settings'
//! one-open-at-a-time state ([`Panel::Launcher`]), opened over IPC:
//! `launcher [toggle | open | close] [apps | files | actions | clipboard]`,
//! which a niri keybind sends. See `docs/launcher.md`.

mod actions;
mod answers;
mod app_menu;
mod entries;
mod grid;
mod header;
mod mode;
mod model;
mod provider;
mod rank;
mod results;
mod shape;
mod usage;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use amane::{
    Animation, Apps, Blend, Button, Color, Column, DesktopApp, Easing, Horizontal, Key, Keyboard,
    LayerWindow, Parent, Rectangle, Row, Scroll, ScrollSource, Service, Stack, Start, Vertical,
    Widget, WindowSize, mark_all, modifiers, window_size,
};

use crate::components::divider::Divider;
use crate::components::glass_region;
use crate::components::kinetic::{Axis, AxisLock, Kinetic};
use crate::components::layout::{blank, boxed, pad, track_cursor};
use crate::components::material::Material;
use crate::components::motion;
use crate::components::pager::{Pager, PagerMotion};
use crate::components::panel::{self, Panel, close, is_open, overlay};
use crate::components::text_edit::{self, Field};
use crate::services::clipboard_history;
use amane_kit::settings::{AppsLayout, QuickKey, change_launcher, launcher};

use entries::{Entry, Scope};
use mode::Mode;
use provider::{Found, Query};
use shape::{Geometry, PILL_H, PILL_W};

/// Layer-surface namespace; the niri `layer-rule` matches it (a clear
/// surface that says where to frost).
pub const LAUNCHER_NS: &str = "amane.launcher";

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
/// How wide Clipboard's list is, beside the preview of the copy picked.
const CLIP_LIST_W: f32 = 420.0;
/// How tall a mode with nothing to show yet is under its search row.
const EMPTY_H: f32 = 200.0;
/// How far one wheel notch scrolls.
const WHEEL_STEP: f32 = 60.0;
/// How small the glass is as it swells in.
const OPEN_SCALE: f32 = 0.96;
/// How the categories turn under the fingers: the dock's paging, a touch
/// calmer for a page as wide as the panel.
const PAGING: PagerMotion = PagerMotion {
    duration: Duration::from_millis(450),
    easing: Easing::Spring { bounce: 0.08 },
    flick: 1.2,
    rubber_band: Some(0.3),
};
/// How long the scrollbar stays after the body last moved.
const SCROLLBAR_IDLE: Duration = Duration::from_millis(800);
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
    scrolled: Option<Instant>,
    fade_due: Option<Instant>,
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
            scrolled: None,
            fade_due: None,
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
    static OPEN: RefCell<Animation> = RefCell::new(motion::pop(0.0));

    // whether OPEN is heading for 1
    static OPENING: Cell<bool> = const { Cell::new(false) };

    // the glass, between the pill and the panel; it starts where it
    // belongs each time the launcher opens (`FRESH`)
    static GEOMETRY: RefCell<Animation<Geometry>> = RefCell::new(motion::sheet(Geometry::default()));
    static FRESH: Cell<bool> = const { Cell::new(true) };

    // the body's scroll, and the pill bar's sideways
    static SCROLL: RefCell<Kinetic> = RefCell::new(Kinetic::new());
    static PILL_SCROLL: RefCell<Kinetic> = RefCell::new(Kinetic::new());

    // the category showing, as an index into the categories shown
    static PAGER: RefCell<Pager> = RefCell::new(Pager::new(Axis::Horizontal, &PAGING));

    // where the highlight is gliding to
    static HIGHLIGHT_X: RefCell<Animation> = RefCell::new(motion::fade(BOX_X, motion::HOVER));
    static HIGHLIGHT_Y: RefCell<Animation> = RefCell::new(motion::fade(BOX_Y, motion::HOVER));
    // how strongly it shows: less while the focus is on the search or pills
    static HIGHLIGHT_ALPHA: RefCell<Animation> = RefCell::new(motion::fade(1.0, motion::HOVER));

    // 0 … 1: the body fading in after the list under it changed
    static SWITCH: RefCell<Animation> = RefCell::new(motion::fade(1.0, motion::FADE));

    // 0 … 1: the scrollbar, showing while the body moves
    static SCROLLBAR: RefCell<Animation> = RefCell::new(motion::fade(0.0, motion::FADE));
}

fn with<R>(change: impl FnOnce(&mut State) -> R) -> R {
    STATE.with_borrow_mut(change)
}

// ───── what it shows ─────────────────────────────────────────────────────────

/// What's under the search row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Nothing: the glass is a pill.
    Idle,
    /// What a search found.
    Results,
    /// Applications: the grid, or the apps listed.
    Apps,
    /// A mode with nothing to show yet.
    Empty,
    /// Clipboard, before its history is turned on: asking to.
    Ask,
}

/// How the body lays its items out: the grid, or a list of rows, the first
/// `skip` items (the answer) drawn over it rather than in it, and a
/// section's heading over each row in `headings` (by its place in the list).
#[derive(Clone, PartialEq)]
enum Layout {
    Grid,
    List { skip: usize, headings: Rc<[usize]> },
}

impl Layout {
    /// Where item `index` is down the body: its top and its height.
    fn span(&self, index: usize) -> (f32, f32) {
        match self {
            Self::Grid => ((index / COLS) as f32 * CELL, CELL),
            Self::List { skip, .. } if index < *skip => (0.0, 0.0),
            Self::List { skip, headings } => {
                let row = index - skip;
                let above = headings.iter().filter(|at| **at <= row).count();
                (row as f32 * (ROW + ROW_GAP) + above as f32 * HEADING, ROW)
            }
        }
    }

    /// Where the highlight rests on item `index`.
    fn highlight(&self, index: usize) -> (f32, f32) {
        match self {
            Self::Grid => (
                (index % COLS) as f32 * CELL + BOX_X,
                (index / COLS) as f32 * CELL + BOX_Y,
            ),
            Self::List { .. } => (0.0, self.span(index).0),
        }
    }

    /// How long `count` rows or apps are, top to bottom.
    fn content(&self, count: usize) -> f32 {
        match self {
            Self::Grid => count.div_ceil(COLS).max(1) as f32 * CELL,
            Self::List { headings, .. } => {
                count as f32 * (ROW + ROW_GAP) + headings.len() as f32 * HEADING
            }
        }
    }

    /// How many items Up and Down move by.
    fn row(&self) -> usize {
        match self {
            Self::Grid => COLS,
            Self::List { .. } => 1,
        }
    }

    /// How many items are drawn over the list rather than in it.
    fn skip(&self) -> usize {
        match self {
            Self::Grid => 0,
            Self::List { skip, .. } => *skip,
        }
    }
}

/// Where `rows` start a new section: a heading goes over each.
fn headings(rows: &[Entry]) -> Vec<usize> {
    let mut last: Option<&str> = None;
    let mut at = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let section = row.section.as_deref();
        if section.is_some() && section != last {
            at.push(index);
        }
        last = section;
    }
    at
}

/// What the launcher shows this moment, worked out once for the view, a
/// key or Enter.
struct View<'a> {
    all: &'a [DesktopApp],
    field: Field,
    mode: Mode,
    scope: Option<Scope>,
    kind: Kind,
    // what the search found, or Applications' apps as rows
    found: Found,
    // the categories with apps under them, the one showing, and its apps
    // (the suggested ones first on the first page)
    buckets: Vec<usize>,
    page: usize,
    suggested: Vec<&'a DesktopApp>,
    shown: Vec<&'a DesktopApp>,
    apps_layout: AppsLayout,
    selected: usize,
    focus: Zone,
}

impl<'a> View<'a> {
    fn new(all: &'a [DesktopApp]) -> Self {
        let (field, mode, scope, selected, focus) = with(|state| {
            (
                state.field.clone(),
                state.mode,
                state.scope.clone(),
                state.selected,
                state.focus,
            )
        });
        let typed = !field.text().trim().is_empty();
        let settings = launcher();
        let apps_layout = settings.apps_layout;
        let kind = match mode {
            _ if scope.is_some() => Kind::Results,
            Mode::Home if typed => Kind::Results,
            Mode::Home => Kind::Idle,
            Mode::Apps if typed => Kind::Results,
            Mode::Apps => Kind::Apps,
            Mode::Actions => Kind::Results,
            Mode::Clipboard if settings.clipboard.enabled => Kind::Results,
            Mode::Clipboard => Kind::Ask,
            Mode::Files => Kind::Empty,
        };

        let buckets = model::active_buckets(
            all.iter()
                .map(|app| model::mask(app.categories(), app.exec())),
        );
        let page = PAGER
            .with_borrow(Pager::page)
            .min(buckets.len().saturating_sub(1));
        let mut found = Found::default();
        let mut suggested = Vec::new();
        let mut shown = Vec::new();
        match kind {
            Kind::Results => {
                let query = Query::new(field.text(), mode, all);
                found = provider::search(&query, scope.as_ref(), &settings.excluded_providers);
            }
            Kind::Apps => {
                suggested = suggestions(all);
                shown = apps_on(all, &buckets, page, &suggested);
                if apps_layout == AppsLayout::List {
                    found.rows = shown.iter().map(|app| Entry::app(app)).collect();
                }
            }
            Kind::Idle | Kind::Empty | Kind::Ask => {}
        }
        drop(settings);

        let mut view = Self {
            all,
            field,
            mode,
            scope,
            kind,
            found,
            buckets,
            page,
            suggested,
            shown,
            apps_layout,
            selected,
            focus,
        };
        view.selected = selected.min(view.count().saturating_sub(1));
        view
    }

    /// Whether the body is the grid of apps.
    fn grid(&self) -> bool {
        self.kind == Kind::Apps && self.apps_layout == AppsLayout::Grid
    }

    fn layout(&self) -> Layout {
        if self.grid() {
            Layout::Grid
        } else {
            Layout::List {
                skip: usize::from(self.found.answer.is_some()),
                headings: headings(&self.found.rows).into(),
            }
        }
    }

    fn count(&self) -> usize {
        match self.kind {
            _ if self.grid() => self.shown.len(),
            Kind::Results | Kind::Apps => {
                usize::from(self.found.answer.is_some()) + self.found.rows.len()
            }
            Kind::Idle | Kind::Empty | Kind::Ask => 0,
        }
    }

    /// The entry item `index` is: the answer, then the rows.
    fn entry(&self, index: usize) -> Option<&Entry> {
        match &self.found.answer {
            Some(answer) if index == 0 => Some(answer),
            Some(_) => self.found.rows.get(index - 1),
            None => self.found.rows.get(index),
        }
    }

    /// Whether the panel has the category pills: Applications, unsearched.
    fn pills(&self) -> bool {
        self.kind == Kind::Apps
    }

    /// How wide the panel is, and what's on it inside its inset.
    fn width(&self) -> f32 {
        if matches!(self.mode, Mode::Apps | Mode::Clipboard) {
            APPS_W
        } else {
            PILL_W
        }
    }

    /// How wide the list is: Clipboard shares its width with a preview of
    /// the copy picked.
    fn list_width(&self) -> f32 {
        if self.mode == Mode::Clipboard {
            CLIP_LIST_W
        } else {
            self.inner()
        }
    }

    fn inner(&self) -> f32 {
        self.width() - INSET * 2.0
    }

    /// How tall the list of results is in view.
    fn list_height(&self) -> f32 {
        // Applications and Actions keep one size, however much they list
        if self.kind == Kind::Apps {
            return BODY;
        }
        if matches!(self.mode, Mode::Actions | Mode::Clipboard) {
            return MAX_ROWS as f32 * (ROW + ROW_GAP) - ROW_GAP;
        }
        let rows = self.found.rows.len().clamp(1, MAX_ROWS);
        rows as f32 * (ROW + ROW_GAP) - ROW_GAP
    }

    /// How tall the panel is: the search row, then what's under it.
    fn height(&self) -> f32 {
        let under = match self.kind {
            Kind::Idle => return PILL_H,
            Kind::Empty | Kind::Ask => EMPTY_H,
            Kind::Apps => PILLS + GAP + BODY + INSET * 2.0,
            Kind::Results => {
                let answer = if self.found.answer.is_some() {
                    ANSWER_H + GAP
                } else {
                    0.0
                };
                let chips = if self.found.scopes.is_empty() {
                    0.0
                } else {
                    CHIP_H + GAP
                };
                answer + chips + self.list_height() + INSET * 2.0
            }
        };
        PILL_H + Divider::height() + under
    }
}

/// The apps used most, filled out to a row with the first of the rest:
/// none until any has been used from the launcher.
fn suggestions(all: &[DesktopApp]) -> Vec<&DesktopApp> {
    let top = usage::read(|usage| usage.top("app:", COLS, usage::now()));
    let mut picked: Vec<&DesktopApp> = top
        .iter()
        .filter_map(|key| all.iter().find(|app| entries::app_key(app) == *key))
        .collect();
    if picked.is_empty() {
        return picked;
    }
    for app in all {
        if picked.len() >= COLS {
            break;
        }
        if !picked.iter().any(|shown| shown.id() == app.id()) {
            picked.push(app);
        }
    }
    picked
}

/// The apps on category page `page`, the suggested ones over the first.
fn apps_on<'a>(
    all: &'a [DesktopApp],
    buckets: &[usize],
    page: usize,
    suggested: &[&'a DesktopApp],
) -> Vec<&'a DesktopApp> {
    let mut apps = if page == 0 && suggested.len() == COLS {
        suggested.to_vec()
    } else {
        Vec::new()
    };
    apps.extend(under(all, bucket(buckets, page)));
    apps
}

/// The category on page `page`: All Applications while none are known.
fn bucket(buckets: &[usize], page: usize) -> usize {
    buckets.get(page).copied().unwrap_or(0)
}

/// The apps under category `bucket`, in the list's order.
fn under(apps: &[DesktopApp], bucket: usize) -> Vec<&DesktopApp> {
    apps.iter()
        .filter(|app| model::in_bucket(model::mask(app.categories(), app.exec()), bucket))
        .collect()
}

// ───── runtime ───────────────────────────────────────────────────────────────

pub(crate) fn opened() {}

pub(crate) fn closed() {}

/// Gone from the screen: the next opening starts afresh, on the pill.
fn hidden() {
    with(|state| *state = State::new());
    FRESH.set(true);
    SCROLL.with_borrow_mut(|scroll| *scroll = Kinetic::new());
    PILL_SCROLL.with_borrow_mut(|scroll| *scroll = Kinetic::new());
    PAGER.with_borrow_mut(|pager| pager.jump(0, &PAGING));
    SWITCH.with_borrow_mut(|switch| *switch = motion::fade(1.0, motion::FADE));
    SCROLLBAR.with_borrow_mut(|bar| *bar = motion::fade(0.0, motion::FADE));
}

/// `amane-shell` IPC: `launcher [toggle | open | close] [mode]`.
pub fn ipc(args: &[String]) -> String {
    const USAGE: &str =
        "usage: launcher [toggle | open | close] [apps | files | actions | clipboard]";
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (verb, mode) = match args.as_slice() {
        [] => ("toggle", None),
        [verb] => (*verb, None),
        [verb, name] => match Mode::named(name) {
            Some(mode) => (*verb, Some(mode)),
            None => return String::from(USAGE),
        },
        _ => return String::from(USAGE),
    };
    let open = is_open(Panel::Launcher);
    match (verb, mode) {
        ("toggle", None) => panel::toggle(Panel::Launcher),
        // the mode showing closes; another switches to it
        ("toggle", Some(mode)) if open && with(|state| state.mode) == mode => close(),
        ("toggle" | "open", Some(mode)) => {
            switch_mode(mode);
            if !open {
                panel::toggle(Panel::Launcher);
            }
        }
        ("open", None) => {
            if !open {
                panel::toggle(Panel::Launcher);
            }
        }
        ("close", None) => {
            if open {
                close();
            }
        }
        _ => return String::from(USAGE),
    }
    mark_all();
    String::from("ok")
}

/// Switches to `mode`, from a fresh search; the one showing again goes
/// back to the pill.
fn switch_mode(mode: Mode) {
    let to = with(|state| {
        let to = if state.mode == mode { Mode::Home } else { mode };
        state.mode = to;
        state.field = Field::new();
        state.scope = None;
        state.selected = 0;
        state.history_at = None;
        state.hover_mode = None;
        state.back_hover = false;
        state.focus = if to == Mode::Apps {
            Zone::Body
        } else {
            Zone::Search
        };
        to
    });
    if to == Mode::Apps {
        PAGER.with_borrow_mut(|pager| pager.jump(0, &PAGING));
    }
    SCROLL.with_borrow_mut(|scroll| scroll.jump(0.0));
}

/// Narrows the search to `scope`, from an empty query.
fn enter_scope(scope: Scope) {
    with(|state| {
        state.scope = Some(scope);
        state.field = Field::new();
        state.selected = 0;
        state.history_at = None;
        state.focus = Zone::Search;
    });
    SCROLL.with_borrow_mut(|scroll| scroll.jump(0.0));
    mark_all();
}

/// Steps back one level: the query, then the scope, then the mode; with
/// none of them, closes the launcher.
fn back() {
    let (typed, scoped, mode) = with(|state| {
        (
            !state.field.text().is_empty(),
            state.scope.is_some(),
            state.mode,
        )
    });
    if typed {
        type_into(|field| {
            *field = Field::new();
            true
        });
    } else if scoped {
        with(|state| {
            state.scope = None;
            state.selected = 0;
        });
    } else if mode != Mode::Home {
        switch_mode(Mode::Home);
    } else {
        close();
    }
}

/// Starts typing quick keys for the action `action`, named `title`, from
/// the ones it has, `keys`.
fn start_capture(action: &str, title: &str, keys: &str) {
    with(|state| {
        state.capture = Some(Capture {
            action: action.to_string(),
            title: title.to_string(),
            keys: keys.to_string(),
            error: None,
        });
    });
    mark_all();
}

/// A key while quick keys are being typed: letters and digits add to
/// them, Backspace takes one back, Return saves them (none clears them)
/// and Escape gives up.
fn capture_key(key: Key) {
    let Some(mut capture) = with(|state| state.capture.clone()) else {
        return;
    };
    match key {
        Key::Character(c) if c.is_ascii_alphanumeric() => {
            if capture.keys.chars().count() < model::MAX_QUICK_KEYS {
                capture.keys.push(c.to_ascii_lowercase());
            }
            capture.error = None;
        }
        Key::Backspace => {
            capture.keys.pop();
            capture.error = None;
        }
        Key::Escape => {
            with(|state| state.capture = None);
            mark_all();
            return;
        }
        Key::Enter => {
            let taken = launcher()
                .quick_keys
                .get(&capture.keys)
                .filter(|key| key.action != capture.action)
                .map(|key| key.title.clone());
            if let Some(owner) = taken {
                capture.error = Some(format!("Used by {owner}"));
            } else if capture.keys.is_empty() || model::valid_quick_keys(&capture.keys) {
                let Capture {
                    action,
                    title,
                    keys,
                    ..
                } = capture;
                change_launcher(|settings| {
                    settings.quick_keys.retain(|_, key| key.action != action);
                    if !keys.is_empty() {
                        settings.quick_keys.insert(keys, QuickKey { action, title });
                    }
                });
                with(|state| state.capture = None);
                mark_all();
                return;
            }
        }
        _ => return,
    }
    with(|state| state.capture = Some(capture));
    mark_all();
}

/// The body moved: the scrollbar shows for a while.
fn moved() {
    with(|state| state.scrolled = Some(Instant::now()));
}

/// A scroll over the body: a wheel scrolls it, a sideways tilt turns the
/// category, and fingers do either, whichever way they set out.
fn on_body_scroll(scroll: Scroll) {
    let (paging, pages) = with(|state| (state.shown.0 == Kind::Apps, state.pages));
    match scroll.source {
        ScrollSource::Wheel => {
            SCROLL.with_borrow_mut(|body| body.scroll(&scroll, Axis::Vertical, WHEEL_STEP));
            moved();
        }
        ScrollSource::Tilt if paging => {
            PAGER.with_borrow_mut(|pager| pager.step(scroll.x, pages, &PAGING));
        }
        ScrollSource::Tilt => {}
        ScrollSource::Finger | ScrollSource::Continuous => {
            let (x, y) = scroll.pixels();
            let now = Instant::now();
            let picked = with(|state| {
                let picked = state.gesture.feed(x, y, paging && pages > 1, now);
                if scroll.stop {
                    state.gesture.release();
                }
                picked
            });
            match picked {
                Some((Axis::Vertical, along)) => {
                    SCROLL.with_borrow_mut(|body| body.follow(along, scroll.stop, now));
                    moved();
                }
                Some((Axis::Horizontal, along)) => PAGER.with_borrow_mut(|pager| {
                    pager.follow(along, scroll.stop, GRID_W, pages, &PAGING);
                }),
                // fingers lifting before the gesture picked a way still end
                // whatever they'd started
                None if scroll.stop => {
                    SCROLL.with_borrow_mut(|body| body.follow(0.0, true, now));
                    PAGER.with_borrow_mut(|pager| pager.follow(0.0, true, GRID_W, pages, &PAGING));
                }
                None => {}
            }
        }
    }
    mark_all();
}

/// The pill bar follows a sideways swipe, or a wheel turned over it.
fn on_pill_scroll(scroll: Scroll) {
    PILL_SCROLL.with_borrow_mut(|pills| match scroll.source {
        ScrollSource::Finger | ScrollSource::Continuous => {
            pills.scroll(&scroll, Axis::Horizontal, WHEEL_STEP)
        }
        ScrollSource::Wheel | ScrollSource::Tilt => {
            let lines = if scroll.x.abs() > f32::EPSILON {
                scroll.x
            } else {
                scroll.y
            };
            let to = pills.target() + lines * WHEEL_STEP;
            pills.go(to);
        }
    });
    mark_all();
}

/// Highlights item `index` of `layout`, scrolling it into view in a body
/// `height` tall.
fn select(index: usize, layout: &Layout, height: f32) {
    with(|state| state.selected = index);
    let (top, span) = layout.span(index);
    SCROLL.with_borrow_mut(|scroll| {
        let at = scroll.target();
        if top < at {
            scroll.go(top);
        } else if top + span > at + height {
            scroll.go(top + span - height);
        }
    });
}

/// Hovering an app or entry highlights it, and brings the focus to them.
fn hover(index: usize) -> impl Fn(bool) + 'static {
    move |inside| {
        if inside
            && with(|state| {
                state.focus = Zone::Body;
                std::mem::replace(&mut state.selected, index)
            }) != index
        {
            mark_all();
        }
    }
}

/// Turns to category page `page`: sliding there from the next one, or
/// fading in from further away rather than flashing every one between.
fn turn_to(page: usize) {
    let pages = with(|state| state.pages);
    PAGER.with_borrow_mut(|pager| {
        if pager.page().abs_diff(page) > 1 {
            pager.jump(page, &PAGING);
            SWITCH.with_borrow_mut(|switch| *switch = motion::fade(0.0, motion::FADE));
        } else {
            pager.go(page, pages, &PAGING);
        }
    });
    mark_all();
}

/// Edits the search text; a change starts the list over at the top. The
/// keys typed go to the search field.
fn type_into(edit: impl FnOnce(&mut Field) -> bool) {
    let changed = with(|state| {
        let before = state.field.text().to_string();
        let changed = edit(&mut state.field) || state.field.text() != before;
        state.focus = Zone::Search;
        if changed {
            state.selected = 0;
            state.history_at = None;
        }
        changed
    });
    if changed {
        SCROLL.with_borrow_mut(|scroll| scroll.go(0.0));
    }
}

/// Up and Down through the searches run before, while the field is empty
/// or holds one of them: `older` goes back one, else forward to the empty
/// field. Whether there was one to go to.
fn walk_history(older: bool) -> bool {
    let history: Vec<String> = usage::read(|usage| usage.history().iter().cloned().collect());
    with(|state| {
        let Some(to) = model::history_step(state.history_at, older, history.len()) else {
            return false;
        };
        state.history_at = to;
        let mut field = Field::new();
        if let Some(at) = to {
            field.insert(&history[at]);
        }
        state.field = field;
        state.selected = 0;
        state.focus = Zone::Search;
        true
    })
}

/// What Enter runs: the highlighted app or entry, else the first.
fn run_selected() {
    let apps = Apps::read();
    let view = View::new(apps.list());
    let query = view.field.text().to_string();
    let entry = if view.grid() {
        view.shown
            .get(view.selected)
            .or(view.shown.first())
            .map(|app| Entry::app(app))
    } else {
        view.entry(view.selected).or(view.entry(0)).cloned()
    };
    drop(view);
    drop(apps);
    if let Some(entry) = entry {
        entries::run(&entry, &query);
    }
}

/// Every key: nothing else on the panel takes the focus.
fn on_key(key: Key) {
    if with(|state| state.capture.is_some()) {
        capture_key(key);
        return;
    }
    let apps = Apps::read();
    let view = View::new(apps.list());
    let held = modifiers();
    let typed = !view.field.text().is_empty();
    let listing = !view.grid();

    // a chord isn't text: the editing ones edit, Ctrl+1 … 4 pick a mode,
    // and the rest go nowhere
    if held.ctrl
        && let Key::Character(letter) = key
    {
        match letter.to_ascii_lowercase() {
            'a' if typed => with(|state| state.field.select_all()),
            'c' if typed => text_edit::copy(&view.field.selected()),
            'x' if typed => {
                text_edit::copy(&view.field.selected());
                type_into(Field::delete_selection);
            }
            'v' => {
                if let Some(text) = text_edit::paste() {
                    type_into(|field| {
                        field.insert(&text);
                        true
                    });
                }
            }
            // the keyboard's way to set the picked action's quick keys
            'k' => {
                if let Some(entry) = view.entry(view.selected)
                    && let Some(title) = &entry.action_title
                    && !matches!(entry.action, entries::Action::Hint)
                {
                    start_capture(&entry.key, title, entry.quick_keys.as_deref().unwrap_or(""));
                }
            }
            digit => {
                if let Some(mode) = Mode::from_digit(digit) {
                    switch_mode(mode);
                }
            }
        }
        mark_all();
        return;
    }
    if held.command() && matches!(key, Key::Character(_) | Key::Space) {
        return;
    }

    let layout = view.layout();
    let count = view.count();
    let selected = view.selected;
    let height = view.list_height();
    let set_focus = |zone| with(|state| state.focus = zone);
    let history = with(|state| state.history_at.is_some());
    let fresh = view.mode == Mode::Home && view.scope.is_none() && (!typed || history);

    match (key, view.focus) {
        (Key::Character(letter), _) => type_into(|field| {
            field.insert(&letter.to_string());
            true
        }),
        // a search doesn't start with a space
        (Key::Space, _) if typed => type_into(|field| {
            field.insert(" ");
            true
        }),
        // Backspace in an empty field leaves the scope
        (Key::Backspace, _) if !typed && view.scope.is_some() => back(),
        (Key::Backspace, _) => type_into(Field::backspace),
        // Tab narrows to the row picked, when it narrows to anything
        (Key::Tab, _) if !held.shift && listing => {
            match view.entry(selected).and_then(|entry| entry.scope.clone()) {
                Some(scope) => {
                    drop(view);
                    drop(apps);
                    enter_scope(scope);
                }
                None => set_focus(view.focus.next(false, view.pills())),
            }
        }
        (Key::Tab, focus) => set_focus(focus.next(held.shift, view.pills())),
        (Key::Escape, _) => {
            drop(view);
            drop(apps);
            back();
        }
        (Key::Enter, Zone::Pills) => set_focus(Zone::Body),
        // Shift+Return puts a copy back on the clipboard, without pasting it
        (Key::Enter, _) if held.shift && view.mode == Mode::Clipboard => {
            if let Some(entries::Action::Paste(clip)) =
                view.entry(selected).map(|entry| entry.action.clone())
            {
                close();
                clipboard_history::put_back(&clip, false);
            }
        }
        (Key::Enter, _) => {
            drop(view);
            drop(apps);
            run_selected();
        }
        (Key::Delete, _) if view.mode == Mode::Clipboard => {
            if let Some(entries::Action::Paste(clip)) =
                view.entry(selected).map(|entry| entry.action.clone())
            {
                clipboard_history::delete(clip.id);
            }
        }

        // the searches run before, from an empty field
        (Key::Up, Zone::Search) if fresh && walk_history(true) => {}
        (Key::Down, Zone::Search) if fresh && history => {
            walk_history(false);
        }

        // while listing, Left and Right are the caret's and Up and Down
        // the list's, wherever the focus is
        (Key::Left, _) if listing && typed => with(|state| state.field.left(held.shift)),
        (Key::Right, _) if listing && typed => with(|state| state.field.right(held.shift)),
        (Key::Home, _) if listing && typed => with(|state| state.field.move_to(0, held.shift)),
        (Key::End, _) if listing && typed => {
            with(|state| state.field.move_to(usize::MAX, held.shift))
        }

        (Key::Down, Zone::Search) if view.pills() => set_focus(Zone::Pills),
        (Key::Down, Zone::Search) if count > 0 => {
            set_focus(Zone::Body);
            select((selected + 1).min(count - 1), &layout, height);
        }

        (Key::Left, Zone::Pills) => turn_to(view.page.saturating_sub(1)),
        (Key::Right, Zone::Pills) => {
            turn_to((view.page + 1).min(view.buckets.len().saturating_sub(1)))
        }
        (Key::Home, Zone::Pills) => turn_to(0),
        (Key::End, Zone::Pills) => turn_to(view.buckets.len().saturating_sub(1)),
        (Key::Up, Zone::Pills) => set_focus(Zone::Search),
        (Key::Down, Zone::Pills) => set_focus(Zone::Body),

        // off the top of the apps or rows, up to what's over them
        (Key::Up, Zone::Body) if selected < layout.row() => {
            if selected > 0 {
                select(0, &layout, height);
            }
            set_focus(if view.pills() {
                Zone::Pills
            } else {
                Zone::Search
            })
        }
        (Key::Left, Zone::Body) if count > 0 => select(selected.saturating_sub(1), &layout, height),
        (Key::Right, Zone::Body) if count > 0 => {
            select((selected + 1).min(count - 1), &layout, height)
        }
        (Key::Up, Zone::Body) if count > 0 => {
            select(selected.saturating_sub(layout.row()), &layout, height)
        }
        (Key::Down, Zone::Body) if count > 0 => {
            select((selected + layout.row()).min(count - 1), &layout, height)
        }
        (Key::Home, Zone::Body) if count > 0 => select(0, &layout, height),
        (Key::End, Zone::Body) if count > 0 => select(count - 1, &layout, height),
        (Key::Up, Zone::Search) if listing && count > 0 => {
            select(selected.saturating_sub(1), &layout, height)
        }
        _ => return,
    }
    mark_all();
}

// ───── view ──────────────────────────────────────────────────────────────────

/// Steps the glass's opening towards `open` and gives how far it is, a
/// little past 1 on the spring. Closing is quicker than opening.
fn unfold(open: bool) -> f32 {
    OPEN.with_borrow_mut(|anim| {
        if OPENING.replace(open) != open {
            anim.retime(if open { motion::POP } else { motion::CLOSE });
            anim.to(if open { 1.0 } else { 0.0 });
        }
        anim.value()
    })
}

/// Steps `anim` towards `to` and gives where it is.
fn ease(anim: &'static std::thread::LocalKey<RefCell<Animation>>, to: f32) -> f32 {
    anim.with_borrow_mut(|anim| {
        anim.to(to);
        anim.value()
    })
}

/// The surface: hidden while closed, else the glass high on a clear
/// surface that closes the launcher when clicked.
pub fn surface() -> LayerWindow {
    // the clipboard's history follows its setting, whether or not the
    // launcher is open
    clipboard_history::sync(&launcher().clipboard);
    let open = is_open(Panel::Launcher);
    let unfolded = unfold(open);

    if !open && unfolded <= 0.001 {
        hidden();
        return window()
            .visible(false)
            .blur_shapes(Vec::new())
            .child(blank());
    }

    let apps = Apps::read();
    let view = View::new(apps.list());

    // the glass heads for where this moment's view puts it, starting there
    // when the launcher has just opened
    let screen = window_size();
    let target = shape::target(
        screen,
        (view.width(), view.height()),
        view.kind == Kind::Idle,
    );
    let geometry = GEOMETRY.with_borrow_mut(|anim| {
        if FRESH.replace(false) {
            *anim = motion::sheet(target);
        } else {
            anim.to(target);
        }
        anim.value()
    });

    // The glass swells in about the pill's centre, the frost with it. A
    // compositor that can't fade frost only shows or hides it, so there the
    // glass arrives whole rather than fading in over frost at full strength.
    let fades = amane::blur_fades();
    let scale = f32::blend(motion::scale(OPEN_SCALE), 1.0, unfolded);
    let opacity = if open && !fades {
        1.0
    } else {
        unfolded.clamp(0.0, 1.0)
    };
    let (sheet, _) = geometry.drawn();
    let centre = (sheet.x + sheet.width / 2.0, sheet.y + PILL_H / 2.0);
    let frost = if open || fades {
        glass_region::faded(
            glass_region::transformed(geometry.frost(), scale, centre, (0.0, 0.0)),
            opacity,
        )
    } else {
        Vec::new()
    };

    let mut layers: Vec<Box<dyn Widget>> = vec![boxed(glass(&view, geometry))];
    if geometry.shown() > 0.01 {
        layers.extend(header::mode_buttons(&geometry, view.mode));
    }
    drop(view);
    drop(apps);

    // a layer scales about its own centre, the surface's: moved by how far
    // that is from the pill's, scaled down, it swells from the pill
    let (screen_w, screen_h) = screen;
    let shift = (
        (centre.0 - screen_w / 2.0) * (1.0 - scale),
        (centre.1 - screen_h / 2.0) * (1.0 - scale),
    );
    let group = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .child(Stack::new(layers).width(Parent).height(Parent))
        .opacity(opacity)
        .scale(scale)
        .translate(shift.0, shift.1);

    // a click outside the glass closes it; the glass keeps its own clicks
    let outside = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .on_click(|_| close())
        .child(group);

    window()
        .visible(true)
        .blur_shapes(frost)
        .child(track_cursor(outside))
}

/// The sheet at `geometry`, and what's on it: laid out for the size it's
/// heading to and clipped to the size it is, so growing reveals the rows
/// rather than squeezing them.
fn glass(view: &View, geometry: Geometry) -> Rectangle {
    let (sheet, radius) = geometry.drawn();
    let content = Rectangle::new()
        .width(view.width())
        .height(view.height().max(sheet.height))
        .fill(Color::TRANSPARENT)
        .align_child(Start, Start)
        .child(panel(view));
    let glass = Rectangle::new()
        .width(sheet.width)
        .height(sheet.height)
        .radius(radius)
        .fill(Material::Sheet.fill())
        .clip()
        .align_child(Start, Start)
        // the glass keeps its clicks from the surface under it, which
        // closes the launcher
        .on_click(|_| {})
        .child(content);
    Material::Sheet
        .lit(glass, radius)
        .translate(sheet.x, sheet.y)
}

/// The search row and, under it, what this moment shows.
fn panel(view: &View) -> Column {
    let layout = view.layout();
    let count = view.count();
    let pages = view.buckets.len();
    let width = view.inner();

    let offset = PAGER.with_borrow_mut(|pager| {
        pager.settle_stale(pages, &PAGING);
        if pager.page() >= pages && pages > 0 {
            pager.jump(pages - 1, &PAGING);
        }
        pager.offset(pages, &PAGING)
    });

    // what the body shows: a change starts the list afresh, snapping the
    // highlight and the scroll there rather than sliding them
    let showing = (
        view.kind,
        if view.kind == Kind::Apps {
            view.page
        } else {
            0
        },
    );
    let (before, switched) = with(|state| {
        state.pages = pages;
        state.selected = view.selected;
        let before = std::mem::replace(&mut state.shown, showing);
        (before, before != showing)
    });
    let selected = if switched { 0 } else { view.selected };
    let (to_x, to_y) = layout.highlight(selected);
    if switched {
        with(|state| state.selected = 0);
        HIGHLIGHT_X.with_borrow_mut(|x| *x = motion::fade(to_x, motion::HOVER));
        HIGHLIGHT_Y.with_borrow_mut(|y| *y = motion::fade(to_y, motion::HOVER));
        SCROLL.with_borrow_mut(|scroll| scroll.jump(0.0));
        // into or out of a search, the body fades in; a category slides
        if before.0 != showing.0 {
            SWITCH.with_borrow_mut(|switch| *switch = motion::fade(0.0, motion::FADE));
        }
    }

    // how far the body scrolls
    let body_height = view.list_height();
    let scroll = SCROLL.with_borrow_mut(|scroll| {
        scroll.set_max(layout.content(count.saturating_sub(layout.skip())) - body_height);
        scroll.settle_stale();
        if scroll.swiping() {
            moved();
        }
        scroll.value()
    });

    // Super, pressed since the launcher opened, shows the rows' fuller lines
    let held = modifiers().logo;
    let reveal = with(|state| {
        if !held {
            state.logo_armed = true;
        }
        state.logo_armed && held
    });

    // a search's results are what Enter runs, whichever zone has the keys
    let focused = view.focus == Zone::Body || view.kind == Kind::Results;
    let at = grid::At {
        scroll,
        x: ease(&HIGHLIGHT_X, to_x),
        y: ease(&HIGHLIGHT_Y, to_y),
        highlight: ease(&HIGHLIGHT_ALPHA, if focused { 1.0 } else { UNFOCUSED }),
        fade: ease(&SWITCH, 1.0),
        scrollbar: scrollbar(),
        selected,
        reveal,
    };

    let (hover_mode, back_hover) = with(|state| (state.hover_mode, state.back_hover));
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(header::search_row(
        view,
        view.focus == Zone::Search,
        hover_mode,
        back_hover,
    ))];
    if view.kind == Kind::Idle {
        return Column::new(parts).width(view.width());
    }
    parts.push(boxed(Divider::build()));

    let query = view.field.text().to_string();
    let mut body: Vec<Box<dyn Widget>> = Vec::new();
    match view.kind {
        Kind::Results => {
            if let Some(answer) = &view.found.answer {
                body.push(boxed(results::answer_card(
                    answer,
                    width,
                    selected == 0,
                    &query,
                )));
            }
            if !view.found.scopes.is_empty() {
                body.push(boxed(results::scope_chips(&view.found.scopes, width)));
            }
            let list_w = view.list_width();
            let list: Box<dyn Widget> =
                if view.mode == Mode::Clipboard && view.found.rows.is_empty() {
                    boxed(results::empty(
                        if query.is_empty() {
                            results::Empty::NothingCopied
                        } else {
                            results::Empty::NoMatch
                        },
                        width,
                        body_height,
                    ))
                } else {
                    boxed(results::list(
                        &view.found.rows,
                        at,
                        list_w,
                        body_height,
                        &layout,
                        &query,
                        view.mode == Mode::Actions,
                    ))
                };
            if view.mode == Mode::Clipboard && !view.found.rows.is_empty() {
                // the copy picked, beside the list
                let preview = view.entry(selected).and_then(|entry| entry.preview.clone());
                body.push(boxed(
                    Row::new(vec![
                        list,
                        boxed(results::preview(
                            preview.as_ref(),
                            width - list_w - GAP,
                            body_height,
                        )),
                    ])
                    .gap(GAP),
                ));
            } else {
                body.push(list);
            }
        }
        Kind::Ask => body.push(boxed(results::ask_clipboard(width, EMPTY_H - INSET * 2.0))),
        Kind::Apps => {
            body.push(boxed(header::pill_bar(
                &view.buckets,
                offset,
                view.focus == Zone::Pills,
                width,
            )));
            body.push(if view.all.is_empty() {
                boxed(results::empty(results::Empty::Loading, width, BODY))
            } else if view.grid() {
                boxed(grid::pages(view, offset, at))
            } else {
                boxed(results::list(
                    &view.found.rows,
                    at,
                    width,
                    BODY,
                    &layout,
                    "",
                    false,
                ))
            });
        }
        Kind::Empty => {
            body.push(boxed(results::empty(
                results::Empty::Soon(view.mode),
                width,
                EMPTY_H - INSET * 2.0,
            )));
        }
        Kind::Idle => {}
    }
    parts.push(boxed(
        Rectangle::new()
            .width(view.width())
            .height(view.height() - PILL_H - Divider::height())
            .fill(Color::TRANSPARENT)
            .padding(pad(INSET, INSET))
            .align_child(Start, Start)
            .child(Column::new(body).gap(GAP)),
    ));
    Column::new(parts).width(view.width())
}

/// How much the scrollbar shows: while the body moves, and a little after.
fn scrollbar() -> f32 {
    let now = Instant::now();
    let (scrolled, due) = with(|state| (state.scrolled, state.fade_due));
    let showing = scrolled.is_some_and(|at| now.duration_since(at) < SCROLLBAR_IDLE);
    // amane has no timers: a frame comes when the scrollbar is due to fade
    if let Some(at) = scrolled.filter(|_| showing)
        && due.is_none_or(|due| now >= due)
    {
        let due = at + SCROLLBAR_IDLE;
        with(|state| state.fade_due = Some(due));
        motion::redraw_after(due.saturating_duration_since(now));
    }
    ease(&SCROLLBAR, if showing { 1.0 } else { 0.0 })
}

/// The surface over the output, keeping clear of what the bar reserves.
fn window() -> LayerWindow {
    let base = LayerWindow::new()
        .width(WindowSize::Full)
        .height(WindowSize::Full)
        // modal while open: every key is the launcher's from the first moment
        .keyboard(Keyboard::Exclusive)
        .on_key(on_key);
    overlay(base, LAUNCHER_NS, Vertical::Middle, Horizontal::Middle)
}

/// A click on `app`: a launch, or its menu at the pointer.
fn on_app_click(app: DesktopApp) -> impl Fn(Button) + 'static {
    move |button| match button {
        Button::Left => entries::run(&Entry::app(&app), ""),
        Button::Right => app_menu::open(&app),
        #[allow(unreachable_patterns)]
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_goes_round_the_zones() {
        assert_eq!(Zone::Body.next(false, true), Zone::Search);
        assert_eq!(Zone::Search.next(false, true), Zone::Pills);
        assert_eq!(Zone::Pills.next(false, true), Zone::Body);
        assert_eq!(Zone::Search.next(true, true), Zone::Body);
    }

    #[test]
    fn without_pills_tab_goes_between_the_field_and_the_list() {
        assert_eq!(Zone::Search.next(false, false), Zone::Body);
        assert_eq!(Zone::Body.next(false, false), Zone::Search);
        // the pills, focused before the search began, count as the field
        assert_eq!(Zone::Pills.next(false, false), Zone::Body);
    }

    #[test]
    fn the_layouts_put_items_where_they_draw() {
        assert_eq!(Layout::Grid.span(COLS + 1), (CELL, CELL));
        assert_eq!(
            Layout::Grid.highlight(COLS + 1),
            (CELL + BOX_X, CELL + BOX_Y)
        );
        let list = Layout::List {
            skip: 0,
            headings: Rc::from([]),
        };
        assert_eq!(list.span(2), (2.0 * (ROW + ROW_GAP), ROW));
        assert_eq!(Layout::Grid.content(0), CELL);
        assert_eq!(Layout::Grid.content(COLS + 1), 2.0 * CELL);
    }

    #[test]
    fn headings_push_the_rows_under_them_down() {
        let list = Layout::List {
            skip: 0,
            headings: Rc::from([0, 2]),
        };
        assert_eq!(list.span(0), (HEADING, ROW));
        assert_eq!(list.span(1), (ROW + ROW_GAP + HEADING, ROW));
        assert_eq!(list.span(2), (2.0 * (ROW + ROW_GAP) + 2.0 * HEADING, ROW));
        assert_eq!(list.content(3), 3.0 * (ROW + ROW_GAP) + 2.0 * HEADING);
    }

    #[test]
    fn a_section_starts_where_its_name_changes() {
        let row = |section: Option<&str>| {
            let mut entry = Entry::new("k", "t", entries::Icon::Glyph(' '), entries::Action::Hint);
            entry.section = section.map(String::from);
            entry
        };
        let rows = [
            row(Some("Suggestions")),
            row(Some("Suggestions")),
            row(Some("System")),
            row(None),
            row(Some("Firefox")),
        ];
        assert_eq!(headings(&rows), vec![0, 2, 4]);
    }

    #[test]
    fn an_answer_over_the_list_isnt_in_it() {
        let list = Layout::List {
            skip: 1,
            headings: Rc::from([]),
        };
        assert_eq!(list.span(0), (0.0, 0.0));
        assert_eq!(list.span(1), (0.0, ROW));
        assert_eq!(list.span(3), (2.0 * (ROW + ROW_GAP), ROW));
    }
}
