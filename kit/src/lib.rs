//! The shell's look, for the shell and for apps that run beside it: the
//! design tokens from Penpot (`design-tokens/tokens.json`), the theme they
//! resolve against, and the controls built from them.
//!
//! - [`buttons`] — push, icon and segmented buttons, and a switch.
//! - [`card`] — M3's card: a filled or outlined surface, optionally frosted.
//! - [`glass`] — the lit edge and glow every translucent surface wears.
//! - [`app_bar`] — M3's small top app bar: a panel's or sheet's title row.
//! - [`list`] — M3's list: one-line items with leading and trailing slots.
//! - [`divider`] — M3's divider: a hairline between groups.
//! - [`slider`] — M3's slider: a 0–100 level along a track.
//! - [`tooltip`] — M3's plain tooltip: a hovered control's label.
//! - [`scroll_view`] — a clipped viewport scrolled along one axis, with a
//!   scrollbar.
//! - [`flow`] — fixed-size items flowing into rows, wrapping at a width.
//! - [`app_icon`] — an app's icon from its desktop entry, by app_id.
//!
//! Under those, with no look of their own:
//!
//! - [`layout`] — layout primitives (`boxed`, `pad`, `icon`, `split_row` …).
//! - [`icons`] — the Material Symbols glyphs and their pickers.
//! - [`color`] — colour arithmetic (`over`, `faded`, `with_alpha`).
//! - [`motion`] — the shared springs and fades, and keyed list motion.
//! - [`kinetic`] — following fingers, and the momentum a flick leaves.
//! - [`pager`] — turning pages with the fingers, a flick or a wheel.
//! - [`glass_region`] — where the compositor frosts behind clear glass.
//! - [`edge_reveal`] — a surface hiding at a screen edge until the pointer
//!   touches it.
//! - [`tokens`] and [`theme`] — the design tokens and the mode, density,
//!   accent and text scale they resolve with.

pub mod app_bar;
pub mod app_icon;
pub mod buttons;
pub mod card;
pub mod color;
pub mod divider;
pub mod edge_reveal;
pub mod flow;
pub mod glass;
pub mod glass_region;
pub mod icons;
pub mod kinetic;
pub mod layout;
pub mod list;
pub mod motion;
pub mod pager;
pub mod scroll_view;
pub mod slider;
pub mod theme;
pub mod tokens;
pub mod tooltip;
