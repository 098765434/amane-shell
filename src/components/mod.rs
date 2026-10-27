//! The shell's components: its surfaces (the status bar, the dock, the
//! launcher, notifications, Quick Settings, the desktop widgets, the context
//! menu and the alert) and the pieces only the shell uses (`panel`,
//! `battery`, `grid`, `search_field`, `text_edit`, `workspace_menu`).
//! Each is documented in `docs/shell/`.
//!
//! The building blocks they share live in the kit (`amane-kit`), so apps can
//! use them too, and are re-exported here under their old paths; see
//! `docs/kit/components.md` and `docs/kit/controls.md`.

pub mod alert;
pub mod battery;
pub mod bluetooth_prompt;
pub mod context_menu;
pub mod dock;
pub mod grid;
pub mod launcher;
pub mod notifications;
pub mod panel;
pub mod polkit_prompt;
pub mod quick_settings;
pub mod search_field;
pub mod status_bar;
pub mod text_edit;
pub mod widgets;
pub mod workspace_menu;

pub use amane_kit::{
    app_bar, app_icon, buttons, card, color, control_size, dialog, divider, edge_reveal, flow,
    glass, glass_region, icons, kinetic, layout, list, material, motion, pager, scroll_view,
    slider, tooltip,
};
