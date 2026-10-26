//! The grid Quick Settings' controls and the desktop's widgets are laid
//! out on: square cells with a gap between them, items spanning whole
//! cells, a layout of where each sits, and the drags that move and resize
//! them. Pure: no tokens and no drawing, so each owner brings its own
//! sizes and its own look.
//!
//! - [`geometry`] — spans, places, the canvas, its pixels, and which cells
//!   are taken.
//! - [`layout`] — where each item sits, and the layout a move, a placement
//!   or a resize would leave (swap or push).
//! - [`drag`] — what the pointer holds and the preview a drag makes.

pub mod drag;
pub mod geometry;
pub mod layout;

pub use layout::{Entry, GridKey, Layout};
