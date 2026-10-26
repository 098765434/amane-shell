//! A layout — where each item sits on the canvas and how big it is, or
//! that it's hidden (waiting in an editor's catalog). What the items are
//! is the owner's: anything with the sizes it comes in ([`GridKey`]).
//!
//! Moves and resizes are worked out on a copy (`moved`, `placed_at`,
//! `resized`): the items in the way swap with the moving one when it covers
//! just one its own size, else they're pushed to the first free cells; when
//! one can't fit, nothing moves. An editor previews those copies as it
//! drags and commits one on the drop.

use std::fmt::Debug;
use std::hash::Hash;

use super::geometry::{Canvas, Occupancy, Place, Rect, Span};

/// An item a layout can hold: a key, and the sizes it comes in.
pub trait GridKey: Copy + Eq + Hash + Debug {
    /// The sizes it comes in, smallest first; never empty.
    fn supported(&self) -> &'static [Span];

    /// `span`, or the nearest size it comes in.
    fn clamp(&self, span: Span) -> Span {
        let supported = self.supported();
        if supported.contains(&span) {
            return span;
        }
        *supported
            .iter()
            .min_by_key(|s| s.cells().abs_diff(span.cells()))
            .unwrap_or(&supported[0])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry<K> {
    pub id: K,
    pub span: Span,
    /// Its cell, or `None` when it's hidden.
    pub at: Option<Place>,
}

impl<K> Entry<K> {
    pub fn rect(&self) -> Option<Rect> {
        self.at.map(|at| Rect::new(at, self.span))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout<K> {
    pub canvas: Canvas,
    pub entries: Vec<Entry<K>>,
}

impl<K: GridKey> Layout<K> {
    pub fn new(canvas: Canvas) -> Self {
        Self {
            canvas,
            entries: Vec::new(),
        }
    }

    /// The items on the canvas, in layout order.
    pub fn placed(&self) -> impl Iterator<Item = (K, Rect)> + '_ {
        self.entries
            .iter()
            .filter_map(|entry| entry.rect().map(|rect| (entry.id, rect)))
    }

    /// The items hidden.
    pub fn hidden(&self) -> impl Iterator<Item = K> + '_ {
        self.entries
            .iter()
            .filter(|entry| entry.at.is_none())
            .map(|entry| entry.id)
    }

    pub fn rect_of(&self, id: K) -> Option<Rect> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .and_then(Entry::rect)
    }

    /// The size `id` has, on the canvas or hidden.
    pub fn span_of(&self, id: K) -> Option<Span> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.span)
    }

    pub fn occupancy(&self) -> Occupancy<K> {
        Occupancy::of(self.canvas, self.placed())
    }

    /// How many rows the canvas needs to show everything placed.
    #[cfg(test)]
    pub fn rows_used(&self) -> u8 {
        self.occupancy().rows_used()
    }

    fn entry(&mut self, id: K) -> Option<&mut Entry<K>> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }

    /// Adds `entry` as it is, unless the layout has `id` already.
    pub fn insert(&mut self, entry: Entry<K>) {
        if !self.entries.iter().any(|e| e.id == entry.id) {
            self.entries.push(entry);
        }
    }

    /// Takes `id` out of the layout altogether.
    pub fn remove(&mut self, id: K) {
        self.entries.retain(|entry| entry.id != id);
    }

    pub fn hide(&mut self, id: K) {
        if let Some(entry) = self.entry(id) {
            entry.at = None;
        }
    }

    /// Puts `id` in the first free cells it fits, at its size or failing
    /// that its smallest; whether there was room.
    pub fn show(&mut self, id: K) -> bool {
        let occupancy = self.occupancy();
        let Some(entry) = self.entry(id) else {
            return false;
        };
        if entry.at.is_some() {
            return true;
        }
        let smallest = id.supported().first().copied().unwrap_or(entry.span);
        for span in [entry.span, smallest] {
            if let Some(at) = occupancy.first_free(span) {
                entry.span = span;
                entry.at = Some(at);
                return true;
            }
        }
        false
    }

    /// Makes the entries a layout: anything off the canvas or over another
    /// item moves to the first free cells, or hides when there are none.
    pub fn repair(&mut self) {
        let mut occupancy: Occupancy<K> = Occupancy::new(self.canvas);
        for i in 0..self.entries.len() {
            let entry = self.entries[i];
            let Some(rect) = entry.rect() else {
                continue;
            };
            let at = if occupancy.free(&rect, None) {
                Some(rect.at)
            } else {
                occupancy.first_free(entry.span)
            };
            self.entries[i].at = at;
            if let Some(at) = at {
                occupancy.put(entry.id, Rect::new(at, entry.span));
            }
        }
    }

    /// This layout on `canvas`: what still fits stays, the rest goes to the
    /// first free cells, and what can't fit at all hides. For a screen that
    /// shrank; the layout it came from is what's kept.
    pub fn refit(&self, canvas: Canvas) -> Self {
        let mut out = Self {
            canvas,
            entries: self.entries.clone(),
        };
        out.repair();
        out
    }

    /// This layout with `id` moved so its top-left is `at`, the items in
    /// its way swapped or pushed aside; `None` when that can't be done.
    pub fn moved(&self, id: K, at: Place) -> Option<Self> {
        let rect = self.rect_of(id)?;
        self.settled(id, Rect::new(at, rect.span), Some(rect))
    }

    /// This layout with `id` at `at`, `span` big (or the nearest size it
    /// comes in), the items in its way swapped or pushed aside as
    /// [`moved`](Self::moved) does: a hidden item put on the canvas by
    /// hand, or one on it moved. `None` when that can't be done.
    pub fn placed_at(&self, id: K, span: Span, at: Place) -> Option<Self> {
        self.span_of(id)?;
        let span = id.clamp(span);
        self.settled(id, Rect::new(at, span), self.rect_of(id))
    }

    /// This layout with `id` at `span` (or the nearest size it comes in),
    /// shifted up or left to stay on the canvas; `None` when it can't be.
    pub fn resized(&self, id: K, span: Span) -> Option<Self> {
        let rect = self.rect_of(id)?;
        let span = id.clamp(span);
        let at = self
            .canvas
            .clamp(i32::from(rect.at.col), i32::from(rect.at.row), span);
        self.settled(id, Rect::new(at, span), Some(rect))
    }

    /// This layout with `id` at `rect`, making room for it; `before` is
    /// where it was, or `None` for a hidden one.
    fn settled(&self, id: K, rect: Rect, before: Option<Rect>) -> Option<Self> {
        if !self.canvas.fits(&rect) {
            return None;
        }
        let mut next = self.clone();
        next.entry(id)?.span = rect.span;
        next.entry(id)?.at = Some(rect.at);
        if before == Some(rect) {
            return Some(next);
        }

        let others = Occupancy::of(self.canvas, self.placed().filter(|(other, _)| *other != id));
        let covered = others.covered(&rect, None);

        // one item its own size under it changes places with it
        if let [other] = covered[..]
            && let Some(before) = before
            && let Some(theirs) = self.rect_of(other)
            && theirs.span == before.span
            && rect.span == before.span
        {
            let swapped = Rect::new(before.at, theirs.span);
            let mut rest = Occupancy::of(
                self.canvas,
                self.placed().filter(|(o, _)| *o != id && *o != other),
            );
            rest.put(id, rect);
            if rest.free(&swapped, None) {
                next.entry(other)?.at = Some(swapped.at);
                return Some(next);
            }
        }

        // else what's under it goes to the first free cells, in order
        let mut taken = Occupancy::of(
            self.canvas,
            self.placed()
                .filter(|(o, _)| !covered.contains(o) && *o != id),
        );
        taken.put(id, rect);
        for other in covered {
            let span = self.rect_of(other)?.span;
            let at = taken.first_free(span)?;
            taken.put(other, Rect::new(at, span));
            next.entry(other)?.at = Some(at);
        }
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: Span = Span::new(2, 2);
    const WIDE: Span = Span::new(4, 2);

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct Key(u32);

    impl GridKey for Key {
        fn supported(&self) -> &'static [Span] {
            &[SQUARE, WIDE]
        }
    }

    fn entry(id: u32, span: Span, col: u8, row: u8) -> Entry<Key> {
        Entry {
            id: Key(id),
            span,
            at: Some(Place::new(col, row)),
        }
    }

    #[test]
    fn insert_and_remove_keep_one_entry_per_key() {
        let mut layout = Layout::new(Canvas { cols: 8, rows: 4 });
        layout.insert(entry(1, SQUARE, 0, 0));
        layout.insert(entry(1, WIDE, 4, 0));
        assert_eq!(layout.entries.len(), 1);
        assert_eq!(layout.span_of(Key(1)), Some(SQUARE));
        layout.remove(Key(1));
        assert!(layout.entries.is_empty());
    }

    #[test]
    fn refitting_to_a_smaller_canvas_moves_or_hides_and_keeps_the_original() {
        let layout = Layout {
            canvas: Canvas { cols: 8, rows: 4 },
            entries: vec![
                entry(1, SQUARE, 0, 0),
                entry(2, SQUARE, 6, 0),
                entry(3, WIDE, 4, 2),
            ],
        };
        let small = layout.refit(Canvas { cols: 4, rows: 2 });
        // the first stays, the second moves into the room left, the wide
        // one has none
        assert_eq!(small.rect_of(Key(1)).map(|r| r.at), Some(Place::new(0, 0)));
        assert_eq!(small.rect_of(Key(2)).map(|r| r.at), Some(Place::new(2, 0)));
        assert_eq!(small.rect_of(Key(3)), None);
        assert_eq!(layout.rect_of(Key(3)).map(|r| r.at), Some(Place::new(4, 2)));
        // the same canvas changes nothing
        assert_eq!(layout.refit(layout.canvas), layout);
    }

    #[test]
    fn a_size_it_lacks_snaps_to_the_nearest() {
        assert_eq!(Key(0).clamp(Span::new(1, 1)), SQUARE);
        assert_eq!(Key(0).clamp(Span::new(4, 4)), WIDE);
        assert_eq!(Key(0).clamp(WIDE), WIDE);
    }
}
