//! The shell's surfaces: the status bar, the dock, the launcher,
//! notifications, Quick Settings, the desktop widgets, the context menu and
//! the alert. Each is documented in `docs/shell/`.
//!
//! What several of them share lives in [`crate::components`].

pub mod alert;
pub mod context_menu;
pub mod dock;
pub mod launcher;
pub mod notifications;
pub mod quick_settings;
pub mod status_bar;
pub mod widgets;
