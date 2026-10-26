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
//! The others make way live, as on iOS. Every preview is worked out from
//! the layout as it was when the drag began (the base), never from the last
//! preview, so the same cell always gives the same arrangement and moving
//! back restores the base. A cell where nothing fits keeps the last preview
//! that did, as iOS stays where it last fit. The cell a control is in is
//! kept until the control is well past the boundary (`Metrics::settle`),
//! so a hand resting on a boundary doesn't make the grid flap.
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
use super::grid::{Canvas, Metrics, Occupancy, Place, Rect, Slot, Span};
use super::layout::{self, Layout};
use super::tiles::{SCOPE, radius_of};
use crate::components::buttons::{IconButton, Shape, Style};
use crate::components::control_size::ControlSize;
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

/// Where a control on the move was taken from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Origin {
    /// Its cells on the canvas.
    Canvas,
    /// The Customize window's catalog: it has no cells until the preview
    /// gives it some, and goes back to the catalog if let go elsewhere.
    Window,
}

/// What the pointer holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Drag {
    /// A control on the move: where on it the pointer took hold (from its
    /// top-left), where the pointer has its top-left now, in the grid's
    /// pixels, the cell the preview has it in, where it came from, and
    /// where the pointer last was (the grid's pixels too).
    Move {
        id: ControlId,
        grab: Point,
        at: (f32, f32),
        place: Option<Place>,
        origin: Origin,
        pointer: Point,
    },
    /// A control's corner handle: where the pointer took hold, the control
    /// as it was then, how big the pull has made it, and the size the
    /// preview gives it.
    Resize {
        id: ControlId,
        grab: Point,
        base: Slot,
        pulled: (f32, f32),
        span: Option<Span>,
    },
}

/// What a press on the canvas lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Hit {
    Body(ControlId, Slot),
    Handle(ControlId, Slot),
}

thread_local! {
    static EDITING: Cell<bool> = const { Cell::new(false) };
    static DRAG: Cell<Option<Drag>> = const { Cell::new(None) };
    // this frame's placement, which a press reads to find what's under it
    static SLOTS: RefCell<Vec<(ControlId, Slot)>> = const { RefCell::new(Vec::new()) };
    // the layout the drag began from, and the one it previews
    static BASE: RefCell<Option<Layout>> = const { RefCell::new(None) };
    static PREVIEW: RefCell<Option<Layout>> = const { RefCell::new(None) };
    // 0 … 1: the badges, handles and ghost discs fading in
    static CHROME: RefCell<Animation> = RefCell::new(motion::fade(0.0, motion::FADE));
    // the layout before the last control was removed, to put it back
    static UNDO: RefCell<Option<Layout>> = const { RefCell::new(None) };
}

pub(super) fn editing() -> bool {
    EDITING.get()
}

pub(super) fn drag() -> Option<Drag> {
    DRAG.get()
}

/// The control on the move, drawn over the rest.
pub(super) fn held() -> Option<ControlId> {
    match DRAG.get() {
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
    DRAG.set(None);
    BASE.with_borrow_mut(|base| *base = None);
    PREVIEW.with_borrow_mut(|preview| *preview = None);
    UNDO.with_borrow_mut(|undo| *undo = None);
    layout::with(|layout| layout.save());
    customize::close();
    mark_all();
}

/// Lets go of a drag in flight without keeping what it previewed; whether
/// there was one.
pub(super) fn cancel() -> bool {
    if DRAG.take().is_none() {
        return false;
    }
    BASE.with_borrow_mut(|base| *base = None);
    PREVIEW.with_borrow_mut(|preview| *preview = None);
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
    PREVIEW
        .with_borrow(|preview| preview.clone())
        .unwrap_or_else(|| layout::with(|layout| layout.clone()))
}

/// Where a control is drawn this frame when the pointer holds it.
pub(super) fn held_pose(id: ControlId) -> Option<Pose> {
    match DRAG.get() {
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
    match DRAG.get() {
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
    if DRAG.get().is_some() {
        return;
    }
    begin();
    let origin = if layout::with(|layout| layout.rect_of(id).is_some()) {
        Origin::Canvas
    } else {
        Origin::Window
    };
    DRAG.set(Some(Drag::Move {
        id,
        grab,
        at: follow(grab, point),
        place: None,
        origin,
        pointer: point,
    }));
    drag_move(point);
}

/// A drag begins: the layout as it is now is what every preview starts from.
fn begin() {
    let base = layout::with(|layout| layout.clone());
    BASE.with_borrow_mut(|slot| *slot = Some(base));
    PREVIEW.with_borrow_mut(|preview| *preview = None);
}

/// Previews `f` of the base: a layout it gives replaces the preview and
/// is `true`; one it can't keeps the last.
fn preview(f: impl FnOnce(&Layout) -> Option<Layout>) -> bool {
    let next = BASE.with_borrow(|base| base.as_ref().and_then(f));
    match next {
        Some(layout) => {
            PREVIEW.with_borrow_mut(|preview| *preview = Some(layout));
            true
        }
        None => false,
    }
}

/// The drag is over: what it previewed is kept.
fn commit() {
    DRAG.set(None);
    BASE.with_borrow_mut(|base| *base = None);
    if let Some(layout) = PREVIEW.with_borrow_mut(Option::take) {
        layout::replace(layout);
    }
}

// ───── the drag, measured on the canvas ─────────────────────────────────────

/// The pointer is at `point` on the canvas with the button down: carries
/// on the drag in flight, or starts one on what's under it.
pub(super) fn drag_at(point: Point) {
    match DRAG.get() {
        Some(Drag::Move { .. }) => drag_move(point),
        Some(Drag::Resize { .. }) => drag_resize(point),
        None => {
            let handle = handle_side();
            match SLOTS.with_borrow(|slots| hit(slots, point, handle)) {
                Some(Hit::Body(id, slot)) => {
                    begin();
                    DRAG.set(Some(Drag::Move {
                        id,
                        grab: Point {
                            x: point.x - slot.x,
                            y: point.y - slot.y,
                        },
                        at: (slot.x, slot.y),
                        place: None,
                        origin: Origin::Canvas,
                        pointer: point,
                    }));
                    drag_move(point);
                }
                Some(Hit::Handle(id, slot)) => {
                    begin();
                    DRAG.set(Some(Drag::Resize {
                        id,
                        grab: point,
                        base: slot,
                        pulled: (slot.width, slot.height),
                        span: None,
                    }));
                    drag_resize(point);
                }
                None => {}
            }
        }
    }
}

/// The button came up. A control let go off the canvas (by more than half
/// a cell beyond its edge) doesn't land: one from the canvas is removed,
/// as a module dragged out of Control Center is, and one from the
/// Customize window goes back to it.
pub(super) fn release() {
    match DRAG.get() {
        Some(Drag::Move {
            id,
            at,
            origin,
            pointer,
            place,
            ..
        }) => {
            let canvas = BASE.with_borrow(|base| base.as_ref().map(|b| b.canvas));
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

/// Whether `point` (in the grid's pixels) is clearly off the canvas: more
/// than half a cell beyond any edge.
pub(super) fn off_canvas(px: &Metrics, canvas: &Canvas, point: Point) -> bool {
    let margin = px.cell / 2.0;
    point.x < -margin
        || point.y < -margin
        || point.x > px.extent(canvas.cols) + margin
        || point.y > px.extent(canvas.rows) + margin
}

/// What `point` on the canvas lands on this frame: the corner handle of a
/// control that comes in more than one size (its bottom-right `handle`
/// square), else the control itself, else nothing. The remove badge isn't
/// looked for: it's a button of its own, above the layer.
pub(super) fn hit(slots: &[(ControlId, Slot)], point: Point, handle: f32) -> Option<Hit> {
    let (id, slot) = slots.iter().copied().find(|(_, slot)| {
        point.x >= slot.x
            && point.y >= slot.y
            && point.x < slot.x + slot.width
            && point.y < slot.y + slot.height
    })?;
    let on_handle = id.supported().len() > 1
        && point.x >= slot.x + slot.width - handle
        && point.y >= slot.y + slot.height - handle;
    Some(if on_handle {
        Hit::Handle(id, slot)
    } else {
        Hit::Body(id, slot)
    })
}

/// The top-left a control grabbed `grab` in from its corner has when the
/// pointer is at `point`.
pub(super) fn follow(grab: Point, point: Point) -> (f32, f32) {
    (point.x - grab.x, point.y - grab.y)
}

/// How big a pull from `grab` to `point` makes a control that was `base`.
pub(super) fn pulled(base: Slot, grab: Point, point: Point) -> (f32, f32) {
    (
        base.width + point.x - grab.x,
        base.height + point.y - grab.y,
    )
}

fn handle_side() -> f32 {
    ControlSize::Medium.height()
}

fn drag_move(point: Point) {
    let Some(Drag::Move {
        id,
        grab,
        place,
        origin,
        ..
    }) = DRAG.get()
    else {
        return;
    };
    let at = follow(grab, point);

    let (canvas, span) = BASE.with_borrow(|base| {
        let base = base.as_ref();
        (base.map(|b| b.canvas), base.and_then(|b| b.span_of(id)))
    });
    let mut place = place;
    if let (Some(canvas), Some(span)) = (canvas, span) {
        let px = Metrics::tokens();
        if off_canvas(&px, &canvas, point) {
            // off the canvas it has no cells: the others go back to where
            // they were, and a control from the canvas leaves a hole
            if place.is_some() {
                place = None;
                PREVIEW.with_borrow_mut(|preview| {
                    *preview = BASE.with_borrow(|base| {
                        base.as_ref().map(|base| {
                            let mut lifted = base.clone();
                            lifted.hide(id);
                            lifted
                        })
                    });
                });
            }
        } else {
            let nearest = px.settle(&canvas, at.0, at.1, span, place, BAND);
            if place != Some(nearest) && preview(|base| base.placed_at(id, span, nearest)) {
                place = Some(nearest);
            }
        }
    }
    DRAG.set(Some(Drag::Move {
        id,
        grab,
        at,
        place,
        origin,
        pointer: point,
    }));
    mark_all();
}

fn drag_resize(point: Point) {
    let Some(Drag::Resize {
        id,
        grab,
        base,
        span,
        ..
    }) = DRAG.get()
    else {
        return;
    };
    let px = Metrics::tokens();
    let pulled = pulled(base, grab, point);
    let nearest = nearest_span_from(span, id.supported(), pulled.0, pulled.1, &px, px.gap / 2.0);
    let mut span = span;
    if span != Some(nearest) && preview(|base| base.resized(id, nearest)) {
        span = Some(nearest);
    }
    DRAG.set(Some(Drag::Resize {
        id,
        grab,
        base,
        pulled,
        span,
    }));
    mark_all();
}

/// The size among `supported` nearest `width` by `height`, in `px`; unless
/// `current` is set and no other size is nearer by more than `band`
/// pixels: the size in hand holds until another is clearly closer.
pub(super) fn nearest_span_from(
    current: Option<Span>,
    supported: &[Span],
    width: f32,
    height: f32,
    px: &Metrics,
    band: f32,
) -> Span {
    let distance = |s: Span| {
        let dw = px.width(s) - width;
        let dh = px.height(s) - height;
        (dw * dw + dh * dh).sqrt()
    };
    let nearest = *supported
        .iter()
        .min_by(|a, b| distance(**a).total_cmp(&distance(**b)))
        .unwrap_or(&Span::M);
    match current {
        Some(current)
            if supported.contains(&current) && distance(current) <= distance(nearest) + band =>
        {
            current
        }
        _ => nearest,
    }
}

/// The pull a resize in flight has made of `id`, for its ghost.
fn pulling(id: ControlId) -> Option<(f32, f32)> {
    match DRAG.get() {
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

#[cfg(test)]
mod tests {
    use super::*;

    const PX: Metrics = Metrics {
        cell: 74.0,
        gap: 16.0,
    };

    fn at(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    fn nearest_span(supported: &[Span], width: f32, height: f32, px: &Metrics) -> Span {
        nearest_span_from(None, supported, width, height, px, 0.0)
    }

    #[test]
    fn a_pull_snaps_to_the_nearest_size_it_comes_in() {
        let round = [Span::S, Span::M];
        assert_eq!(nearest_span(&round, 74.0 + 80.0, 74.0, &PX), Span::M);
        assert_eq!(nearest_span(&round, 74.0 + 20.0, 74.0, &PX), Span::S);
        assert_eq!(nearest_span(&round, 400.0, 400.0, &PX), Span::M);
        let square = [Span::L, Span::FULL];
        // pulled down more than across, a square stays square
        assert_eq!(nearest_span(&square, 200.0, 150.0, &PX), Span::L);
        assert_eq!(nearest_span(&square, 330.0, 80.0, &PX), Span::FULL);
        assert_eq!(nearest_span(&[Span::FULL], 10.0, 10.0, &PX), Span::FULL);
        let tile = [Span::M, Span::W3, Span::FULL];
        assert_eq!(nearest_span(&tile, 164.0 + 60.0, 74.0, &PX), Span::W3);
    }

    #[test]
    fn a_size_in_hand_holds_until_another_is_clearly_nearer() {
        let round = [Span::S, Span::M];
        // halfway between S (74) and M (164) is 119: a hair past it, without
        // a band M wins, with one S holds
        assert_eq!(
            nearest_span_from(Some(Span::S), &round, 121.0, 74.0, &PX, 0.0),
            Span::M
        );
        assert_eq!(
            nearest_span_from(Some(Span::S), &round, 121.0, 74.0, &PX, 8.0),
            Span::S
        );
        assert_eq!(
            nearest_span_from(Some(Span::S), &round, 130.0, 74.0, &PX, 8.0),
            Span::M
        );
        // and back the same way
        assert_eq!(
            nearest_span_from(Some(Span::M), &round, 117.0, 74.0, &PX, 8.0),
            Span::M
        );
        assert_eq!(
            nearest_span_from(Some(Span::M), &round, 108.0, 74.0, &PX, 8.0),
            Span::S
        );
        // a size it doesn't come in can't hold
        assert_eq!(
            nearest_span_from(Some(Span::FULL), &round, 74.0, 74.0, &PX, 8.0),
            Span::S
        );
    }

    #[test]
    fn the_same_pointer_position_twice_does_not_move_the_tile() {
        let grab = at(30.0, 20.0);
        let first = follow(grab, at(130.0, 120.0));
        assert_eq!(first, follow(grab, at(130.0, 120.0)));
        assert_eq!(first, (100.0, 100.0));
        // a step of the pointer is a step of the tile, nothing more
        assert_eq!(follow(grab, at(135.0, 118.0)), (105.0, 98.0));
    }

    #[test]
    fn a_pull_is_measured_from_where_the_handle_was_grabbed() {
        let base = Slot {
            x: 90.0,
            y: 0.0,
            width: 74.0,
            height: 74.0,
        };
        let grab = at(160.0, 70.0);
        assert_eq!(pulled(base, grab, grab), (74.0, 74.0));
        assert_eq!(pulled(base, grab, at(220.0, 70.0)), (134.0, 74.0));
        assert_eq!(pulled(base, grab, at(150.0, 60.0)), (64.0, 64.0));
    }

    #[test]
    fn a_press_lands_on_a_handle_a_body_or_nothing() {
        let round = Slot {
            x: 0.0,
            y: 0.0,
            width: 74.0,
            height: 74.0,
        };
        let tile = Slot {
            x: 90.0,
            y: 0.0,
            width: 164.0,
            height: 74.0,
        };
        let slots = [(ControlId::Lock, round), (ControlId::Wifi, tile)];
        assert_eq!(
            hit(&slots, at(10.0, 10.0), 22.0),
            Some(Hit::Body(ControlId::Lock, round))
        );
        assert_eq!(
            hit(&slots, at(60.0, 60.0), 22.0),
            Some(Hit::Handle(ControlId::Lock, round))
        );
        assert_eq!(
            hit(&slots, at(100.0, 60.0), 22.0),
            Some(Hit::Body(ControlId::Wifi, tile))
        );
        assert_eq!(
            hit(&slots, at(250.0, 70.0), 22.0),
            Some(Hit::Handle(ControlId::Wifi, tile))
        );
        // the gap between cells holds nothing
        assert_eq!(hit(&slots, at(80.0, 10.0), 22.0), None);
        assert_eq!(hit(&slots, at(10.0, 100.0), 22.0), None);
    }
}
