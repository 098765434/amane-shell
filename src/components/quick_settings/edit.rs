//! Edit mode — iOS's "Edit Controls": the grid's controls go inert and can
//! be dragged to another cell, resized by their corner handle, removed by
//! their badge and added back from a list. The layout they end up in is
//! kept (`layout.rs`).
//!
//! A drag is measured on one still layer over the whole surface
//! (`customize::drag_layer`, under the card and the Customize window
//! both), never on the control itself: amane measures a drag
//! against the hit areas of the last frame drawn, and a control on the
//! move is drawn somewhere new each frame, so a drag measured on it
//! counts the pointer's motion against where the control *was*. With a
//! mouse reporting faster than the screen refreshes that happens several
//! times a frame, and the control overshoots and snaps back every frame.
//! The layer doesn't move, so the pointer's position on it is the pointer's
//! position on the canvas, and the held control sits exactly where the
//! pointer took it: the pointer's position less where it was grabbed.
//!
//! The others make way live, as on iOS, each preview worked out from the
//! layout the drag began from (`components::grid::drag::Session`).
//!
//! amane finds the target being dragged by its position in the frame's hit
//! areas, which has to stay the same from frame to frame while a drag is
//! in flight. The layer comes first on the surface, so nothing that
//! changes under a drag comes before it.

use std::cell::{Cell, RefCell};

use amane::{
    Animation, Color, Cursor, Parent, Point, Radius, Rectangle, Size, Stack, Widget, mark_all,
};

use super::controls::ControlId;
use super::customize;
use super::grid::{Canvas, Metrics, Occupancy, Rect, Slot, Span};
use super::layout::{self, Layout};
use super::tiles::{SCOPE, radius_of};
use crate::components::buttons::{IconButton, Shape, Style};
use crate::components::control_size::ControlSize;
use crate::components::grid::drag::{self, Hit, Origin, Session, follow, hit, off_canvas};
use crate::components::icons::{ICON_REMOVE, ICON_RESIZE};
use crate::components::layout::{boxed, icon};
use crate::components::material::{self, FillLevel, Level};
use crate::components::motion::{self, Pose};
use crate::tokens::{quick, sys};

// TODO(token): `quick.drag.band` once Penpot has it (manual §1.1).
/// How far past the middle of the next cell a held control goes before it
/// changes cell, in cells: the dead band that keeps the grid still while
/// the hand rests on a boundary.
const BAND: f32 = 0.1;

/// What the pointer holds.
pub(super) type Drag = drag::Drag<ControlId>;

thread_local! {
    static EDITING: Cell<bool> = const { Cell::new(false) };
    // the drag in flight: the layout it began from, its preview, and what
    // the pointer holds
    static SESSION: RefCell<Option<Session<ControlId>>> = const { RefCell::new(None) };
    // this frame's placement, which a press reads to find what's under it
    static SLOTS: RefCell<Vec<(ControlId, Slot)>> = const { RefCell::new(Vec::new()) };
    // 0 … 1: the badges, handles and ghost discs fading in
    static CHROME: RefCell<Animation> = RefCell::new(motion::fade(0.0, motion::FADE));
    // the layout before the last control was removed, to put it back
    static UNDO: RefCell<Option<Layout>> = const { RefCell::new(None) };
}

pub(super) fn editing() -> bool {
    EDITING.get()
}

pub(super) fn drag() -> Option<Drag> {
    SESSION.with_borrow(|session| session.as_ref().map(|session| session.drag))
}

/// The control on the move, drawn over the rest.
pub(super) fn held() -> Option<ControlId> {
    match drag() {
        Some(Drag::Move { id, .. }) => Some(id),
        _ => None,
    }
}

pub(super) fn toggle() {
    if editing() {
        leave();
    } else {
        enter();
    }
}

pub(super) fn enter() {
    if !EDITING.replace(true) {
        CHROME.with_borrow_mut(|chrome| {
            *chrome = motion::fade(0.0, motion::FADE);
            chrome.to(1.0);
        });
        customize::open();
        mark_all();
    }
}

/// How far in edit mode's dressing has faded.
fn chrome() -> f32 {
    CHROME
        .with_borrow_mut(|chrome| chrome.value())
        .clamp(0.0, 1.0)
}

/// Out of edit mode, keeping the layout (a drag in flight is dropped).
pub(super) fn leave() {
    if !EDITING.replace(false) {
        return;
    }
    SESSION.with_borrow_mut(|session| *session = None);
    UNDO.with_borrow_mut(|undo| *undo = None);
    layout::with(|layout| layout.save());
    customize::close();
    mark_all();
}

/// Lets go of a drag in flight without keeping what it previewed; whether
/// there was one.
pub(super) fn cancel() -> bool {
    if SESSION.with_borrow_mut(Option::take).is_none() {
        return false;
    }
    mark_all();
    true
}

/// The grid tells edit mode where everything is this frame.
pub(super) fn placed(slots: Vec<(ControlId, Slot)>) {
    SLOTS.with_borrow_mut(|known| *known = slots);
}

/// The layout to draw: the drag's preview while one is in flight, else
/// the layout as kept.
pub(super) fn shown_layout() -> Layout {
    SESSION
        .with_borrow(|session| session.as_ref().and_then(|s| s.preview.clone()))
        .unwrap_or_else(|| layout::with(|layout| layout.clone()))
}

/// Where a control is drawn this frame when the pointer holds it.
pub(super) fn held_pose(id: ControlId) -> Option<Pose> {
    match drag() {
        Some(Drag::Move { id: held, at, .. }) if held == id => Some(Pose {
            scale: quick::drag::scale(),
            ..Pose::at(at.1).moved(at.0)
        }),
        _ => None,
    }
}

/// The control on the move and where its top-left is, in the grid's
/// pixels, for whoever draws it outside the grid.
pub(super) fn held_at() -> Option<(ControlId, (f32, f32))> {
    match drag() {
        Some(Drag::Move { id, at, .. }) => Some((id, at)),
        _ => None,
    }
}

/// Takes hold of `id` from the Customize window's catalog: the pointer at
/// `point` (in the grid's pixels, wherever that is) holds it `grab` in
/// from its top-left. On the canvas it's the same as a control picked up
/// there; off the canvas it has no cells yet. A control already on the
/// canvas is picked up from its cells.
pub(super) fn lift(id: ControlId, grab: Point, point: Point) {
    if drag().is_some() {
        return;
    }
    let origin = if layout::with(|layout| layout.rect_of(id).is_some()) {
        Origin::Canvas
    } else {
        Origin::Window
    };
    begin(Drag::Move {
        id,
        grab,
        at: follow(grab, point),
        place: None,
        origin,
        pointer: point,
    });
    motion(point);
}

/// A drag begins: the layout as it is now is what every preview starts from.
fn begin(drag: Drag) {
    let base = layout::with(|layout| layout.clone());
    SESSION.with_borrow_mut(|session| *session = Some(Session::new(base, drag)));
}

/// The pointer moved with the button down: the drag in flight follows it.
fn motion(point: Point) {
    SESSION.with_borrow_mut(|session| {
        if let Some(session) = session {
            session.motion(point, &Metrics::tokens(), BAND);
        }
    });
    mark_all();
}

/// The drag is over: what it previewed is kept.
fn commit() {
    if let Some(layout) = SESSION
        .with_borrow_mut(Option::take)
        .and_then(|session| session.preview)
    {
        layout::replace(layout);
    }
}

// ───── the drag, measured on the canvas ─────────────────────────────────────

/// The pointer is at `point` on the canvas with the button down: carries
/// on the drag in flight, or starts one on what's under it.
pub(super) fn drag_at(point: Point) {
    if drag().is_some() {
        motion(point);
        return;
    }
    let handle = handle_side();
    match SLOTS.with_borrow(|slots| hit(slots, point, handle)) {
        Some(Hit::Body(id, slot)) => {
            begin(Drag::Move {
                id,
                grab: Point {
                    x: point.x - slot.x,
                    y: point.y - slot.y,
                },
                at: (slot.x, slot.y),
                place: None,
                origin: Origin::Canvas,
                pointer: point,
            });
            motion(point);
        }
        Some(Hit::Handle(id, slot)) => {
            begin(Drag::Resize {
                id,
                grab: point,
                base: slot,
                pulled: (slot.width, slot.height),
                span: None,
            });
            motion(point);
        }
        None => {}
    }
}

/// The button came up. A control let go off the canvas (by more than half
/// a cell beyond its edge) doesn't land: one from the canvas is removed,
/// as a module dragged out of Control Center is, and one from the
/// Customize window goes back to it.
pub(super) fn release() {
    match drag() {
        Some(Drag::Move {
            id,
            at,
            origin,
            pointer,
            place,
            ..
        }) => {
            let canvas = SESSION.with_borrow(|session| session.as_ref().map(|s| s.base.canvas));
            let off = canvas.is_some_and(|canvas| {
                off_canvas(&Metrics::tokens(), &canvas, pointer) || place.is_none()
            });
            if off {
                cancel();
                if origin == Origin::Canvas {
                    hide(id);
                }
            } else {
                // the control springs from where it was let go, lift and all
                motion::jump(
                    SCOPE,
                    id.key(),
                    Pose {
                        scale: quick::drag::scale(),
                        ..Pose::at(at.1).moved(at.0)
                    },
                );
                commit();
            }
        }
        Some(Drag::Resize { .. }) => commit(),
        None => {}
    }
    mark_all();
}

fn handle_side() -> f32 {
    ControlSize::Medium.height()
}

/// The pull a resize in flight has made of `id`, for its ghost.
fn pulling(id: ControlId) -> Option<(f32, f32)> {
    match drag() {
        Some(Drag::Resize {
            id: held, pulled, ..
        }) if held == id => Some(pulled),
        _ => None,
    }
}

// ───── hiding and adding ───────────────────────────────────────────────────

pub(super) fn hide(id: ControlId) {
    layout::with(|layout| {
        UNDO.with_borrow_mut(|undo| *undo = Some(layout.clone()));
        layout.hide(id);
        layout.save();
    });
    mark_all();
}

/// Whether a removed control can be put back where it was.
pub(super) fn can_undo() -> bool {
    UNDO.with_borrow(Option::is_some)
}

/// Puts the last removed control back where it was.
pub(super) fn undo() {
    if let Some(layout) = UNDO.with_borrow_mut(Option::take) {
        layout::replace(layout);
    }
    mark_all();
}

/// Control Center's order again, for the controls this machine has.
pub(super) fn reset() {
    UNDO.with_borrow_mut(|undo| *undo = None);
    layout::replace(Layout::default_for(Canvas::tokens(), &|id| {
        super::controls::available(id)
    }));
    mark_all();
}

// ───── the layers ───────────────────────────────────────────────────────────

/// `control` as edit mode shows it: inert, under a surface that only names
/// the grab cursor (the drag layer beneath takes the drag).
pub(super) fn body(control: Box<dyn Widget>, slot: Slot) -> Box<dyn Widget> {
    let surface = Rectangle::new()
        .width(Size::Fixed(slot.width))
        .height(Size::Fixed(slot.height))
        .fill(Color::TRANSPARENT)
        .cursor(Cursor::Grab);
    boxed(Stack::new(vec![control, boxed(surface)]))
}

/// What edit mode lays over control `id` at `slot`, drawn at `pose` so it
/// follows the control: its remove badge and, when it comes in more than
/// one size, its resize handle and the outline a pull in flight draws.
pub(super) fn dressing(id: ControlId, slot: Slot, pose: Pose) -> Box<dyn Widget> {
    let side = handle_side();

    let badge = IconButton::new(format!("amane.qs.edit.{}.remove", id.name()), ICON_REMOVE)
        .style(Style::Tonal)
        .shape(Shape::Round)
        .size(sys::size::icon::s())
        .height(side)
        .width(side)
        .color(material::label(Level::Primary))
        .on_press(move || hide(id))
        .build();
    let badge = Rectangle::new()
        .width(Size::Fixed(slot.width))
        .height(Size::Fixed(slot.height))
        .fill(Color::TRANSPARENT)
        .align_child(amane::Start, amane::Start)
        .child(badge);

    let mut layers: Vec<Box<dyn Widget>> = Vec::new();

    if id.supported().len() > 1 {
        if let Some((w, h)) = pulling(id) {
            // the pull itself, so the hand sees how far it has come; the
            // control snaps to the sizes it comes in
            let px = Metrics::tokens();
            let (w, h) = (w.max(px.cell), h.max(px.cell));
            let ghost = Rectangle::new()
                .width(Size::Fixed(w))
                .height(Size::Fixed(h))
                .radius(radius_of(
                    id,
                    Span {
                        cols: 1,
                        rows: if h > px.cell * 1.5 { 2 } else { 1 },
                    },
                    Slot { height: h, ..slot },
                ))
                .fill(Color::TRANSPARENT)
                .border(sys::border::hairline(), sys::color::accent::bg());
            layers.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(slot.width))
                    .height(Size::Fixed(slot.height))
                    .fill(Color::TRANSPARENT)
                    .align_child(amane::Start, amane::Start)
                    .child(ghost),
            ));
        }
        let handle = Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(Radius::Full)
            .fill(material::fill_role(FillLevel::Secondary))
            .align_child(amane::Center, amane::Center)
            .cursor(Cursor::ResizeBottomRight)
            .child(icon(
                ICON_RESIZE,
                sys::size::icon::s(),
                material::label(Level::Primary),
            ));
        layers.push(boxed(
            Rectangle::new()
                .width(Size::Fixed(slot.width))
                .height(Size::Fixed(slot.height))
                .fill(Color::TRANSPARENT)
                .align_child(amane::End, amane::End)
                .child(handle),
        ));
    }
    layers.push(boxed(badge));

    boxed(
        Rectangle::new()
            .width(Size::Fixed(slot.width))
            .height(Size::Fixed(slot.height))
            .fill(Color::TRANSPARENT)
            .translate(pose.x, pose.y)
            .scale(pose.scale)
            .opacity(chrome())
            .child(Stack::new(layers)),
    )
}

/// The faint discs the free cells show in edit mode, as iOS draws the
/// places a control can go. One disc per cell, the taken ones clear, so
/// the layer is the same shape every frame.
pub(super) fn ghost_grid(
    px: &Metrics,
    canvas: &Canvas,
    occupancy: &Occupancy<ControlId>,
) -> Box<dyn Widget> {
    let circles: Vec<Box<dyn Widget>> = canvas
        .cells()
        .map(|place| {
            let slot = px.slot(Rect::new(place, Span::S));
            let fill = if occupancy.at(place).is_none() {
                sys::color::state::selected()
            } else {
                Color::TRANSPARENT
            };
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(slot.width))
                    .height(Size::Fixed(slot.height))
                    .radius(Radius::Full)
                    .fill(fill)
                    .translate(slot.x, slot.y),
            )
        })
        .collect();
    boxed(
        Rectangle::new()
            .width(Parent)
            .height(Parent)
            .fill(Color::TRANSPARENT)
            .opacity(chrome())
            .child(Stack::new(circles)),
    )
}

/// Where the held control will land: an outline of the cells the preview
/// gives it. Always one layer, empty when nothing is held.
pub(super) fn drop_outline(px: &Metrics, layout: &Layout) -> Box<dyn Widget> {
    let Some((id, rect)) = held().and_then(|id| Some((id, layout.rect_of(id)?))) else {
        return boxed(
            Rectangle::new()
                .width(Size::Fixed(0.0))
                .height(Size::Fixed(0.0)),
        );
    };
    let slot = px.slot(rect);
    boxed(
        Rectangle::new()
            .width(Size::Fixed(slot.width))
            .height(Size::Fixed(slot.height))
            .radius(radius_of(id, rect.span, slot))
            .fill(Color::TRANSPARENT)
            .border(sys::border::hairline(), sys::color::accent::bg())
            .translate(slot.x, slot.y),
    )
}
