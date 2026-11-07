//! The grid — Quick Settings' canvas: `cols` columns by `rows` rows of
//! square cells with a gap between them, and controls that span whole
//! cells (a round button one, a tile two across, Now Playing two by two, a
//! slider three or four across). Every control has a place on the canvas,
//! holes and all; the layout (`layout.rs`) keeps them, and edit mode moves
//! them.
//!
//! The geometry is the shared grid's (`amane_kit::grid`); what's Quick
//! Settings' own is the sizes its controls come in and the canvas and cell
//! its tokens give.

pub use amane_kit::grid::geometry::*;

use crate::tokens::quick;

/// The sizes Quick Settings' controls come in, as `Span::S` and so on.
pub trait Sizes {
    /// A round button.
    const S: Span = Span::new(1, 1);
    /// A tile: its disc and two lines beside it.
    const M: Span = Span::new(2, 1);
    /// A square module: Now Playing.
    const L: Span = Span::new(2, 2);
    /// A slider beside a round button.
    const W3: Span = Span::new(3, 1);
    /// A whole row.
    const FULL: Span = Span::new(4, 1);
}

impl Sizes for Span {}

/// The canvas and its cells as Quick Settings' tokens give them.
pub trait FromTokens {
    /// From `quick.grid.*`.
    fn tokens() -> Self;
}

impl FromTokens for Canvas {
    fn tokens() -> Self {
        Self {
            cols: quick::grid::cols().round().max(1.0) as u8,
            rows: quick::grid::rows().round().max(1.0) as u8,
        }
    }
}

impl FromTokens for Metrics {
    fn tokens() -> Self {
        Self::new(quick::grid::cell(), quick::grid::gap())
    }
}
