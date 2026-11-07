//! The shell's look, for the shell and for apps that run beside it: the
//! design tokens from Penpot (`design-tokens/tokens.json`), the theme they
//! resolve against, and the controls built from them.
//!
//! The source is grouped into folders by kind, and every module is
//! re-exported here, so its path is `amane_kit::<module>` whatever folder
//! it lives in.
//!
//! Controls (`controls/`):
//!
//! - [`buttons`] — push, icon and segmented buttons, a switch, a checkbox
//!   and radio buttons.
//! - [`slider`] — M3's slider: a 0–100 level along a track.
//! - [`dropdown`] — one choice out of a list, behind a button that shows it.
//! - [`color_picker`] — a colour, from a square, a hue strip or hex.
//! - [`control_size`] — the control sizes, and the one in use.
//! - [`tag`] — a small fact about something, like M3's assist chip.
//! - [`menu`] — a pop-up menu of items, under a control or the pointer.
//! - [`leading_icon`] — the glyph on a coloured tile that leads a row.
//! - [`stepper`] — a number nudged up and down.
//! - [`thumbnail_picker`] — one choice out of pictures.
//! - [`nav_buttons`] — the back and forward pair of a window's toolbar.
//! - [`pop_up_button`] — a small round button that opens a menu.
//!
//! The `controls` example shows them all.
//!
//! Containers (`containers/`):
//!
//! - [`card`] — M3's card: a filled or outlined surface, optionally frosted.
//! - [`app_bar`] — M3's small top app bar: a panel's or sheet's title row.
//! - [`list`] — M3's list: one-line items with leading and trailing slots.
//! - [`divider`] — M3's divider: a hairline between groups.
//! - [`dialog`] — M3's basic dialog: the shell's alert, an app's modal.
//! - [`tooltip`] — M3's plain tooltip: a hovered control's label.
//! - [`scroll_view`] — a clipped viewport scrolled along one axis, with a
//!   scrollbar.
//! - [`flow`] — fixed-size items flowing into rows, wrapping at a width.
//! - [`form`] — settings laid out in rounded groups of rows,
//!   each a coloured tile, a title and its control.
//! - [`arrangement`] — screens dragged into place, as Displays arranges them.
//!
//! The look of glass (`look/`):
//!
//! - [`material`] — the materials: glass by role, its thickness and
//!   the vibrant labels on it.
//! - [`glass`] — the lit edge and glow every translucent surface wears.
//! - [`glass_region`] — where the compositor frosts behind clear glass.
//! - [`lab`] — Shader Lab's saved looks, and putting one on a rectangle.
//! - [`style`] — the colours, sizes and fonts the form controls share.
//!
//! Motion (`animation/`):
//!
//! - [`motion`] — the shared springs and fades, and keyed list motion.
//! - [`kinetic`] — following fingers, and the momentum a flick leaves.
//! - [`pager`] — turning pages with the fingers, a flick or a wheel.
//! - [`edge_reveal`] — a surface hiding at a screen edge until the pointer
//!   touches it.
//!
//! Primitives (`foundation/`), with no look of their own:
//!
//! - [`layout`] — layout primitives (`boxed`, `pad`, `icon`, `split_row` …).
//! - [`icons`] — the Material Symbols glyphs and their pickers.
//! - [`color`] — colour arithmetic (`over`, `faded`, `with_alpha`).
//! - [`symbol`] — the few drawn symbols the controls use (chevrons, checks).
//!
//! The system around an app (`platform/`):
//!
//! - [`app`] — an app's window, text fields, dialogs and sidebar.
//! - [`app_icon`] — an app's icon from its desktop entry, by app_id.
//! - [`settings`] — the shell's settings file, which a settings app changes.
//! - [`marker`] — scoped redraws for state kept outside a service.
//!
//! And at the root, [`tokens`] and [`theme`]: the design tokens and the
//! mode, density, accent and text scale they resolve with.

#![warn(missing_docs)]

mod animation;
mod containers;
mod controls;
mod foundation;
mod look;
mod platform;

pub mod theme;
pub mod tokens;

pub use animation::{edge_reveal, kinetic, motion, pager};
pub use containers::{
    app_bar, arrangement, card, dialog, divider, flow, form, list, scroll_view, tooltip,
};
pub use controls::{
    avatar, buttons, color_picker, control_size, dropdown, leading_icon, menu, nav_buttons,
    pop_up_button, slider, spinner, stepper, tag, thumbnail_picker,
};
pub use foundation::{color, icons, layout, symbol};
pub use look::{glass, glass_region, lab, material, style};
pub use platform::{app, app_icon, marker, secrets, settings};
