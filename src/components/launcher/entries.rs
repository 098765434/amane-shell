//! What a search finds and what each find does. An [`Entry`] is one row:
//! its icon, its title, a line under it (and the fuller one Super shows),
//! and its [`Action`] on Enter or a click. Running one counts it in the
//! launcher's [`usage`](super::usage), so what's used rises.

use std::path::PathBuf;
use std::rc::Rc;

use amane::DesktopApp;

use crate::components::panel::close;
use crate::components::text_edit;
use crate::services::clipboard_history::{self, Clip};
use crate::services::run_now;

use super::usage;

#[derive(Clone)]
pub(super) enum Icon {
    App(Box<DesktopApp>),
    Glyph(char),
    /// An image file, shown small: a copied image.
    Image(PathBuf),
}

/// What the side of Clipboard shows of the row picked.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Preview {
    Text(String),
    Image(PathBuf),
}

/// Somewhere a search can be narrowed to: Tab on a row that has one, or a
/// click on its chip.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Scope {
    /// An app: its own actions.
    App(DesktopApp),
    /// An action that asks for something first, like how long a timer
    /// runs: the field takes it.
    Action {
        key: String,
        title: String,
        placeholder: &'static str,
    },
}

impl Scope {
    /// What its chip and the token in the search field say.
    pub(super) fn name(&self) -> &str {
        match self {
            Self::App(app) => app.name(),
            Self::Action { title, .. } => title,
        }
    }

    /// What the empty field says while narrowed to it, if not the mode's
    /// name.
    pub(super) fn placeholder(&self) -> Option<&'static str> {
        match self {
            Self::App(_) => None,
            Self::Action { placeholder, .. } => Some(placeholder),
        }
    }
}

#[derive(Clone)]
pub(super) enum Action {
    Launch(DesktopApp),
    /// One of an app's `[Desktop Action]`s, by its place in the list.
    AppShortcut(DesktopApp, usize),
    /// Opens in the default browser, which then gets the focus.
    Open(String),
    Copy(String),
    /// One of the launcher's actions, with what it asked for.
    Run(Rc<dyn Fn()>),
    /// An action that asks for something first: narrows the search to it.
    Ask(Scope),
    /// A row that only says something, like what to type.
    Hint,
    /// A copy from the clipboard's history, pasted into the window the
    /// launcher was over.
    Paste(Box<Clip>),
}

#[derive(Clone)]
pub(super) struct Entry {
    /// What its use is remembered by, like "app:firefox.desktop".
    pub(super) key: String,
    pub(super) title: String,
    pub(super) subtitle: Option<String>,
    /// The fuller line Super shows in the subtitle's place, like a path.
    pub(super) detail: Option<String>,
    /// A short note at the row's end, like a date.
    pub(super) trailing: Option<String>,
    pub(super) icon: Icon,
    /// A small glyph on the icon's corner, saying where it's from.
    pub(super) badge: Option<char>,
    pub(super) action: Action,
    /// Where Tab narrows the search to from this row.
    pub(super) scope: Option<Scope>,
    /// The heading of the section it starts or belongs to, in a list of
    /// sections (Actions).
    pub(super) section: Option<String>,
    /// For one of the launcher's actions: its name, which quick keys are
    /// saved with, and the quick keys that run it.
    pub(super) action_title: Option<String>,
    pub(super) quick_keys: Option<String>,
    /// What Clipboard shows of it beside the list.
    pub(super) preview: Option<Preview>,
}

impl Entry {
    pub(super) fn new(
        key: impl Into<String>,
        title: impl Into<String>,
        icon: Icon,
        action: Action,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            subtitle: None,
            detail: None,
            trailing: None,
            icon,
            badge: None,
            action,
            scope: None,
            section: None,
            action_title: None,
            quick_keys: None,
            preview: None,
        }
    }

    pub(super) fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub(super) fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub(super) fn scope(mut self, scope: Scope) -> Self {
        self.scope = Some(scope);
        self
    }

    /// An app's row: its icon and description, its command line under
    /// Super, and Tab into its actions when it has any.
    pub(super) fn app(app: &DesktopApp) -> Self {
        let mut entry = Self::new(
            app_key(app),
            app.name(),
            Icon::App(Box::new(app.clone())),
            Action::Launch(app.clone()),
        )
        .detail(format!("{} · {}", app.id(), app.exec()));
        if let Some(description) = app.description() {
            entry = entry.subtitle(truncate(description, DESCRIPTION_CHARS));
        }
        if !app.actions().is_empty() {
            entry = entry.scope(Scope::App(app.clone()));
        }
        entry
    }
}

/// How much of an app's description its row shows.
const DESCRIPTION_CHARS: usize = 64;

/// What an app's use is remembered by.
pub(super) fn app_key(app: &DesktopApp) -> String {
    format!("app:{}", app.id())
}

/// Does what `entry` says, counting it picked for `query`, and closes the
/// launcher; an action that asks for something narrows the search to it
/// instead, and a hint does nothing.
pub(super) fn run(entry: &Entry, query: &str) {
    match &entry.action {
        Action::Hint => return,
        Action::Ask(scope) => {
            super::enter_scope(scope.clone());
            return;
        }
        Action::Paste(clip) => {
            close();
            clipboard_history::put_back(clip, true);
            return;
        }
        _ => {}
    }
    usage::record(&entry.key, query);
    close();
    match &entry.action {
        Action::Launch(app) => app.launch(),
        Action::AppShortcut(app, index) => {
            if let Some(action) = app.actions().get(*index) {
                action.launch();
            }
        }
        Action::Open(url) => open(url.clone()),
        Action::Copy(value) => text_edit::copy(value),
        Action::Run(run) => run(),
        Action::Ask(_) | Action::Hint | Action::Paste(_) => {}
    }
}

/// Opens `url` in the default browser and focuses it, as launching an app
/// does: niri focuses a browser's new window itself, but one already open
/// only gets a tab, so its window is looked up and focused.
fn open(url: String) {
    std::thread::spawn(move || {
        if run_now("xdg-open", &[url]).is_none() {
            return;
        }
        let browser = run_now(
            "xdg-settings",
            &[String::from("get"), String::from("default-web-browser")],
        );
        let windows = run_now(
            "niri",
            &[
                String::from("msg"),
                String::from("--json"),
                String::from("windows"),
            ],
        );
        let (Some(browser), Some(windows)) = (browser, windows) else {
            return;
        };
        let app_id = browser.trim().trim_end_matches(".desktop");
        if let Some(id) = window_of(&windows, app_id) {
            amane::Windows::focus(id);
        }
    });
}

/// The id of the window in niri's `windows` JSON whose app_id is `app_id`,
/// else one whose app_id holds it or is held in it (a desktop file and its
/// windows are often named a little differently).
fn window_of(windows: &str, app_id: &str) -> Option<u64> {
    let want = app_id.to_lowercase();
    if want.is_empty() {
        return None;
    }
    let list: Vec<serde_json::Value> = serde_json::from_str(windows).ok()?;
    let ids = list.iter().filter_map(|window| {
        let id = window.get("id")?.as_u64()?;
        let app = window.get("app_id")?.as_str()?.to_lowercase();
        Some((id, app))
    });
    let mut near = None;
    for (id, app) in ids {
        if app == want {
            return Some(id);
        }
        if near.is_none() && !app.is_empty() && (app.contains(&want) || want.contains(&app)) {
            near = Some(id);
        }
    }
    near
}

/// `text` cut to `max` characters, with an ellipsis when it was longer.
pub(super) fn truncate(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(max.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOWS: &str = r#"[
        {"id": 3, "app_id": "org.gnome.Nautilus", "title": "Home"},
        {"id": 7, "app_id": "firefox-esr", "title": "Mozilla Firefox"},
        {"id": 9, "app_id": "firefox", "title": "Mozilla Firefox"},
        {"id": 11, "app_id": null, "title": "untitled"}
    ]"#;

    #[test]
    fn the_browsers_window_is_found_by_its_app_id() {
        assert_eq!(window_of(WINDOWS, "firefox"), Some(9));
        assert_eq!(window_of(WINDOWS, "Firefox"), Some(9));
    }

    #[test]
    fn a_differently_named_window_is_found_too() {
        assert_eq!(window_of(WINDOWS, "nautilus"), Some(3));
        assert_eq!(window_of(WINDOWS, "chromium"), None);
        assert_eq!(window_of(WINDOWS, ""), None);
        assert_eq!(window_of("not json", "firefox"), None);
    }

    #[test]
    fn long_descriptions_are_cut() {
        assert_eq!(truncate("  short ", 10), "short");
        assert_eq!(truncate("abcdefghij", 5), "abcd…");
    }
}
