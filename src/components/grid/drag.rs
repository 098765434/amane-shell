//! Dragging on the grid: what the pointer holds, what a press lands on,
//! and the preview a drag makes of the layout it began from.
//!
//! Every preview is worked out from the layout as it was when the drag
//! began (the base), never from the last preview, so the same cell always
//! gives the same arrangement and moving back restores the base. A cell
//! where nothing fits keeps the last preview that did, as iOS stays where
//! it last fit. The cell an item is in is kept until it's well past the
//! boundary (`Metrics::settle`), so a hand resting on a boundary doesn't
//! make the grid flap.
//!
//! The pointer is measured in the grid's pixels, on a layer that doesn't
//! move (see the editors' docs for why), so where an item is drawn is the
//! pointer's position less where it was grabbed.

use amane::Point;

use super::geometry::{Canvas, Metrics, Place, Slot, Span};
use super::layout::{GridKey, Layout};

/// Where an item on the move was taken from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Its cells on the canvas.
    Canvas,
    /// An editor's catalog: it has no cells until the preview gives it
    /// some, and goes back to the catalog if let go elsewhere.
    Window,
}

/// What the pointer holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drag<K> {
    /// A control on the move: where on it the pointer took hold (from its
    /// top-left), where the pointer has its top-left now, in the grid's
    /// pixels, the cell the preview has it in, where it came from, and
    /// where the pointer last was (the grid's pixels too).
    Move {
        id: K,
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
        id: K,
        grab: Point,
        base: Slot,
        pulled: (f32, f32),
        span: Option<Span>,
    },
}

/// What a press on the canvas lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit<K> {
    Body(K, Slot),
    Handle(K, Slot),
}

/// A drag in flight: the layout it began from, the one it previews (none
/// until a place fits), and what the pointer holds.
#[derive(Clone, Debug)]
pub struct Session<K> {
    pub base: Layout<K>,
    pub preview: Option<Layout<K>>,
    pub drag: Drag<K>,
}

impl<K: GridKey> Session<K> {
    pub fn new(base: Layout<K>, drag: Drag<K>) -> Self {
        Self {
            base,
            preview: None,
            drag,
        }
    }

    /// Previews `f` of the base: a layout it gives replaces the preview
    /// and is `true`; one it can't keeps the last.
    pub fn preview_with(&mut self, f: impl FnOnce(&Layout<K>) -> Option<Layout<K>>) -> bool {
        match f(&self.base) {
            Some(layout) => {
                self.preview = Some(layout);
                true
            }
            None => false,
        }
    }

    /// The pointer is at `point` with the button down: the held item
    /// follows it, settling into the nearest cells (`band` the dead band,
    /// in pitches), or the pulled item into the nearest size.
    pub fn motion(&mut self, point: Point, px: &Metrics, band: f32) {
        match self.drag {
            Drag::Move { .. } => self.motion_move(point, px, band),
            Drag::Resize { .. } => self.motion_resize(point, px),
        }
    }

    fn motion_move(&mut self, point: Point, px: &Metrics, band: f32) {
        let Drag::Move {
            id,
            grab,
            place,
            origin,
            ..
        } = self.drag
        else {
            return;
        };
        let at = follow(grab, point);
        let canvas = self.base.canvas;
        let mut place = place;
        if let Some(span) = self.base.span_of(id) {
            if off_canvas(px, &canvas, point) {
                // off the canvas it has no cells: the others go back to
                // where they were, and one from the canvas leaves a hole
                if place.is_some() {
                    place = None;
                    let mut lifted = self.base.clone();
                    lifted.hide(id);
                    self.preview = Some(lifted);
                }
            } else {
                let nearest = px.settle(&canvas, at.0, at.1, span, place, band);
                if place != Some(nearest)
                    && self.preview_with(|base| base.placed_at(id, span, nearest))
                {
                    place = Some(nearest);
                }
            }
        }
        self.drag = Drag::Move {
            id,
            grab,
            at,
            place,
            origin,
            pointer: point,
        };
    }

    fn motion_resize(&mut self, point: Point, px: &Metrics) {
        let Drag::Resize {
            id,
            grab,
            base,
            span,
            ..
        } = self.drag
        else {
            return;
        };
        let pulled = pulled(base, grab, point);
        let nearest = nearest_span_from(span, id.supported(), pulled.0, pulled.1, px, px.gap / 2.0);
        let mut span = span;
        if span != Some(nearest) && self.preview_with(|base| base.resized(id, nearest)) {
            span = Some(nearest);
        }
        self.drag = Drag::Resize {
            id,
            grab,
            base,
            pulled,
            span,
        };
    }
}

/// Whether `point` (in the grid's pixels) is clearly off the canvas: more
/// than half a cell beyond any edge.
pub fn off_canvas(px: &Metrics, canvas: &Canvas, point: Point) -> bool {
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
pub fn hit<K: GridKey>(slots: &[(K, Slot)], point: Point, handle: f32) -> Option<Hit<K>> {
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
pub fn follow(grab: Point, point: Point) -> (f32, f32) {
    (point.x - grab.x, point.y - grab.y)
}

/// How big a pull from `grab` to `point` makes a control that was `base`.
pub fn pulled(base: Slot, grab: Point, point: Point) -> (f32, f32) {
    (
        base.width + point.x - grab.x,
        base.height + point.y - grab.y,
    )
}

/// The size among `supported` nearest `width` by `height`, in `px`; unless
/// `current` is set and no other size is nearer by more than `band`
/// pixels: the size in hand holds until another is clearly closer.
pub fn nearest_span_from(
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
    let Some(&nearest) = supported
        .iter()
        .min_by(|a, b| distance(**a).total_cmp(&distance(**b)))
    else {
        return current.unwrap_or(Span::new(1, 1));
    };
    match current {
        Some(current)
            if supported.contains(&current) && distance(current) <= distance(nearest) + band =>
        {
            current
        }
        _ => nearest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PX: Metrics = Metrics::new(74.0, 16.0);

    const S: Span = Span::new(1, 1);
    const M: Span = Span::new(2, 1);
    const L: Span = Span::new(2, 2);
    const W3: Span = Span::new(3, 1);
    const FULL: Span = Span::new(4, 1);

    /// A round button that grows into a tile, as Quick Settings' Lock does.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct Key(&'static [Span]);

    impl GridKey for Key {
        fn supported(&self) -> &'static [Span] {
            self.0
        }
    }

    const LOCK: Key = Key(&[S, M]);
    const WIFI: Key = Key(&[M, W3, FULL]);

    fn at(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    fn nearest_span(supported: &[Span], width: f32, height: f32, px: &Metrics) -> Span {
        nearest_span_from(None, supported, width, height, px, 0.0)
    }

    #[test]
    fn a_pull_snaps_to_the_nearest_size_it_comes_in() {
        let round = [S, M];
        assert_eq!(nearest_span(&round, 74.0 + 80.0, 74.0, &PX), M);
        assert_eq!(nearest_span(&round, 74.0 + 20.0, 74.0, &PX), S);
        assert_eq!(nearest_span(&round, 400.0, 400.0, &PX), M);
        let square = [L, FULL];
        // pulled down more than across, a square stays square
        assert_eq!(nearest_span(&square, 200.0, 150.0, &PX), L);
        assert_eq!(nearest_span(&square, 330.0, 80.0, &PX), FULL);
        assert_eq!(nearest_span(&[FULL], 10.0, 10.0, &PX), FULL);
        let tile = [M, W3, FULL];
        assert_eq!(nearest_span(&tile, 164.0 + 60.0, 74.0, &PX), W3);
    }

    #[test]
    fn a_size_in_hand_holds_until_another_is_clearly_nearer() {
        let round = [S, M];
        // halfway between S (74) and M (164) is 119: a hair past it, without
        // a band M wins, with one S holds
        assert_eq!(nearest_span_from(Some(S), &round, 121.0, 74.0, &PX, 0.0), M);
        assert_eq!(nearest_span_from(Some(S), &round, 121.0, 74.0, &PX, 8.0), S);
        assert_eq!(nearest_span_from(Some(S), &round, 130.0, 74.0, &PX, 8.0), M);
        // and back the same way
        assert_eq!(nearest_span_from(Some(M), &round, 117.0, 74.0, &PX, 8.0), M);
        assert_eq!(nearest_span_from(Some(M), &round, 108.0, 74.0, &PX, 8.0), S);
        // a size it doesn't come in can't hold
        assert_eq!(
            nearest_span_from(Some(FULL), &round, 74.0, 74.0, &PX, 8.0),
            S
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
        let slots = [(LOCK, round), (WIFI, tile)];
        assert_eq!(
            hit(&slots, at(10.0, 10.0), 22.0),
            Some(Hit::Body(LOCK, round))
        );
        assert_eq!(
            hit(&slots, at(60.0, 60.0), 22.0),
            Some(Hit::Handle(LOCK, round))
        );
        assert_eq!(
            hit(&slots, at(100.0, 60.0), 22.0),
            Some(Hit::Body(WIFI, tile))
        );
        assert_eq!(
            hit(&slots, at(250.0, 70.0), 22.0),
            Some(Hit::Handle(WIFI, tile))
        );
        // the gap between cells holds nothing
        assert_eq!(hit(&slots, at(80.0, 10.0), 22.0), None);
        assert_eq!(hit(&slots, at(10.0, 100.0), 22.0), None);
    }
}
