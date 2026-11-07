//! `launcher.json`: the launcher's apps layout, quick keys and clipboard history.

use super::*;

/// How the launcher shows the apps in its Apps mode.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum AppsLayout {
    #[default]
    /// Icons in a grid.
    Grid,
    /// Rows, one an app.
    List,
}

/// An action's quick keys: what they run, by the launcher's key for it,
/// and its name then, for a settings page that can't ask the launcher.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", default)]
pub struct QuickKey {
    /// The action, by its id.
    pub action: String,
    /// What the action is called, as it was when the keys were set.
    pub title: String,
}

/// The launcher's clipboard history: off until asked for, and what it
/// keeps when on.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct ClipboardHistory {
    /// Whether the launcher keeps what's copied.
    pub enabled: bool,

    /// How many copies it keeps, newest first.
    pub max_items: usize,

    /// Whether copied images are kept too.
    pub images: bool,

    /// Apps whose copies aren't kept, by their app ids, like password
    /// managers.
    pub exclude_apps: Vec<String>,
}

impl Default for ClipboardHistory {
    fn default() -> Self {
        Self {
            enabled: false,
            max_items: 200,
            images: true,
            exclude_apps: [
                "org.keepassxc.KeePassXC",
                "keepassxc",
                "bitwarden",
                "Bitwarden",
                "1password",
                "1Password",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

/// The launcher: how it lays the apps out, what its search leaves out,
/// the quick keys that run its actions, and its clipboard history.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Launcher {
    /// How the launcher's apps are laid out.
    pub apps_layout: AppsLayout,

    /// The sources the search leaves out, by their ids, like "web".
    pub excluded_providers: Vec<String>,

    /// Keys typed in the launcher, like "nl", and the action each runs.
    pub quick_keys: BTreeMap<String, QuickKey>,

    /// The clipboard history's settings.
    pub clipboard: ClipboardHistory,
}

impl Launcher {
    /// The quick keys that run `action`, if any do.
    pub fn keys_for(&self, action: &str) -> Option<&str> {
        self.quick_keys
            .iter()
            .find(|(_, key)| key.action == action)
            .map(|(keys, _)| keys.as_str())
    }
}

impl Settings for Launcher {
    const APP: &'static str = crate::app_ids::SHELL;
    const FILE: &'static str = "launcher";
}

/// The launcher settings as they are now; a view that reads them redraws
/// when they change.
pub fn launcher() -> RwLockReadGuard<'static, Config<Launcher>> {
    Config::<Launcher>::read()
}

/// Changes the launcher settings and saves them, for input handlers.
pub fn change_launcher(edit: impl FnOnce(&mut Launcher)) {
    Config::<Launcher>::change(edit);
}
