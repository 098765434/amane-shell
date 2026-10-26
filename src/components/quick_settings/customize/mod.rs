//! The Customize window — macOS's Customize Control Center for Quick
//! Settings: one frosted sheet in the middle of the screen with a sidebar
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

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use amane::{
    Animation, Blend, BlurShape, Center, Color, Cursor, Key, Parent, Point, Rectangle, Row, Scroll,
    Size, Stack, Start, Text, Widget, modifiers,
};

use super::controls::{self, ControlId, Ctx};
use super::grid::{Metrics, Place, Rect, Slot};
use super::{edit, layout, tile, tiles};
use crate::components::buttons::{PushButton, Shape, Style};
use crate::components::divider::Divider;
use crate::components::flow::Flow;
use crate::components::glass_region;
use crate::components::kinetic::{Axis, Kinetic};
use crate::components::layout::{boxed, caption, pad, split_row, text_height, wrapped_lines};
use crate::components::list::{self, List, ListItem};
use crate::components::material::{self, Level, Material};
use crate::components::motion;
use crate::components::scroll_view::ScrollView;
use crate::components::search_field::SearchField;
use crate::components::text_edit::Field;
use crate::tokens::{Styled, quick, sys};

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
/// How long the scrollbar stays after the pane last moved.
const SCROLLBAR_IDLE: Duration = Duration::from_millis(800);

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
    scrolled: Option<Instant>,
    fade_due: Option<Instant>,
}

impl State {
    const fn new() -> Self {
        Self {
            field: Field::new(),
            category: Category::AllControls,
            tiles: Vec::new(),
            scrolled: None,
            fade_due: None,
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
    static SCROLLBAR: RefCell<Animation> = RefCell::new(motion::fade(0.0, motion::FADE));
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
    SCROLLBAR.with_borrow_mut(|bar| *bar = motion::fade(0.0, motion::FADE));
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
        let taken = state.field.handle(key, modifiers());
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

/// The surface says where the canvas is this frame.
pub(super) fn set_canvas_origin(x: f32, y: f32) {
    CANVAS_ORIGIN.set((x, y));
}

fn on_canvas(p: Point) -> Point {
    let (x, y) = CANVAS_ORIGIN.get();
    Point {
        x: p.x - x,
        y: p.y - y,
    }
}

/// The pane moved: the scrollbar shows for a while.
fn moved() {
    with(|state| state.scrolled = Some(Instant::now()));
}

fn on_pane_scroll(scroll: Scroll) {
    SCROLL.with_borrow_mut(|pane| pane.scroll(&scroll, Axis::Vertical, WHEEL_STEP));
    moved();
    super::QuickSettings::changed();
}

fn on_side_scroll(scroll: Scroll) {
    SIDE_SCROLL.with_borrow_mut(|side| side.scroll(&scroll, Axis::Vertical, WHEEL_STEP));
    super::QuickSettings::changed();
}

// ───── the drag, measured on the surface ────────────────────────────────────

/// The pointer is at `point` on the surface with the button down: carries
/// on the drag in flight; else takes hold of the control under it in the
/// pane; else hands the press to the canvas, which picks up what's there.
fn drag_at(point: Point) {
    if edit::drag().is_some() {
        edit::drag_at(on_canvas(point));
        return;
    }
    let under = with(|state| {
        state
            .tiles
            .iter()
            .find(|(_, area)| area.contains(point))
            .map(|(id, area)| (*id, *area))
    });
    match under {
        Some((id, area)) => {
            // where on the control the pointer took hold, as drawn: the
            // control is its true size in the pane
            let grab = Point {
                x: point.x - area.x,
                y: point.y - area.y,
            };
            edit::lift(id, grab, on_canvas(point));
        }
        None => edit::drag_at(on_canvas(point)),
    }
}

/// The still layer under the card and the window that every drag is
/// measured on. It shows the hand's cursor while a drag is in flight; at
/// rest what's under it names its own.
pub(super) fn drag_layer() -> Box<dyn Widget> {
    let layer = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .on_drag(drag_at)
        .on_release(|_| edit::release());
    let layer = match edit::drag() {
        Some(edit::Drag::Move { .. }) => layer.cursor(Cursor::Grabbing),
        Some(edit::Drag::Resize { .. }) => layer.cursor(Cursor::ResizeBottomRight),
        None => layer,
    };
    boxed(layer)
}

/// The control in hand, drawn over everything where the pointer took it,
/// and the frost behind it; `origin` is the canvas's top-left on the
/// surface.
pub(super) fn ghost(origin: (f32, f32)) -> Option<(Box<dyn Widget>, BlurShape)> {
    let (id, at) = edit::held_at()?;
    let span = layout::with(|layout| layout.span_of(id)).unwrap_or(id.default_span());
    let px = Metrics::tokens();
    let slot = px.slot(Rect::new(Place::new(0, 0), span));
    let control =
        controls::build(id, span, &Ctx::new(true)).unwrap_or_else(|| tiles::placeholder(id, slot));
    let scale = quick::drag::scale();
    let (x, y) = (origin.0 + at.0, origin.1 + at.1);
    let radius = tiles::radius_of(id, span, slot);
    let frost = Material::Control.shape(
        x + slot.width * (1.0 - scale) / 2.0,
        y + slot.height * (1.0 - scale) / 2.0,
        slot.width * scale,
        slot.height * scale,
        radius * scale,
        1.0,
    );
    let widget = Rectangle::new()
        .width(Size::Fixed(slot.width))
        .height(Size::Fixed(slot.height))
        .fill(Color::TRANSPARENT)
        .translate(x, y)
        .scale(scale)
        .child(Stack::new(vec![control]));
    Some((boxed(widget), frost))
}

// ───── view ──────────────────────────────────────────────────────────────────

/// Where the window sits on a `screen` (the surface's size) beside the
/// `card`: in the middle, moved left to clear the card by a gap when the
/// screen has the room, and no taller than the screen allows.
pub(super) fn place(screen: (f32, f32), card: Area) -> Area {
    let margin = sys::space::gap::l();
    let width = WIDTH.min(screen.0 - margin * 2.0).max(1.0);
    let height = HEIGHT.min(screen.1 - margin * 2.0).max(1.0);
    let mut x = (screen.0 - width) / 2.0;
    if x + width + margin > card.x {
        x = (card.x - margin - width).max(margin);
    }
    let y = (screen.1 - height) / 2.0;
    Area {
        x: x.round(),
        y: y.round(),
        width,
        height,
    }
}

/// The window as drawn this frame: its widget (over the whole surface),
/// the frost behind it, and where it takes the pointer.
pub(super) struct Built {
    pub widget: Box<dyn Widget>,
    pub frost: Vec<BlurShape>,
    pub area: Area,
}

/// Steps the window's opening and gives how far it is, a little past 1 on
/// the spring; `None` once it has gone.
fn unfold() -> Option<f32> {
    let opening = OPENING.get();
    let value = OPEN.with_borrow_mut(|open| open.value());
    if !opening && value <= 0.001 {
        return None;
    }
    Some(value)
}

/// The window on a surface `screen` big, beside the `card`, or nothing
/// while it's closed.
pub(super) fn view(screen: (f32, f32), card: Area) -> Option<Built> {
    let unfolded = unfold()?;
    let area = place(screen, card);
    let radius = quick::card::radius();

    let sheet = tile::sheet(
        area.width,
        area.height,
        radius,
        Material::Sheet,
        Material::Sheet.tint(),
        content(&area),
    );

    // it swells in about its centre, the frost with it (see the launcher)
    let fades = amane::blur_fades();
    let scale = f32::blend(motion::scale(OPEN_SCALE), 1.0, unfolded);
    let opacity = if OPENING.get() && !fades {
        1.0
    } else {
        unfolded.clamp(0.0, 1.0)
    };
    let frost = if OPENING.get() || fades {
        let glass = Material::Sheet.shape(area.x, area.y, area.width, area.height, radius, 1.0);
        let centre = (area.x + area.width / 2.0, area.y + area.height / 2.0);
        glass_region::faded(
            glass_region::transformed(vec![glass], scale, centre, (0.0, 0.0)),
            opacity,
        )
    } else {
        Vec::new()
    };
    let widget = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .align_child(Start, Start)
        .child(
            sheet
                .opacity(opacity)
                .scale(scale)
                .translate(area.x, area.y),
        );
    Some(Built {
        widget: boxed(widget),
        frost,
        area,
    })
}

/// What's on the sheet: the sidebar, a hairline, the pane over the footer.
fn content(area: &Area) -> Rectangle {
    let (field, category) = with(|state| (state.field.clone(), state.category));
    let query = field.text().to_string();
    let searching = !query.trim().is_empty();

    let pane_x = SIDEBAR + Divider::width();
    let pane_w = (area.width - pane_x).max(1.0);
    let footer_h = sys::size::control::height() + sys::space::gap::m() * 2.0;
    let pane_h = (area.height - footer_h - Divider::height()).max(1.0);

    let sidebar = sidebar(&field, category, area.height);
    let pane = pane(
        category,
        &query,
        Area {
            x: area.x + pane_x,
            y: area.y,
            width: pane_w,
            height: pane_h,
        },
        searching,
    );
    let right = amane::Column::new(vec![
        boxed(pane),
        boxed(Divider::build()),
        boxed(footer(pane_w, footer_h)),
    ])
    .width(Size::Fixed(pane_w));

    Rectangle::new()
        .width(Size::Fixed(area.width))
        .height(Size::Fixed(area.height))
        .fill(Color::TRANSPARENT)
        .align_child(Start, Start)
        .child(
            Row::new(vec![
                boxed(sidebar),
                boxed(Divider::vertical(area.height)),
                boxed(right),
            ])
            .align(Start),
        )
}

/// The search field over the category rows, scrolling when they don't fit.
fn sidebar(field: &Field, picked: Category, height: f32) -> Rectangle {
    let inner_w = SIDEBAR - INSET * 2.0;
    let gap = sys::space::gap::m();
    let search = SearchField::new("amane.qs.customize.search", field, true)
        .placeholder("Search Controls")
        .width(inner_w)
        .on_clear(clear_query)
        .build();

    let items: Vec<ListItem> = Category::ALL
        .into_iter()
        .map(|category| {
            ListItem::new(
                format!("amane.qs.customize.category.{}", category.key()),
                category.title(),
            )
            .leading_tile(category.glyph())
            .bare(true)
            .selected(category == picked)
            .on_press(move || pick(category))
        })
        .collect();
    let rows = Category::ALL.len() as f32;
    let rows_h = rows * sys::size::row::height() + (rows - 1.0) * sys::space::gap::xs();
    let list = list::with_row_width(inner_w, || List::new(items).build());

    let list_h = (height - INSET * 2.0 - SearchField::height() - gap).max(1.0);
    let scroll = SIDE_SCROLL.with_borrow_mut(|scroll| {
        scroll.set_max((rows_h - list_h).max(0.0));
        scroll.settle_stale();
        scroll.value()
    });
    let list = ScrollView::new(Axis::Vertical, inner_w, list_h)
        .content(rows_h)
        .offset(scroll)
        .on_scroll(on_side_scroll)
        .build(list);

    Rectangle::new()
        .width(Size::Fixed(SIDEBAR))
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .padding(pad(INSET, INSET))
        .align_child(Start, Start)
        .child(
            amane::Column::new(vec![boxed(search), boxed(list)])
                .gap(gap)
                .width(Size::Fixed(inner_w)),
        )
}

/// The pane: the sections of controls, scrolling. `viewport` is where it
/// is on the surface, for the controls to record where they're drawn.
fn pane(category: Category, query: &str, viewport: Area, searching: bool) -> Rectangle {
    let hidden: Vec<ControlId> = layout::with(|layout| layout.hidden().collect());
    let sections = catalog::sections(category, query, &hidden);
    let inner_w = (viewport.width - INSET * 2.0).max(1.0);
    let px = Metrics::tokens();
    let title_style = sys::r#type::title();
    let title_h = text_height(title_style);
    let gap_m = sys::space::gap::m();
    let gap_l = sys::space::gap::l();

    // laid out by hand, top to bottom, so every control's place is known
    let mut y = INSET;
    let mut layers: Vec<Box<dyn Widget>> = Vec::new();
    // (id, rect in the pane's content)
    let mut placed: Vec<(ControlId, Area)> = Vec::new();
    let count = sections.len();
    for (i, section) in sections.into_iter().enumerate() {
        layers.push(boxed(
            Rectangle::new()
                .width(Size::Fixed(inner_w))
                .height(Size::Fixed(title_h))
                .fill(Color::TRANSPARENT)
                .align_child(Start, Center)
                .translate(INSET, y)
                .child(
                    Text::new(section.title)
                        .style(title_style)
                        .color(material::label(Level::Primary)),
                ),
        ));
        y += title_h + gap_m;

        if section.ids.is_empty() {
            let note = if searching {
                "No controls match"
            } else {
                "Nothing here"
            };
            let note_h = text_height(sys::r#type::caption());
            layers.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(inner_w))
                    .height(Size::Fixed(note_h))
                    .fill(Color::TRANSPARENT)
                    .align_child(Start, Center)
                    .translate(INSET, y)
                    .child(caption(note)),
            ));
            y += note_h;
        } else {
            let mut flow = Flow::new(inner_w, px.gap, gap_l);
            let mut cells: Vec<(ControlId, Slot, f32)> = Vec::new();
            for id in &section.ids {
                let (item, item_w, item_h, slot) = catalog_item(*id, &px);
                flow = flow.item(item_w, item_h, item);
                cells.push((*id, slot, item_w));
            }
            let (flow, flow_h, spots) = flow.build();
            for ((id, slot, item_w), spot) in cells.into_iter().zip(spots) {
                placed.push((
                    id,
                    Area {
                        x: INSET + spot.x + (item_w - slot.width) / 2.0,
                        y: y + spot.y,
                        width: slot.width,
                        height: slot.height,
                    },
                ));
            }
            layers.push(boxed(flow.translate(INSET, y)));
            y += flow_h;
        }

        if i + 1 < count {
            y += gap_l;
            layers.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(inner_w))
                    .height(Size::Fixed(Divider::height()))
                    .fill(Color::TRANSPARENT)
                    .translate(INSET, y)
                    .child(Divider::build()),
            ));
            y += Divider::height() + gap_l;
        }
    }
    let content_h = y + INSET;

    let scroll = SCROLL.with_borrow_mut(|scroll| {
        scroll.set_max((content_h - viewport.height).max(0.0));
        scroll.settle_stale();
        if scroll.swiping() {
            moved();
        }
        scroll.value()
    });

    // where each control is on the surface, inside the pane
    with(|state| {
        state.tiles = placed
            .into_iter()
            .filter_map(|(id, at)| {
                let on_surface = Area {
                    x: viewport.x + at.x,
                    y: viewport.y + at.y - scroll,
                    ..at
                };
                on_surface.clipped(&viewport).map(|area| (id, area))
            })
            .collect();
    });

    let content = Rectangle::new()
        .width(Size::Fixed(viewport.width))
        .height(Size::Fixed(content_h.max(viewport.height)))
        .fill(Color::TRANSPARENT)
        .child(Stack::new(layers));
    ScrollView::new(Axis::Vertical, viewport.width, viewport.height)
        .content(content_h)
        .offset(scroll)
        .scrollbar(scrollbar())
        .on_scroll(on_pane_scroll)
        .build(content)
}

/// One control in the pane: the control at its default size on a plain
/// tint, taking nothing but the hand, with its name centred under it on
/// up to two lines. Gives the item, how wide and tall it is, and the
/// control's slot within it (at the top, centred).
fn catalog_item(id: ControlId, px: &Metrics) -> (Rectangle, f32, f32, Slot) {
    let span = id.default_span();
    let slot = px.slot(Rect::new(Place::new(0, 0), span));
    // a round button's name needs more room than its disc
    let item_w = slot.width.max(px.cell + px.gap);
    let control = tile::plain(|| {
        controls::build(id, span, &Ctx::new(true)).unwrap_or_else(|| tiles::placeholder(id, slot))
    });
    let control = edit::body(control, slot);

    let style = quick::r#type::caption();
    let line_h = quick::r#type::line().max(text_height(style));
    let lines = wrapped_lines(id.title(), style, item_w, 2);
    let gap = sys::space::gap::xs();
    let caption_h = lines.len() as f32 * line_h;
    let item_h = slot.height + gap + caption_h;

    let name: Vec<Box<dyn Widget>> = lines
        .into_iter()
        .map(|line| {
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(item_w))
                    .height(Size::Fixed(line_h))
                    .fill(Color::TRANSPARENT)
                    .align_child(Center, Center)
                    .child(
                        Text::new(line)
                            .style(style)
                            .color(material::label(Level::Primary)),
                    ),
            )
        })
        .collect();

    let item = Rectangle::new()
        .width(Size::Fixed(item_w))
        .height(Size::Fixed(item_h))
        .fill(Color::TRANSPARENT)
        .child(Stack::new(vec![
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(slot.width))
                    .height(Size::Fixed(slot.height))
                    .fill(Color::TRANSPARENT)
                    .translate((item_w - slot.width) / 2.0, 0.0)
                    .child(Stack::new(vec![control])),
            ),
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(item_w))
                    .height(Size::Fixed(caption_h))
                    .fill(Color::TRANSPARENT)
                    .translate(0.0, slot.height + gap)
                    .child(amane::Column::new(name)),
            ),
        ]));
    (item, item_w, item_h, slot)
}

/// The hint and, at the end, Undo Remove (while there's something to
/// undo), Reset Layout and Done, the one main action, in the accent.
fn footer(width: f32, height: f32) -> Rectangle {
    let mut actions: Vec<Box<dyn Widget>> = Vec::new();
    if edit::can_undo() {
        actions.push(boxed(
            PushButton::new("amane.qs.customize.undo", "Undo Remove")
                .style(Style::Plain)
                .shape(Shape::Round)
                .on_press(edit::undo)
                .build(),
        ));
    }
    actions.push(boxed(
        PushButton::new("amane.qs.customize.reset", "Reset Layout")
            .style(Style::Plain)
            .shape(Shape::Round)
            .on_press(edit::reset)
            .build(),
    ));
    actions.push(boxed(
        PushButton::new("amane.qs.customize.done", "Done")
            .style(Style::Filled)
            .shape(Shape::Round)
            .text(sys::r#type::label())
            .on_press(edit::leave)
            .build(),
    ));
    let hint = caption("Drag a control to place it in Quick Settings.");
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .padding(pad(INSET, 0.0))
        .align_child(Start, Center)
        .child(split_row(hint, actions))
}

/// How much the scrollbar shows: while the pane moves, and a little after.
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
        motion::redraw_after::<super::QuickSettings>(due.saturating_duration_since(now));
    }
    SCROLLBAR.with_borrow_mut(|bar| {
        bar.to(if showing { 1.0 } else { 0.0 });
        bar.value().clamp(0.0, 1.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Density, Mode};
    use crate::tokens::{Key as TokenKey, with_key};

    fn placed(screen: (f32, f32), card_x: f32) -> Area {
        with_key(TokenKey::new(Mode::Dark, Density::Comfortable), || {
            place(
                screen,
                Area {
                    x: card_x,
                    y: 0.0,
                    width: 392.0,
                    height: 800.0,
                },
            )
        })
    }

    #[test]
    fn the_window_is_centred_and_clears_the_card() {
        // a wide screen: in the middle, the card far to the right
        let on_wide = placed((2560.0, 1440.0), 2150.0);
        assert_eq!((on_wide.width, on_wide.height), (WIDTH, HEIGHT));
        assert_eq!(on_wide.x, ((2560.0 - WIDTH) / 2.0_f32).round());
        assert_eq!(on_wide.y, ((1440.0 - HEIGHT) / 2.0_f32).round());
        // a narrower one: moved left so a gap stays between it and the card
        let on_narrow = placed((1920.0, 1200.0), 1510.0);
        assert!(on_narrow.x + on_narrow.width < 1510.0);
        assert!(on_narrow.x > 0.0);
    }

    #[test]
    fn a_short_screen_shortens_the_window() {
        let short = placed((1920.0, 900.0), 1510.0);
        assert!(short.height < HEIGHT);
        assert!(short.height < 900.0);
        assert!(short.y >= 0.0);
    }

    #[test]
    fn an_area_clips_to_another() {
        let pane = Area {
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 200.0,
        };
        let inside = Area {
            x: 150.0,
            y: 150.0,
            width: 20.0,
            height: 20.0,
        };
        assert_eq!(inside.clipped(&pane), Some(inside));
        let across = Area {
            x: 250.0,
            y: 280.0,
            width: 100.0,
            height: 100.0,
        };
        assert_eq!(
            across.clipped(&pane),
            Some(Area {
                x: 250.0,
                y: 280.0,
                width: 50.0,
                height: 20.0
            })
        );
        let out = Area {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        };
        assert_eq!(out.clipped(&pane), None);
        assert!(pane.contains(Point { x: 100.0, y: 100.0 }));
        assert!(!pane.contains(Point { x: 300.0, y: 100.0 }));
    }
}
