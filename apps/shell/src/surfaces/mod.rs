//! The shell's surfaces: the status bar, the dock, the launcher,
//! notifications, Quick Settings, the desktop widgets, the context menu, the
//! alert and the on-screen display. Each is documented in `docs/shell/`.
//!
//! What several of them share and only the shell has lives in [`crate::chrome`]; the rest is
//! the kit's.

pub mod alert;
pub mod dock;
pub mod fill;
pub mod launcher;
pub mod notifications;
pub mod osd;
pub mod quick_settings;
pub mod status_bar;
pub mod widgets;
