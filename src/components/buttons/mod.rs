//! Reusable buttons, drawn from `sys.*` tokens only so any component can use
//! them:
//!
//! - [`SegmentedButton`] — a row of 2–5 related options on one track, single-
//!   or multi-select (after Material 3's segmented buttons, in the shell's
//!   macOS-style look).

// The public API (icons, multi-select, radius, disabled) is broader than the
// bar uses yet, so dead_code is allowed module-wide; unused_imports is not.
#![allow(dead_code)]

mod segmented;

pub use segmented::{Segment, SegmentedButton};
