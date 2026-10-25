//! Launcher — an app launcher after macOS Launchpad, by way of KDE's Tahoe
//! Launcher: one glass panel in the middle of the screen with a search
//! field, a row of category pills and the installed apps in a grid five
//! across and four down, scrolling smoothly. One highlight glides to the
//! app under the pointer or the arrow keys.
//!
//! Typing turns the grid into a list of what the search finds
//! ([`entries`]): a sum's answer, the matching apps, the session actions
//! named, the URL typed and a web search. The launcher draws the search
//! text itself rather than focusing a `TextInput`, so every key reaches
//! it: the arrows move through the grid, and in the list Left and Right
//! move the caret, Shift selects and Ctrl+A/C/X/V edit ([`edit`], amane's
//! `modifiers()`).
//!
//! The glass is Quick Settings' and the dock's: the compositor frosts
//! behind the panel where the surface says ([`glass_region`]), or the
//! shader draws its lit edge where the compositor has no glass material.
//! The surface keeps one size, open or closing, so nothing is resized
//! while it moves.
//!
//! It shares the notification center's and Quick Settings'
//! one-open-at-a-time state ([`Panel::Launcher`]), opened over IPC:
//! `launcher [toggle | open | close]`, which a niri keybind sends. See
//! `docs/launcher.md`.

mod edit;
mod entries;
mod grid;
mod header;
mod model;
mod results;

use std::cell::{Cell, RefCell};

use amane::{
    Animation, Apps, Blend, Column, DesktopApp, Horizontal, Key, Keyboard, LayerWindow, Rectangle,
    Scroll, ScrollSource, Service, Start, Vertical, Widget, mark_all, modifiers,
};

use crate::components::divider::Divider;
use crate::components::glass_region;
use crate::components::layout::{blank, boxed, pad};
use crate::components::motion;
use crate::components::taskbar::{self, Panel, close, is_open, overlay};

use edit::Field;
use entries::Entry;

/// Layer-surface namespace; the niri `layer-rule` matches it (a clear
/// surface that says where to frost).
pub const LAUNCHER_NS: &str = "amane.launcher";

// TODO(token): `launcher.grid.*`, `launcher.panel.*`, `launcher.result.*`
// and `launcher.open.scale` once Penpot has them (manual §1.1).
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

/// How wide what's on the panel is: the grid's width.
const WIDTH: f32 = CELL * COLS as f32;
/// How tall the grid, or the list in its place, is in view.
const BODY: f32 = CELL * ROWS as f32;
/// Where the highlight sits in a cell: centred, at the cell's top padding.
const BOX_X: f32 = (CELL - ICON_BOX) / 2.0;
const BOX_Y: f32 = 10.0;

struct State {
    field: Field,
    // the pill picked, an index into `model::CATEGORIES`
    category: usize,
    // the app or entry highlighted
    selected: usize,
    // whether the last frame showed search results, and under which pill:
    // a change snaps the highlight and the scroll rather than slide them
    // between lists that have nothing to do with each other
    shown: (bool, usize),
    // where the body is scrolled to (the scroll eases after it), and how far
    // it goes
    scroll: f32,
    scroll_max: f32,
    // the same for the pill bar, sideways
    pills: f32,
    pills_max: f32,
}

impl State {
    const fn new() -> Self {
        Self {
            field: Field::new(),
            category: 0,
            selected: 0,
            shown: (false, 0),
            scroll: 0.0,
            scroll_max: 0.0,
            pills: 0.0,
            pills_max: 0.0,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::new()) };

    // 0 closed … 1 open: the panel fades and swells in
    static OPEN: RefCell<Animation> = RefCell::new(motion::pop(0.0));

    // whether OPEN is heading for 1
    static OPENING: Cell<bool> = const { Cell::new(false) };

    // the body's scroll, easing after `State::scroll`
    static SCROLL: RefCell<Animation> = RefCell::new(motion::fade(0.0, motion::PRESS));

    // where the highlight is gliding to
    static HIGHLIGHT_X: RefCell<Animation> = RefCell::new(motion::fade(BOX_X, motion::HOVER));
    static HIGHLIGHT_Y: RefCell<Animation> = RefCell::new(motion::fade(BOX_Y, motion::HOVER));

    // 0 … 1: the body fading in after the list under it changed
    static SWITCH: RefCell<Animation> = RefCell::new(motion::fade(1.0, motion::FADE));
}

fn with<R>(change: impl FnOnce(&mut State) -> R) -> R {
    STATE.with_borrow_mut(change)
}

// ───── runtime ───────────────────────────────────────────────────────────────

pub(crate) fn opened() {}

pub(crate) fn closed() {}

/// Gone from the screen: the next opening starts afresh, on all the apps.
fn hidden() {
    with(|state| *state = State::new());
    SCROLL.with_borrow_mut(|scroll| *scroll = motion::fade(0.0, motion::PRESS));
    SWITCH.with_borrow_mut(|switch| *switch = motion::fade(1.0, motion::FADE));
}

/// `amane-shell` IPC: `launcher [toggle | open | close]`.
pub fn ipc(args: &[String]) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["toggle"] => taskbar::toggle(Panel::Launcher),
        ["open"] => {
            if !is_open(Panel::Launcher) {
                taskbar::toggle(Panel::Launcher);
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

/// Scrolls the body to `to`, kept in range.
fn scroll_to(to: f32) {
    with(|state| state.scroll = to.clamp(0.0, state.scroll_max));
}

/// A scroll in pixels: a wheel steps, fingers are followed.
fn scrolled(scroll: &Scroll) -> (f32, f32) {
    match scroll.source {
        ScrollSource::Wheel => (scroll.x * WHEEL_STEP, scroll.y * WHEEL_STEP),
        _ => scroll.pixels(),
    }
}

fn on_scroll(scroll: Scroll) {
    let (_, y) = scrolled(&scroll);
    let to = with(|state| state.scroll) + y;
    scroll_to(to);
    mark_all();
}

/// The pill bar follows a sideways swipe, or a wheel turned over it.
fn on_pill_scroll(scroll: Scroll) {
    let (x, y) = scrolled(&scroll);
    let step = if x.abs() > f32::EPSILON { x } else { y };
    with(|state| state.pills = (state.pills + step).clamp(0.0, state.pills_max));
    mark_all();
}

/// Highlights entry `index`, scrolling it into view.
fn select(index: usize, searching: bool) {
    let (top, height) = if searching {
        (index as f32 * (ROW + ROW_GAP), ROW)
    } else {
        ((index / COLS) as f32 * CELL, CELL)
    };
    with(|state| {
        state.selected = index;
        if top < state.scroll {
            state.scroll = top;
        } else if top + height > state.scroll + BODY {
            state.scroll = top + height - BODY;
        }
        state.scroll = state.scroll.clamp(0.0, state.scroll_max);
    });
}

/// Hovering an app or entry highlights it.
fn hover(index: usize) -> impl Fn(bool) + 'static {
    move |inside| {
        if inside && with(|state| std::mem::replace(&mut state.selected, index)) != index {
            mark_all();
        }
    }
}

/// Picks pill `category`, back at the top of its apps.
fn pick(category: usize) {
    with(|state| {
        state.category = category;
        state.selected = 0;
        state.scroll = 0.0;
    });
    mark_all();
}

/// Edits the search text; a change starts the list over at the top.
fn type_into(edit: impl FnOnce(&mut Field) -> bool) {
    with(|state| {
        let before = state.field.text().to_string();
        let changed = edit(&mut state.field);
        if changed || state.field.text() != before {
            state.selected = 0;
            state.scroll = 0.0;
        }
    });
}

/// The apps under pill `category`, in the list's order.
fn under(apps: &[DesktopApp], category: usize) -> Vec<&DesktopApp> {
    apps.iter()
        .filter(|app| model::in_bucket(model::mask(app.categories(), app.exec()), category))
        .collect()
}

/// What Enter runs: the highlighted app or entry, else the first.
fn run_selected() {
    let (field, category, selected) =
        with(|state| (state.field.clone(), state.category, state.selected));
    let apps = Apps::read();
    if searching(&field) {
        let found = entries::build(field.text(), apps.list());
        drop(apps);
        if let Some(entry) = found.get(selected).or_else(|| found.first()) {
            entries::run_action(&entry.action);
        }
    } else {
        let shown = under(apps.list(), category);
        if let Some(app) = shown.get(selected).or_else(|| shown.first()) {
            let app = (*app).clone();
            drop(apps);
            close();
            app.launch();
        }
    }
}

fn searching(field: &Field) -> bool {
    !field.text().trim().is_empty()
}

/// Every key: nothing else on the panel takes the focus.
fn on_key(key: Key) {
    let (field, category, selected) =
        with(|state| (state.field.clone(), state.category, state.selected));
    let searching = searching(&field);
    let count = {
        let apps = Apps::read();
        if searching {
            entries::build(field.text(), apps.list()).len()
        } else {
            under(apps.list(), category).len()
        }
    };
    let held = modifiers();

    // a chord isn't text: the editing ones edit, the rest go nowhere
    if held.ctrl
        && let Key::Character(letter) = key
    {
        match letter.to_ascii_lowercase() {
            'a' if searching => with(|state| state.field.select_all()),
            'c' if searching => entries::copy(&field.selected()),
            'x' if searching => {
                entries::copy(&field.selected());
                type_into(Field::delete_selection);
            }
            'v' => {
                if let Some(text) = entries::paste() {
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

    let step = if searching { 1 } else { COLS };
    match key {
        Key::Character(letter) => type_into(|field| {
            field.insert(&letter.to_string());
            true
        }),
        // a search doesn't start with a space
        Key::Space if !field.text().is_empty() => type_into(|field| {
            field.insert(" ");
            true
        }),
        Key::Backspace => type_into(Field::backspace),
        Key::Enter => run_selected(),
        // a search clears first, then the launcher closes
        Key::Escape if searching => type_into(|field| {
            *field = Field::new();
            true
        }),
        Key::Escape => close(),
        // in the list, Left and Right are the caret's; in the grid, the
        // highlight's
        Key::Left if searching => with(|state| state.field.left(held.shift)),
        Key::Right if searching => with(|state| state.field.right(held.shift)),
        Key::Home if searching => with(|state| state.field.move_to(0, held.shift)),
        Key::End if searching => with(|state| state.field.move_to(usize::MAX, held.shift)),
        Key::Left if count > 0 => select(selected.saturating_sub(1), false),
        Key::Right if count > 0 => select((selected + 1).min(count - 1), false),
        Key::Up if count > 0 => select(selected.saturating_sub(step), searching),
        Key::Down if count > 0 => select((selected + step).min(count - 1), searching),
        Key::Tab if count > 0 => select((selected + 1) % count, searching),
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

/// The surface: hidden while closed, else the glass panel.
pub fn surface() -> LayerWindow {
    let open = is_open(Panel::Launcher);
    let unfolded = unfold(open);
    let (width, height) = size();

    if !open && unfolded <= 0.001 {
        hidden();
        return window(width, height)
            .visible(false)
            .blur_shapes(Vec::new())
            .child(blank());
    }

    // The panel swells in about its centre, the frost with it. A compositor
    // that can't fade frost only shows or hides it, so there the panel
    // arrives whole rather than fading in over frost at full strength.
    let fades = amane::blur_fades();
    let scale = f32::blend(OPEN_SCALE, 1.0, unfolded);
    let opacity = if open && !fades {
        1.0
    } else {
        unfolded.clamp(0.0, 1.0)
    };
    let frost = if open || fades {
        let glass = glass_region::module(0.0, 0.0, width, height, RADIUS, 1.0);
        glass_region::faded(
            glass_region::transformed(vec![glass], scale, (width / 2.0, height / 2.0), (0.0, 0.0)),
            opacity,
        )
    } else {
        Vec::new()
    };

    window(width, height)
        .visible(true)
        .blur_shapes(frost)
        .child(panel(width, height).opacity(opacity).scale(scale))
}

/// The glass panel and everything on it.
fn panel(width: f32, height: f32) -> Rectangle {
    let apps = Apps::read();
    let all = apps.list();
    let (field, category, selected) =
        with(|state| (state.field.clone(), state.category, state.selected));
    let searching = searching(&field);

    let found: Vec<Entry> = if searching {
        entries::build(field.text(), all)
    } else {
        Vec::new()
    };
    let shown = if searching {
        Vec::new()
    } else {
        under(all, category)
    };
    let count = if searching { found.len() } else { shown.len() };
    let selected = selected.min(count.saturating_sub(1));

    // how far the body scrolls, the target kept in range of it
    let content = if searching {
        count as f32 * (ROW + ROW_GAP)
    } else {
        count.div_ceil(COLS).max(1) as f32 * CELL
    };
    let target = with(|state| {
        state.selected = selected;
        state.scroll_max = (content - BODY).max(0.0);
        state.scroll = state.scroll.clamp(0.0, state.scroll_max);
        state.scroll
    });

    // where the highlight rests on the selected app or row
    let (to_x, to_y) = if searching {
        (0.0, selected as f32 * (ROW + ROW_GAP))
    } else {
        (
            (selected % COLS) as f32 * CELL + BOX_X,
            (selected / COLS) as f32 * CELL + BOX_Y,
        )
    };
    let mode = (searching, category);
    let switched = with(|state| std::mem::replace(&mut state.shown, mode)) != mode;
    if switched {
        // another list: the highlight and the scroll start where they belong,
        // and the body fades in
        HIGHLIGHT_X.with_borrow_mut(|x| *x = motion::fade(to_x, motion::HOVER));
        HIGHLIGHT_Y.with_borrow_mut(|y| *y = motion::fade(to_y, motion::HOVER));
        SCROLL.with_borrow_mut(|scroll| *scroll = motion::fade(target, motion::PRESS));
        SWITCH.with_borrow_mut(|switch| *switch = motion::fade(0.0, motion::FADE));
    }
    let ease = |anim: &'static std::thread::LocalKey<RefCell<Animation>>, to: f32| {
        anim.with_borrow_mut(|anim| {
            anim.to(to);
            anim.value()
        })
    };
    let at = grid::At {
        scroll: ease(&SCROLL, target),
        x: ease(&HIGHLIGHT_X, to_x),
        y: ease(&HIGHLIGHT_Y, to_y),
        fade: ease(&SWITCH, 1.0),
    };

    let body: Box<dyn Widget> = if searching {
        boxed(results::list(&found, at))
    } else if shown.is_empty() {
        boxed(results::empty(if all.is_empty() {
            "Loading…"
        } else {
            "Nothing in this category"
        }))
    } else {
        boxed(grid::grid(&shown, at))
    };

    let buckets = model::active_buckets(
        all.iter()
            .map(|app| model::mask(app.categories(), app.exec())),
    );
    let column = Column::new(vec![
        boxed(header::search_row(&field)),
        boxed(Divider::build()),
        boxed(header::pill_bar(category, &buckets)),
        body,
    ])
    .width(WIDTH);

    let glass = Rectangle::new()
        .width(width)
        .height(height)
        .radius(RADIUS)
        .fill(glass_region::tint());
    glass_region::lit(glass, RADIUS)
        .padding(pad(INSET, INSET))
        .align_child(Start, Start)
        .child(column)
}

/// The surface in the middle of the screen, `width` by `height`.
fn window(width: f32, height: f32) -> LayerWindow {
    let base = LayerWindow::new()
        .width(width)
        .height(height)
        // modal while open: every key is the launcher's from the first moment
        .keyboard(Keyboard::Exclusive)
        .on_key(on_key);
    overlay(base, LAUNCHER_NS, Vertical::Middle, Horizontal::Middle)
}
