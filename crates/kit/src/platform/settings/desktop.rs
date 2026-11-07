//! `desktop.json`: the desktop's widgets and how they look.

use super::*;

/// How the desktop's widgets look while windows are open over the desktop.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum WidgetStyle {
    /// Faded back while a window is open on the monitor's workspace.
    #[default]
    Automatic,
    /// Always as they are with nothing open.
    Full,
    /// Always faded back.
    Dim,
}

/// The desktop: its widgets.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Desktop {
    /// Whether the desktop shows its widgets.
    pub show_widgets: bool,
    /// How the widgets look.
    pub widget_style: WidgetStyle,
}

impl Default for Desktop {
    fn default() -> Self {
        Self {
            show_widgets: true,
            widget_style: WidgetStyle::Automatic,
        }
    }
}

impl Settings for Desktop {
    const APP: &'static str = "amane-shell";
    const FILE: &'static str = "desktop";
}

/// The desktop settings as they are now; a view that reads them redraws
/// when they change.
pub fn desktop() -> RwLockReadGuard<'static, Config<Desktop>> {
    Config::<Desktop>::read()
}

/// Changes the desktop settings and saves them, for input handlers.
pub fn change_desktop(edit: impl FnOnce(&mut Desktop)) {
    Config::<Desktop>::change(edit);
}
