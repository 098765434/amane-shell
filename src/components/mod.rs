//! Reusable shell components, each self-contained and driven by the Penpot
//! tokens in `design-tokens/tokens.json` (`sys.*` plus its own prefix):
//!
//! - [`buttons`] — reusable controls: push, icon and segmented buttons, and
//!   a switch.
//! - [`context_menu`] — a macOS-style, arbitrarily-nested context menu built
//!   on the fork's xdg-popup support.
//! - [`notifications`] — iOS/macOS-style toasts, a notification center in
//!   a sidebar, and Do Not Disturb, over amane's notification daemon.
//! - [`taskbar`] — a bottom taskbar that is compact in normal use and
//!   expands into an interactive dock while niri's overview is open.
//!
//! The building blocks they share read `sys.*` only, so any component can
//! use them. Like the buttons, each is named and structured after its
//! Material 3 counterpart and drawn in the shell's macOS look:
//!
//! - [`card`] — M3's card: a filled or outlined surface, optionally frosted.
//! - [`app_bar`] — M3's small top app bar: a panel's or sheet's title row.
//! - [`list`] — M3's list: one-line items with leading and trailing slots.
//! - [`divider`] — M3's divider: a hairline between groups.
//! - [`slider`] — M3's slider: a 0–100 level along a track.
//!
//! Under those, with no look of their own:
//!
//! - [`layout`] — layout primitives (`boxed`, `pad`, `icon`, `split_row` …).
//! - [`icons`] — the Material Symbols glyphs and their pickers.
//! - [`color`] — colour arithmetic (`over`, `faded`, `with_alpha`).
//! - [`motion`] — the shared springs and fades, and keyed list motion.

pub mod app_bar;
pub mod buttons;
pub mod card;
pub mod color;
pub mod context_menu;
pub mod divider;
pub mod icons;
pub mod layout;
pub mod list;
pub mod motion;
pub mod notifications;
pub mod slider;
pub mod taskbar;
