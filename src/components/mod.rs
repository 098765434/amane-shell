//! Reusable shell components, each self-contained and driven by the Penpot
//! tokens in `design-tokens/tokens.json` (`sys.*` plus its own prefix):
//!
//! - [`buttons`] — reusable controls: push, icon and segmented buttons, and
//!   a switch.
//! - [`context_menu`] — a macOS-style, arbitrarily-nested context menu built
//!   on the fork's xdg-popup support.
//! - [`notifications`] — iOS/macOS-style toasts, a notification center in
//!   a sidebar, and Do Not Disturb, over amane's notification daemon.
//! - [`quick_settings`] — GNOME's quick settings in the macOS Control
//!   Center look: a grid of tiles and sliders, each with a page.
//! - [`status_bar`] — a compact bottom status bar: workspaces and the
//!   status cluster.
//! - [`dock`] — the macOS Big Sur dock for the apps open on each
//!   workspace, coming up from the bottom edge and with niri's overview.
//! - [`launcher`] — a Launchpad-style app launcher: a glass panel with a
//!   search, category pills and a grid of the installed apps.
//!
//! The building blocks they share read `sys.*` only, so any component can
//! use them. Most live in the kit (`amane-kit`), so apps can use them too,
//! and are re-exported here under their old paths. Like the buttons, each is named and structured after its
//! Material 3 counterpart and drawn in the shell's macOS look:
//!
//! - [`card`] — M3's card: a filled or outlined surface, optionally frosted.
//! - [`glass`] — the lit edge and glow every translucent surface wears.
//! - [`app_bar`] — M3's small top app bar: a panel's or sheet's title row.
//! - [`list`] — M3's list: one-line items with leading and trailing slots.
//! - [`divider`] — M3's divider: a hairline between groups.
//! - [`slider`] — M3's slider: a 0–100 level along a track.
//! - [`tooltip`] — M3's plain tooltip: a hovered control's label.
//! - [`scroll_view`] — a clipped viewport scrolled along one axis, with a
//!   scrollbar.
//! - [`search_field`] — a search capsule: a magnifier, the query with its
//!   caret and selection, and a clear button.
//! - [`flow`] — fixed-size items flowing into rows, wrapping at a width.
//! - [`workspace_menu`] — a submenu of workspaces to send something to.
//! - [`app_icon`] — an app's icon from its desktop entry, by app_id.
//! - [`panel`] — the one-open-at-a-time state of Quick Settings, the
//!   notification center and the launcher, and their overlay surfaces.
//! - [`battery`] — when a battery reads low, and the colour that says so.
//!
//! Under those, with no look of their own:
//!
//! - [`layout`] — layout primitives (`boxed`, `pad`, `icon`, `split_row` …).
//! - [`icons`] — the Material Symbols glyphs and their pickers.
//! - [`color`] — colour arithmetic (`over`, `faded`, `with_alpha`).
//! - [`motion`] — the shared springs and fades, and keyed list motion.
//! - [`kinetic`] — following fingers, and the momentum a flick leaves.
//! - [`text_edit`] — a one-line field's text, caret and selection, and the
//!   keys that edit it.
//! - [`pager`] — turning pages with the fingers, a flick or a wheel.
//! - [`glass_region`] — where the compositor frosts behind clear glass.
//! - [`edge_reveal`] — a surface hiding at a screen edge until the pointer
//!   touches it.

pub mod battery;
pub mod context_menu;
pub mod dock;
pub mod launcher;
pub mod notifications;
pub mod panel;
pub mod quick_settings;
pub mod search_field;
pub mod status_bar;
pub mod text_edit;
pub mod workspace_menu;

pub use amane_kit::{
    app_bar, app_icon, buttons, card, color, divider, edge_reveal, flow, glass, glass_region,
    icons, kinetic, layout, list, material, motion, pager, scroll_view, slider, tooltip,
};
