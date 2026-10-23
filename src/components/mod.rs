//! Reusable shell components, each self-contained and driven by the Penpot
//! tokens in `design-tokens/tokens.json` (`sys.*` plus its own prefix):
//!
//! - [`buttons`] — reusable controls, like the [`buttons::SegmentedButton`].
//! - [`context_menu`] — a macOS-style, arbitrarily-nested context menu built
//!   on the fork's xdg-popup support.
//! - [`taskbar`] — a bottom taskbar that is compact in normal use and
//!   expands into an interactive dock while niri's overview is open.

pub mod buttons;
pub mod context_menu;
pub mod taskbar;
