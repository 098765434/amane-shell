//! The shell's look, for the shell and for apps that run beside it: the
//! design tokens from Penpot (`design-tokens/tokens.json`), the theme they
//! resolve against, and the controls built from them.
//!
//! - [`app`] — an app's window, text fields, dialogs and sidebar.
//! - [`buttons`] — push, icon and segmented buttons, a switch, a checkbox
//!   and radio buttons.
//! - [`card`] — M3's card: a filled or outlined surface, optionally frosted.
//! - [`material`] — Apple's materials: glass by role, its thickness and
//!   the vibrant labels on it.
//! - [`glass`] — the lit edge and glow every translucent surface wears.
//! - [`app_bar`] — M3's small top app bar: a panel's or sheet's title row.
//! - [`list`] — M3's list: one-line items with leading and trailing slots.
//! - [`divider`] — M3's divider: a hairline between groups.
//! - [`slider`] — M3's slider: a 0–100 level along a track.
//! - [`dropdown`] — one choice out of a list, behind a button that shows it.
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
//! - [`settings`] — the shell's settings file, which a settings app changes.
//! - [`tokens`] and [`theme`] — the design tokens and the mode, density,
//!   accent and text scale they resolve with.

pub mod app;
pub mod app_bar;
pub mod app_icon;
pub mod buttons;
pub mod card;
pub mod color;
pub mod control_size;
pub mod divider;
pub mod dropdown;
pub mod edge_reveal;
pub mod flow;
pub mod glass;
pub mod glass_region;
pub mod icons;
pub mod kinetic;
pub mod layout;
pub mod list;
pub mod material;
pub mod motion;
pub mod pager;
pub mod scroll_view;
pub mod settings;
pub mod slider;
pub mod theme;
pub mod tokens;
pub mod tooltip;
