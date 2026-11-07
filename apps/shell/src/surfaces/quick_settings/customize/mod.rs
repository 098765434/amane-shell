//! The Customize window — where Quick Settings' controls are added from:
//! one frosted sheet in the middle of the screen with a sidebar
//! (a search field over the categories) and a pane listing every control
//! at its real size on a plain tint, named under it, under section
//! headers. A control is dragged from the pane onto the card, which stays
//! open beside the window in edit mode, and dragged off the card to remove
//! it; a footer carries the hint, Undo Remove, Reset Layout and **Done**.
//!
//! The window is drawn on Quick Settings' own surface, which covers the
//! output while editing (`super::surface`): amane can't carry a drag from
//! one surface to another, so the card and the window share one, with one
//! still layer under both that every drag is measured on ([`drag_layer`]).
//! A press on a control in the pane takes hold of it ([`edit::lift`]); the
//! control in hand is drawn here over the card and the window alike
//! ([`ghost`]), in the grid's own pixels, while the grid's preview makes
//! way for it. Clicks outside the card and the window go through to what's
//! under them.
//!
//! Opening edit mode opens the window (`edit::enter`), and Done, Escape or
//! anything else that leaves edit mode closes it. See
//! `docs/shell/quick-settings.md`.

mod catalog;
mod drag;
#[cfg(test)]
mod tests;
mod view;

use drag::*;
pub(super) use drag::{drag_layer, ghost, place, set_canvas_origin};
pub(super) use view::view;

use std::cell::{Cell, RefCell};

use amane::{
    Animation, Blend, BlurShape, Center, Color, Cursor, Key, Parent, Point, Rectangle, Row, Scroll,
    Stack, Start, Text, Widget, modifiers,
};

use super::controls::{self, ControlId, Ctx};
use super::grid::{Metrics, Place, Rect, Slot};
use super::{edit, layout, tile, tiles};
use crate::surfaces::quick_settings::grid::FromTokens as _;
use crate::tokens::{Styled, quick, sys};
use amane_kit::buttons::{PushButton, Shape, Style};
use amane_kit::divider::Divider;
use amane_kit::flow::Flow;
use amane_kit::glass_region;
use amane_kit::kinetic::{Axis, Kinetic};
use amane_kit::layout::{boxed, caption, pad, split_row, text_height, wrapped_lines};
use amane_kit::list::{self, List, ListItem};
use amane_kit::material::{self, Level, Material};
use amane_kit::motion;
use amane_kit::scroll_view::{ScrollView, ScrollbarFade};
use amane_kit::search_field::SearchField;
use amane_kit::text_edit::Field;

pub(super) use catalog::Category;

// TODO(token): `customize.window.{width,height}`, `customize.sidebar.width`,
// `customize.pane.inset`, `customize.open.scale`, `customize.scroll.wheel-step`
// and `customize.scrollbar.idle` once Penpot has them (manual §1.1).
/// The window, as big as the reference; shorter on a short display.
const WIDTH: f32 = 860.0;
const HEIGHT: f32 = 960.0;
/// The sidebar's width, the search field and the rows in it.
const SIDEBAR: f32 = 300.0;
/// The room between the sheet's edge and what's on it.
const INSET: f32 = 24.0;
/// How small the window is as it swells in.
const OPEN_SCALE: f32 = 0.96;
/// How far one wheel notch scrolls the pane.
const WHEEL_STEP: f32 = 60.0;

/// A rectangle in the surface's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Area {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Area {
    fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.x + self.width && p.y < self.y + self.height
    }

    /// The part of `self` inside `other`, if any.
    fn clipped(&self, other: &Area) -> Option<Area> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        (right > x && bottom > y).then_some(Area {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }
}

struct State {
    field: Field,
    category: Category,
    // the controls in the pane this frame, where they're drawn on the
    // surface (clipped to the pane), for a press to find what's under it
    tiles: Vec<(ControlId, Area)>,
    // when the pane last moved, and when the scrollbar is next due to fade
}

impl State {
    const fn new() -> Self {
        Self {
            field: Field::new(),
            category: Category::AllControls,
            tiles: Vec::new(),
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::new()) };
    // 0 closed … 1 open: the window fades and swells in
    static OPEN: RefCell<Animation> = RefCell::new(motion::pop(0.0));
    // whether OPEN is heading for 1
    static OPENING: Cell<bool> = const { Cell::new(false) };
    // how far the pane and the sidebar scroll
    static SCROLL: RefCell<Kinetic> = RefCell::new(Kinetic::new());
    static SIDE_SCROLL: RefCell<Kinetic> = RefCell::new(Kinetic::new());
    // 0 … 1: the scrollbar, showing while the pane moves
    static SCROLLBAR: RefCell<ScrollbarFade> = RefCell::new(ScrollbarFade::new());
    // where the canvas's top-left is on the surface this frame: a press on
    // the surface is measured from it
    static CANVAS_ORIGIN: Cell<(f32, f32)> = const { Cell::new((0.0, 0.0)) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with_borrow_mut(f)
}

// ───── runtime ───────────────────────────────────────────────────────────────

/// Opens the window (edit mode does, as it begins).
pub(super) fn open() {
    if !OPENING.replace(true) {
        OPEN.with_borrow_mut(|open| {
            open.retime(motion::POP);
            open.to(1.0);
        });
        super::QuickSettings::changed();
    }
}

/// Closes the window (edit mode does, as it ends); it fades out, then
/// starts afresh.
pub(super) fn close() {
    if OPENING.replace(false) {
        OPEN.with_borrow_mut(|open| {
            open.retime(motion::CLOSE);
            open.to(0.0);
        });
        super::QuickSettings::changed();
    }
}

/// Whether the window is on the surface: open, or still fading out.
pub(super) fn visible() -> bool {
    OPENING.get() || OPEN.with_borrow(|open| open.value() > 0.001)
}

/// Gone from the screen: the next opening starts afresh, on All Controls
/// with no query, at the top.
pub(super) fn hidden() {
    with(|state| *state = State::new());
    SCROLL.with_borrow_mut(|scroll| *scroll = Kinetic::new());
    SIDE_SCROLL.with_borrow_mut(|scroll| *scroll = Kinetic::new());
    SCROLLBAR.with_borrow_mut(ScrollbarFade::reset);
    OPENING.set(false);
    OPEN.with_borrow_mut(|open| *open = motion::pop(0.0));
}

/// Whether there's a query to clear before Escape means anything else.
pub(super) fn has_query() -> bool {
    with(|state| !state.field.text().is_empty())
}

pub(super) fn clear_query() {
    with(|state| state.field = Field::new());
    SCROLL.with_borrow_mut(|scroll| scroll.go(0.0));
    super::QuickSettings::changed();
}

/// A key while the window is open: it goes to the search field. Whether
/// the field took it.
pub(super) fn key(key: Key) -> bool {
    let taken = with(|state| {
        let before = state.field.text().to_string();
        let taken = state.field.handle(key, modifiers(), |text| {
            with(|state| state.field.insert(&text));
            SCROLL.with_borrow_mut(|scroll| scroll.go(0.0));
            super::QuickSettings::changed();
        });
        if state.field.text() != before {
            SCROLL.with_borrow_mut(|scroll| scroll.go(0.0));
        }
        taken
    });
    if taken {
        super::QuickSettings::changed();
    }
    taken
}

fn pick(category: Category) {
    with(|state| {
        state.category = category;
        state.field = Field::new();
    });
    SCROLL.with_borrow_mut(|scroll| scroll.go(0.0));
    super::QuickSettings::changed();
}

// ───── view ──────────────────────────────────────────────────────────────────

/// The window as drawn this frame: its widget (over the whole surface),
/// the frost behind it, and where it takes the pointer.
pub(super) struct Built {
    pub widget: Box<dyn Widget>,
    pub frost: Vec<BlurShape>,
    pub area: Area,
}
