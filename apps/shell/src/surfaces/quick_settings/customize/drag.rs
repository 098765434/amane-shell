//! Dragging a control from the pane onto the canvas: where the pointer is, the ghost under it,
//! and where it lands.

use super::*;
use amane_kit::layout::frame;

/// The surface says where the canvas is this frame.
pub(crate) fn set_canvas_origin(x: f32, y: f32) {
    CANVAS_ORIGIN.set((x, y));
}

pub(super) fn on_canvas(p: Point) -> Point {
    let (x, y) = CANVAS_ORIGIN.get();
    Point {
        x: p.x - x,
        y: p.y - y,
    }
}

/// The pane moved: the scrollbar shows for a while.
pub(super) fn moved() {
    SCROLLBAR.with_borrow_mut(ScrollbarFade::moved);
}

pub(super) fn on_pane_scroll(scroll: Scroll) {
    SCROLL.with_borrow_mut(|pane| pane.scroll(&scroll, Axis::Vertical));
    moved();
    super::super::QuickSettings::changed();
}

pub(super) fn on_side_scroll(scroll: Scroll) {
    SIDE_SCROLL.with_borrow_mut(|side| side.scroll(&scroll, Axis::Vertical));
    super::super::QuickSettings::changed();
}

/// The pointer is at `point` on the surface with the button down: carries
/// on the drag in flight; else takes hold of the control under it in the
/// pane; else hands the press to the canvas, which picks up what's there.
pub(super) fn drag_at(point: Point) {
    if edit::drag().is_some() {
        edit::drag_at(on_canvas(point));
        return;
    }
    let under = with(|state| {
        state
            .tiles
            .iter()
            .find(|(_, area)| area.contains(point.x, point.y))
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
pub(crate) fn drag_layer() -> Box<dyn Widget> {
    let layer = Rectangle::new()
        .width(Parent)
        .height(Parent)
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
pub(crate) fn ghost(origin: (f32, f32)) -> Option<(Box<dyn Widget>, BlurShape)> {
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
    let widget = frame(slot.width, slot.height)
        .translate(x, y)
        .scale(scale)
        .child(Stack::new(vec![control]));
    Some((boxed(widget), frost))
}

/// Where the window sits on a `screen` (the surface's size) beside the
/// `card`: in the middle, moved left to clear the card by a gap when the
/// screen has the room, and no taller than the screen allows.
pub(crate) fn place(screen: (f32, f32), card: Area) -> Area {
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
