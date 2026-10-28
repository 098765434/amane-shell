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
//! - [`arrangement`] — screens dragged into place, as Displays arranges them.
//!
//! The look of glass (`look/`):
//!
//! - [`material`] — Apple's materials: glass by role, its thickness and
//!   the vibrant labels on it.
//! - [`glass`] — the lit edge and glow every translucent surface wears.
//! - [`glass_region`] — where the compositor frosts behind clear glass.
//! - [`lab`] — Shader Lab's saved looks, and putting one on a rectangle.
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
    app_bar, arrangement, card, dialog, divider, flow, list, scroll_view, tooltip,
};
pub use controls::{buttons, color_picker, control_size, dropdown, slider, tag};
pub use foundation::{color, icons, layout};
pub use look::{glass, glass_region, lab, material};
pub use platform::{app, app_icon, marker, settings};
