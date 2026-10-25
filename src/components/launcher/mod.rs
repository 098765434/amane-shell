//! Launcher — an app launcher after macOS Launchpad, by way of KDE's Tahoe
//! Launcher: one glass panel in the middle of the screen with a search
//! field, a row of category pills and the installed apps in a grid five
//! across and four down. One highlight glides to the app under the pointer
//! or the arrow keys.
//!
//! Two fingers move it as they would a page: up and down they scroll the
//! grid, which coasts on after a flick ([`Kinetic`]); sideways they slide
//! the next category's apps in under them, settling on a category when
//! they lift ([`Pager`]). The pills follow, the picked one's fill sliding
//! along under them.
//!
//! Typing turns the grid into a list of what the search finds
//! ([`entries`]): a sum's answer, the matching apps, the session actions
//! named, the URL typed and a web search. The launcher draws the search
//! text itself rather than focusing a `TextInput`, so every key reaches
//! it. Tab moves between the search field, the pills and the apps, and
//! the arrows move within whichever has the focus; in the list Left and
//! Right move the caret, Shift selects and Ctrl+A/C/X/V edit (`text_edit::Field`,
//! amane's `modifiers()`). Right-clicking an app opens its menu
//! ([`app_menu`]).
//!
//! The surface covers the output, clear outside the panel, so a click
//! anywhere but the panel closes it. The glass is Quick Settings' and the
//! dock's: the compositor frosts behind the panel where the surface says
//! ([`glass_region`]), or the shader draws its lit edge where the
//! compositor has no glass material. The surface keeps one size, open or
//! closing, so nothing is resized while it moves.
//!
//! It shares the notification center's and Quick Settings'
//! one-open-at-a-time state ([`Panel::Launcher`]), opened over IPC:
//! `launcher [toggle | open | close]`, which a niri keybind sends. See
//! `docs/launcher.md`.

mod app_menu;
mod entries;
mod grid;
mod header;
mod model;
mod results;

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use amane::{
    Animation, Apps, Blend, Button, Center, Color, Column, DesktopApp, Easing, Horizontal, Key,
    Keyboard, LayerWindow, Parent, Rectangle, Scroll, ScrollSource, Service, Start, Vertical,
    Widget, WindowSize, mark_all, modifiers, window_size,
};

use crate::components::divider::Divider;
use crate::components::glass_region::{self, Thickness};
use crate::components::kinetic::{Axis, AxisLock, Kinetic};
use crate::components::layout::{blank, boxed, pad, track_cursor};
use crate::components::motion;
use crate::components::pager::{Pager, PagerMotion};
use crate::components::panel::{self, Panel, close, is_open, overlay};

use entries::Entry;

use crate::components::text_edit::{self, Field};

/// Layer-surface namespace; the niri `layer-rule` matches it (a clear
/// surface that says where to frost).
pub const LAUNCHER_NS: &str = "amane.launcher";

// TODO(token): `launcher.grid.*`, `launcher.panel.*`, `launcher.result.*`,
// `launcher.open.scale` and `launcher.motion.*` once Penpot has them
// (manual §1.1).
/// Apps across the grid, and rows of them in view.
const COLS: usize = 5;
const ROWS: usize = 4;
/// An app's square cell, its icon, and the highlight around the icon.
const CELL: f32 = 150.0;
const ICON: f32 = 64.0;
const ICON_BOX: f32 = 84.0;
/// The room between the panel's edge and what's on it.
const INSET: f32 = 15.0;
/// The panel's corners.
const RADIUS: f32 = 22.0;
/// How tall the search row and the pill bar are.
const HEADER: f32 = 40.0;
const PILLS: f32 = 40.0;
/// A result row, and the gap under it.
const ROW: f32 = 56.0;
const ROW_GAP: f32 = 6.0;
/// How far one wheel notch scrolls.
const WHEEL_STEP: f32 = 60.0;
/// How small the panel is as it swells in.
const OPEN_SCALE: f32 = 0.96;
/// How the categories turn under the fingers: the dock's paging, a touch
/// calmer for a page as wide as the panel.
const PAGING: PagerMotion = PagerMotion {
    duration: Duration::from_millis(450),
    easing: Easing::Spring { bounce: 0.08 },
    flick: 1.2,
    rubber_band: Some(0.3),
};
/// How long the scrollbar stays after the grid last moved.
const SCROLLBAR_IDLE: Duration = Duration::from_millis(800);
/// How strongly the highlight shows while the focus is elsewhere.
const UNFOCUSED: f32 = 0.45;

/// How wide what's on the panel is: the grid's width.
const WIDTH: f32 = CELL * COLS as f32;
/// How tall the grid, or the list in its place, is in view.
const BODY: f32 = CELL * ROWS as f32;
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
    /// The next zone Tab goes to (`back` for Shift+Tab). While searching
    /// there are no pills: focused from before, they count as the field.
    fn next(self, back: bool, searching: bool) -> Self {
        let order: &[Self] = if searching {
            &[Self::Search, Self::Body]
        } else {
            &[Self::Search, Self::Pills, Self::Body]
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
    // the app or entry highlighted
    selected: usize,
    focus: Zone,
    // what the last frame showed, search results or a category's page: a
    // change snaps the highlight and the scroll rather than slide them
    // between lists that have nothing to do with each other
    shown: (bool, usize),
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
            selected: 0,
            focus: Zone::Body,
            shown: (false, 0),
            pages: 0,
            pills_for: None,
            gesture: AxisLock::DEFAULT,
            scrolled: None,
            fade_due: None,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::new()) };

    // 0 closed … 1 open: the panel fades and swells in
    static OPEN: RefCell<Animation> = RefCell::new(motion::pop(0.0));

    // whether OPEN is heading for 1
    static OPENING: Cell<bool> = const { Cell::new(false) };

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

/// How the body lays its items out: the grid, or the list of results.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Layout {
    Grid,
    List,
}

impl Layout {
    /// Where item `index` is down the body: its top and its height.
    fn span(self, index: usize) -> (f32, f32) {
        match self {
            Self::Grid => ((index / COLS) as f32 * CELL, CELL),
            Self::List => (index as f32 * (ROW + ROW_GAP), ROW),
        }
    }

    /// Where the highlight rests on item `index`.
    fn highlight(self, index: usize) -> (f32, f32) {
        match self {
            Self::Grid => (
                (index % COLS) as f32 * CELL + BOX_X,
                (index / COLS) as f32 * CELL + BOX_Y,
            ),
            Self::List => (0.0, index as f32 * (ROW + ROW_GAP)),
        }
    }

    /// How long `count` items are, top to bottom.
    fn content(self, count: usize) -> f32 {
        match self {
            Self::Grid => count.div_ceil(COLS).max(1) as f32 * CELL,
            Self::List => count as f32 * (ROW + ROW_GAP),
        }
    }

    /// How many items Up and Down move by.
    fn row(self) -> usize {
        match self {
            Self::Grid => COLS,
            Self::List => 1,
        }
    }
}

/// What the launcher shows this moment, worked out once for the view, a
/// key or Enter.
struct View<'a> {
    all: &'a [DesktopApp],
    field: Field,
    searching: bool,
    // what the search found
    found: Vec<Entry>,
    // the categories with apps under them, the one showing, and its apps
    buckets: Vec<usize>,
    page: usize,
    shown: Vec<&'a DesktopApp>,
    selected: usize,
    focus: Zone,
}

impl<'a> View<'a> {
    fn new(all: &'a [DesktopApp]) -> Self {
        let (field, selected, focus) =
            with(|state| (state.field.clone(), state.selected, state.focus));
        let searching = !field.text().trim().is_empty();
        let buckets = model::active_buckets(
            all.iter()
                .map(|app| model::mask(app.categories(), app.exec())),
        );
        let page = PAGER
            .with_borrow(Pager::page)
            .min(buckets.len().saturating_sub(1));
        let (found, shown) = if searching {
            (entries::build(field.text(), all), Vec::new())
        } else {
            (Vec::new(), under(all, bucket(&buckets, page)))
        };
        let mut view = Self {
            all,
            field,
            searching,
            found,
            buckets,
            page,
            shown,
            selected,
            focus,
        };
        view.selected = selected.min(view.count().saturating_sub(1));
        view
    }

    fn layout(&self) -> Layout {
        if self.searching {
            Layout::List
        } else {
            Layout::Grid
        }
    }

    fn count(&self) -> usize {
        if self.searching {
            self.found.len()
        } else {
            self.shown.len()
        }
    }
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

/// Gone from the screen: the next opening starts afresh, on all the apps.
fn hidden() {
    with(|state| *state = State::new());
    SCROLL.with_borrow_mut(|scroll| *scroll = Kinetic::new());
    PILL_SCROLL.with_borrow_mut(|scroll| *scroll = Kinetic::new());
    PAGER.with_borrow_mut(|pager| pager.jump(0, &PAGING));
    SWITCH.with_borrow_mut(|switch| *switch = motion::fade(1.0, motion::FADE));
    SCROLLBAR.with_borrow_mut(|bar| *bar = motion::fade(0.0, motion::FADE));
}

/// `amane-shell` IPC: `launcher [toggle | open | close]`.
pub fn ipc(args: &[String]) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["toggle"] => panel::toggle(Panel::Launcher),
        ["open"] => {
            if !is_open(Panel::Launcher) {
                panel::toggle(Panel::Launcher);
            }
        }
        ["close"] => {
            if is_open(Panel::Launcher) {
                close();
            }
        }
        _ => return String::from("usage: launcher [toggle | open | close]"),
    }
    mark_all();
    String::from("ok")
}

/// Closes the launcher and starts `app`.
fn launch(app: &DesktopApp) {
    close();
    app.launch();
}

/// The body moved: the scrollbar shows for a while.
fn moved() {
    with(|state| state.scrolled = Some(Instant::now()));
}

/// A scroll over the body: a wheel scrolls it, a sideways tilt turns the
/// category, and fingers do either, whichever way they set out.
fn on_body_scroll(scroll: Scroll) {
    let (searching, pages) = with(|state| (!state.field.text().trim().is_empty(), state.pages));
    match scroll.source {
        ScrollSource::Wheel => {
            SCROLL.with_borrow_mut(|body| body.scroll(&scroll, Axis::Vertical, WHEEL_STEP));
            moved();
        }
        ScrollSource::Tilt if !searching => {
            PAGER.with_borrow_mut(|pager| pager.step(scroll.x, pages, &PAGING));
        }
        ScrollSource::Tilt => {}
        ScrollSource::Finger | ScrollSource::Continuous => {
            let (x, y) = scroll.pixels();
            let now = Instant::now();
            let picked = with(|state| {
                let picked = state.gesture.feed(x, y, !searching && pages > 1, now);
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
                    pager.follow(along, scroll.stop, WIDTH, pages, &PAGING);
                }),
                // fingers lifting before the gesture picked a way still end
                // whatever they'd started
                None if scroll.stop => {
                    SCROLL.with_borrow_mut(|body| body.follow(0.0, true, now));
                    PAGER.with_borrow_mut(|pager| pager.follow(0.0, true, WIDTH, pages, &PAGING));
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

/// Highlights item `index` of `layout`, scrolling it into view.
fn select(index: usize, layout: Layout) {
    with(|state| state.selected = index);
    let (top, height) = layout.span(index);
    SCROLL.with_borrow_mut(|scroll| {
        let at = scroll.target();
        if top < at {
            scroll.go(top);
        } else if top + height > at + BODY {
            scroll.go(top + height - BODY);
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
        }
        changed
    });
    if changed {
        SCROLL.with_borrow_mut(|scroll| scroll.go(0.0));
    }
}

/// What Enter runs: the highlighted app or entry, else the first.
fn run_selected() {
    let apps = Apps::read();
    let view = View::new(apps.list());
    if view.searching {
        let Some(entry) = view.found.get(view.selected).or(view.found.first()) else {
            return;
        };
        let action = entry.action.clone();
        drop(view);
        drop(apps);
        entries::run_action(&action);
    } else if let Some(app) = view.shown.get(view.selected).or(view.shown.first()) {
        let app = (*app).clone();
        drop(view);
        drop(apps);
        launch(&app);
    }
}

/// Every key: nothing else on the panel takes the focus.
fn on_key(key: Key) {
    let apps = Apps::read();
    let view = View::new(apps.list());
    let held = modifiers();
    let searching = view.searching;

    // a chord isn't text: the editing ones edit, the rest go nowhere
    if held.ctrl
        && let Key::Character(letter) = key
    {
        match letter.to_ascii_lowercase() {
            'a' if searching => with(|state| state.field.select_all()),
            'c' if searching => text_edit::copy(&view.field.selected()),
            'x' if searching => {
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
            _ => {}
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
    let set_focus = |zone| with(|state| state.focus = zone);

    match (key, view.focus) {
        (Key::Character(letter), _) => type_into(|field| {
            field.insert(&letter.to_string());
            true
        }),
        // a search doesn't start with a space
        (Key::Space, _) if !view.field.text().is_empty() => type_into(|field| {
            field.insert(" ");
            true
        }),
        (Key::Backspace, _) => type_into(Field::backspace),
        (Key::Tab, focus) => set_focus(focus.next(held.shift, searching)),
        // a search clears first, then the launcher closes
        (Key::Escape, _) if searching => {
            type_into(|field| {
                *field = Field::new();
                true
            });
            set_focus(Zone::Body);
        }
        (Key::Escape, _) => close(),
        (Key::Enter, Zone::Pills) => set_focus(Zone::Body),
        (Key::Enter, _) => {
            drop(view);
            drop(apps);
            run_selected();
        }

        // while searching, Left and Right are the caret's and Up and Down
        // the list's, wherever the focus is
        (Key::Left, _) if searching => with(|state| state.field.left(held.shift)),
        (Key::Right, _) if searching => with(|state| state.field.right(held.shift)),
        (Key::Home, _) if searching => with(|state| state.field.move_to(0, held.shift)),
        (Key::End, _) if searching => with(|state| state.field.move_to(usize::MAX, held.shift)),

        (Key::Down, Zone::Search) if searching => set_focus(Zone::Body),
        (Key::Down, Zone::Search) => set_focus(Zone::Pills),

        (Key::Left, Zone::Pills) => turn_to(view.page.saturating_sub(1)),
        (Key::Right, Zone::Pills) => {
            turn_to((view.page + 1).min(view.buckets.len().saturating_sub(1)))
        }
        (Key::Home, Zone::Pills) => turn_to(0),
        (Key::End, Zone::Pills) => turn_to(view.buckets.len().saturating_sub(1)),
        (Key::Up, Zone::Pills) => set_focus(Zone::Search),
        (Key::Down, Zone::Pills) => set_focus(Zone::Body),

        // off the top of the apps, up to what's over them
        (Key::Up, Zone::Body) if selected < layout.row() => {
            set_focus(if searching { Zone::Search } else { Zone::Pills })
        }
        (Key::Left, Zone::Body) if count > 0 => select(selected.saturating_sub(1), layout),
        (Key::Right, Zone::Body) if count > 0 => select((selected + 1).min(count - 1), layout),
        (Key::Up, Zone::Body) if count > 0 => select(selected.saturating_sub(layout.row()), layout),
        (Key::Down, Zone::Body) if count > 0 => {
            select((selected + layout.row()).min(count - 1), layout)
        }
        (Key::Home, Zone::Body) if count > 0 => select(0, layout),
        (Key::End, Zone::Body) if count > 0 => select(count - 1, layout),
        // Up and Down in the list reach it from the search field too
        (Key::Up, Zone::Search) if searching && count > 0 => {
            select(selected.saturating_sub(1), layout)
        }
        _ => return,
    }
    mark_all();
}

// ───── view ──────────────────────────────────────────────────────────────────

/// Steps the panel's opening towards `open` and gives how far it is, a
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

/// How big the panel is: the grid, the rows over it and the inset round
/// them all.
fn size() -> (f32, f32) {
    let height = HEADER + Divider::height() + PILLS + BODY + INSET * 2.0;
    (WIDTH + INSET * 2.0, height)
}

/// Steps `anim` towards `to` and gives where it is.
fn ease(anim: &'static std::thread::LocalKey<RefCell<Animation>>, to: f32) -> f32 {
    anim.with_borrow_mut(|anim| {
        anim.to(to);
        anim.value()
    })
}

/// The surface: hidden while closed, else the glass panel in the middle of
/// a clear surface that closes the launcher when clicked.
pub fn surface() -> LayerWindow {
    let open = is_open(Panel::Launcher);
    let unfolded = unfold(open);
    let (width, height) = size();

    if !open && unfolded <= 0.001 {
        hidden();
        return window()
            .visible(false)
            .blur_shapes(Vec::new())
            .child(blank());
    }

    // where the panel sits: the middle of the surface, once the compositor
    // has said how big that is
    let (screen_w, screen_h) = window_size();
    let origin = (
        ((screen_w - width) / 2.0).max(0.0),
        ((screen_h - height) / 2.0).max(0.0),
    );

    // The panel swells in about its centre, the frost with it. A compositor
    // that can't fade frost only shows or hides it, so there the panel
    // arrives whole rather than fading in over frost at full strength.
    let fades = amane::blur_fades();
    let scale = f32::blend(motion::scale(OPEN_SCALE), 1.0, unfolded);
    let opacity = if open && !fades {
        1.0
    } else {
        unfolded.clamp(0.0, 1.0)
    };
    let frost = if open || fades {
        let glass = glass_region::module(
            origin.0,
            origin.1,
            width,
            height,
            RADIUS,
            1.0,
            Thickness::Thick.tint(),
        );
        let centre = (origin.0 + width / 2.0, origin.1 + height / 2.0);
        glass_region::faded(
            glass_region::transformed(vec![glass], scale, centre, (0.0, 0.0)),
            opacity,
        )
    } else {
        Vec::new()
    };

    // a click outside the panel closes it; the panel keeps its own clicks
    let outside = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .on_click(|_| close())
        .align_child(Center, Center)
        .child(panel(width, height).opacity(opacity).scale(scale));

    window()
        .visible(true)
        .blur_shapes(frost)
        .child(track_cursor(outside))
}

/// The glass panel and everything on it.
fn panel(width: f32, height: f32) -> Rectangle {
    let apps = Apps::read();
    let view = View::new(apps.list());
    let layout = view.layout();
    let count = view.count();
    let pages = view.buckets.len();

    let offset = PAGER.with_borrow_mut(|pager| {
        pager.settle_stale(pages, &PAGING);
        if pager.page() >= pages && pages > 0 {
            pager.jump(pages - 1, &PAGING);
        }
        pager.offset(pages, &PAGING)
    });

    // what the body shows: a change starts the list afresh, snapping the
    // highlight and the scroll there rather than sliding them
    let mode = (view.searching, view.page);
    let (shown, switched) = with(|state| {
        state.pages = pages;
        state.selected = view.selected;
        let shown = std::mem::replace(&mut state.shown, mode);
        (shown, shown != mode)
    });
    let selected = if switched { 0 } else { view.selected };
    let (to_x, to_y) = layout.highlight(selected);
    if switched {
        with(|state| state.selected = 0);
        HIGHLIGHT_X.with_borrow_mut(|x| *x = motion::fade(to_x, motion::HOVER));
        HIGHLIGHT_Y.with_borrow_mut(|y| *y = motion::fade(to_y, motion::HOVER));
        SCROLL.with_borrow_mut(|scroll| scroll.jump(0.0));
        // into or out of a search, the body fades in; a category slides
        if shown.0 != mode.0 {
            SWITCH.with_borrow_mut(|switch| *switch = motion::fade(0.0, motion::FADE));
        }
    }

    // how far the body scrolls
    let scroll = SCROLL.with_borrow_mut(|scroll| {
        scroll.set_max(layout.content(count) - BODY);
        scroll.settle_stale();
        if scroll.swiping() {
            moved();
        }
        scroll.value()
    });

    // a search's results are what Enter runs, whichever zone has the keys
    let focused = view.focus == Zone::Body || view.searching;
    let at = grid::At {
        scroll,
        x: ease(&HIGHLIGHT_X, to_x),
        y: ease(&HIGHLIGHT_Y, to_y),
        highlight: ease(&HIGHLIGHT_ALPHA, if focused { 1.0 } else { UNFOCUSED }),
        fade: ease(&SWITCH, 1.0),
        scrollbar: scrollbar(),
    };

    let body: Box<dyn Widget> = if view.searching {
        boxed(results::list(&view.found, at))
    } else if view.all.is_empty() {
        boxed(results::empty(results::Empty::Loading))
    } else {
        boxed(grid::pages(&view, offset, at))
    };

    let column = Column::new(vec![
        boxed(header::search_row(&view.field, view.focus == Zone::Search)),
        boxed(Divider::build()),
        boxed(header::pill_bar(
            &view.buckets,
            offset,
            view.focus == Zone::Pills,
        )),
        body,
    ])
    .width(WIDTH);

    let glass = Rectangle::new()
        .width(width)
        .height(height)
        .radius(RADIUS)
        .fill(glass_region::fill(Thickness::Thick.tint()))
        // the panel keeps its clicks from the surface under it, which
        // closes the launcher
        .on_click(|_| {});
    glass_region::lit(glass, RADIUS)
        .padding(pad(INSET, INSET))
        .align_child(Start, Start)
        .child(column)
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
        Button::Left => launch(&app),
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
        assert_eq!(Zone::Body.next(false, false), Zone::Search);
        assert_eq!(Zone::Search.next(false, false), Zone::Pills);
        assert_eq!(Zone::Pills.next(false, false), Zone::Body);
        assert_eq!(Zone::Search.next(true, false), Zone::Body);
    }

    #[test]
    fn a_search_has_no_pills_to_tab_to() {
        assert_eq!(Zone::Search.next(false, true), Zone::Body);
        assert_eq!(Zone::Body.next(false, true), Zone::Search);
        // the pills, focused before the search began, count as the field
        assert_eq!(Zone::Pills.next(false, true), Zone::Body);
    }

    #[test]
    fn the_layouts_put_items_where_they_draw() {
        assert_eq!(Layout::Grid.span(COLS + 1), (CELL, CELL));
        assert_eq!(
            Layout::Grid.highlight(COLS + 1),
            (CELL + BOX_X, CELL + BOX_Y)
        );
        assert_eq!(Layout::List.span(2), (2.0 * (ROW + ROW_GAP), ROW));
        assert_eq!(Layout::Grid.content(0), CELL);
        assert_eq!(Layout::Grid.content(COLS + 1), 2.0 * CELL);
    }
}
