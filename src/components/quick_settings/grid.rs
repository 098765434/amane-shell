//! The grid — iOS Control Center's canvas: `cols` columns by `rows` rows of
//! square cells with a gap between them, and controls that span whole
//! cells (a round button one, a tile two across, Now Playing two by two, a
//! slider three or four across). Every control has a place on the canvas,
//! holes and all; the layout (`layout.rs`) keeps them, and edit mode moves
//! them.
//!
//! The geometry is the shared grid's (`components::grid`); what's Quick
//! Settings' own is the sizes its controls come in and the canvas and cell
//! its tokens give.

pub use crate::components::grid::geometry::*;

use crate::tokens::quick;

impl Span {
    /// A round button.
    pub const S: Span = Span::new(1, 1);
    /// A tile: its disc and two lines beside it.
    pub const M: Span = Span::new(2, 1);
    /// A square module: Now Playing.
    pub const L: Span = Span::new(2, 2);
    /// A slider beside a round button.
    pub const W3: Span = Span::new(3, 1);
    /// A whole row.
    pub const FULL: Span = Span::new(4, 1);
}

impl Canvas {
    pub fn tokens() -> Self {
        Self {
            cols: quick::grid::cols().round().max(1.0) as u8,
            rows: quick::grid::rows().round().max(1.0) as u8,
        }
    }
}

impl Metrics {
    pub fn tokens() -> Self {
        Self::new(quick::grid::cell(), quick::grid::gap())
    }
}
