//! What the two grid editors share, Quick Settings' edit mode and the
//! desktop's Edit Widgets: how a press on an item becomes a drag, whether a
//! drag let go lands, and the outlines that show where it will. Each keeps
//! its own drag in flight and its own look; the grid itself (spans,
//! layouts, previews) is the kit's `grid`, which reads no tokens.

use amane::{Point, Rectangle, Size, Widget};

use crate::tokens::sys;
use emaki_kit::grid::GridKey;
use emaki_kit::grid::drag::{Drag, Hit, Origin, Session, off_canvas};
use emaki_kit::grid::geometry::{Metrics, Slot};
use emaki_kit::layout::{boxed, frame};

/// The drag a press on `hit` begins: an item's body moves it, held where
/// the pointer took it (`from`); its handle resizes it, measured from
/// there. `pointer` is where the pointer is now, both in the grid's pixels.
pub fn begin<K: GridKey>(hit: Hit<K>, from: Point, pointer: Point) -> Drag<K> {
    match hit {
        Hit::Body(id, slot) => Drag::Move {
            id,
            grab: Point {
                x: from.x - slot.x,
                y: from.y - slot.y,
            },
            at: (slot.x, slot.y),
            place: None,
            origin: Origin::Canvas,
            pointer,
        },
        Hit::Handle(id, slot) => Drag::Resize {
            id,
            grab: from,
            base: slot,
            pulled: (slot.width, slot.height),
            span: None,
        },
    }
}

/// How a drag let go ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Landing<K> {
    /// The item moved lands where the preview has it, from where it was
    /// let go (its top-left in the grid's pixels).
    Moved {
        id: K,
        at: (f32, f32),
        origin: Origin,
    },
    /// The item resized keeps the size the preview has.
    Resized,
    /// Let go off the canvas (by more than half a cell beyond its edge), or
    /// where there's no room: nothing lands, and the item goes back where it
    /// came from (the catalog), or, from the canvas, is removed.
    Off { id: K, origin: Origin },
}

/// How `session`, let go, ends on a grid of `px`.
pub fn landing<K: GridKey>(session: &Session<K>, px: &Metrics) -> Landing<K> {
    match session.drag {
        Drag::Move {
            id,
            at,
            origin,
            pointer,
            place,
            ..
        } => {
            if place.is_none() || off_canvas(px, &session.base.canvas, pointer) {
                Landing::Off { id, origin }
            } else {
                Landing::Moved { id, at, origin }
            }
        }
        Drag::Resize { .. } => Landing::Resized,
    }
}

/// An outline in the accent colour, `width` by `height` with `radius`
/// corners, its top-left at (`x`, `y`): where an item in hand will land,
/// or how far a pull has come.
pub fn outline(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Box<dyn Widget> {
    boxed(outlined(width, height, radius).translate(x, y))
}

fn outlined(width: f32, height: f32, radius: f32) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(height)
        .radius(radius)
        .border(sys::border::hairline(), sys::color::accent::bg())
}

/// Nothing, as a layer: the outline's place while nothing is held, so the
/// stack keeps its shape from one frame to the next.
pub fn no_outline() -> Box<dyn Widget> {
    boxed(
        Rectangle::new()
            .width(Size::Fixed(0.0))
            .height(Size::Fixed(0.0)),
    )
}

/// The pull a resize in flight has made of `id`: how wide and tall the
/// hand has it, before it snaps to a size the item comes in.
pub fn pulling<K: GridKey>(drag: Option<Drag<K>>, id: K) -> Option<(f32, f32)> {
    match drag {
        Some(Drag::Resize {
            id: held, pulled, ..
        }) if held == id => Some(pulled),
        _ => None,
    }
}

/// The pull's outline over an item at `slot` (in its own frame, from its
/// top-left), never smaller than one cell, so the hand sees how far it
/// has come.
pub fn pull_outline(slot: Slot, pulled: (f32, f32), px: &Metrics, radius: f32) -> Box<dyn Widget> {
    let (w, h) = (pulled.0.max(px.cell), pulled.1.max(px.cell));
    boxed(
        frame(slot.width, slot.height)
            .align_child(amane::Start, amane::Start)
            .child(outlined(w, h, radius)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use emaki_kit::grid::Layout;
    use emaki_kit::grid::geometry::{Canvas, Span};

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct Item;

    impl GridKey for Item {
        fn supported(&self) -> &'static [Span] {
            const ONE: [Span; 1] = [Span::new(1, 1)];
            &ONE
        }
    }

    const PX: Metrics = Metrics::new(74.0, 16.0);

    fn session(drag: Drag<Item>) -> Session<Item> {
        Session::new(Layout::new(Canvas { cols: 4, rows: 4 }), drag)
    }

    #[test]
    fn a_press_on_the_body_moves_and_on_the_handle_resizes() {
        let slot = Slot {
            x: 90.0,
            y: 0.0,
            width: 74.0,
            height: 74.0,
        };
        let from = Point { x: 100.0, y: 10.0 };
        let now = Point { x: 120.0, y: 10.0 };
        match begin(Hit::Body(Item, slot), from, now) {
            Drag::Move {
                grab, at, pointer, ..
            } => {
                assert_eq!((grab.x, grab.y), (10.0, 10.0));
                assert_eq!(at, (90.0, 0.0));
                assert_eq!((pointer.x, pointer.y), (120.0, 10.0));
            }
            Drag::Resize { .. } => panic!("the body moves"),
        }
        assert!(matches!(
            begin(Hit::Handle(Item, slot), from, now),
            Drag::Resize {
                pulled: (74.0, 74.0),
                ..
            }
        ));
    }

    #[test]
    fn a_drag_let_go_off_the_canvas_or_with_no_place_lands_nowhere() {
        let moving = |pointer: Point, place| {
            session(Drag::Move {
                id: Item,
                grab: Point { x: 0.0, y: 0.0 },
                at: (pointer.x, pointer.y),
                place,
                origin: Origin::Canvas,
                pointer,
            })
        };
        let on = Point { x: 10.0, y: 10.0 };
        let cell = Some(emaki_kit::grid::geometry::Place::new(0, 0));
        assert!(matches!(
            landing(&moving(on, cell), &PX),
            Landing::Moved { .. }
        ));
        // no room under it
        assert!(matches!(
            landing(&moving(on, None), &PX),
            Landing::Off { .. }
        ));
        // more than half a cell past the edge, though the preview had a place
        let past = Point { x: -40.0, y: 10.0 };
        assert!(matches!(
            landing(&moving(past, cell), &PX),
            Landing::Off { .. }
        ));
    }
}
