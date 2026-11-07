//! The pieces only the shell uses, shared by several of its surfaces
//! ([`crate::surfaces`]): `panel`, `battery`, `grid`, `search_field` and
//! `text_edit`.
//!
//! The building blocks they share live in the kit (`amane-kit`), so apps can
//! use them too, and are re-exported here under their old paths; see
//! `docs/kit/components.md` and `docs/kit/controls.md`.

pub mod battery;
pub mod grid;
pub mod panel;
pub mod search_field;
pub mod text_edit;

pub use amane_kit::{
    app_bar, app_icon, buttons, card, color, control_size, dialog, divider, edge_reveal, flow,
    glass, glass_region, icons, kinetic, layout, list, material, motion, pager, scroll_view,
    slider, tooltip,
};
