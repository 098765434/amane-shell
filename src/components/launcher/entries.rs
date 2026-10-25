//! What a search finds, in the order it's listed: a sum's answer, the
//! matching apps, the session actions named, the URL typed and, always
//! last, a web search, so the list is never empty. Each runs on Enter or a
//! click and closes the launcher.

use amane::{DesktopApp, Service};

use crate::components::icons::{
    ICON_CALCULATE, ICON_LOCK, ICON_LOG_OUT, ICON_OPEN_IN_NEW, ICON_POWER, ICON_RESTART,
    ICON_SEARCH, ICON_SLEEP,
};
use crate::components::taskbar::close;
use crate::services::session::Session;
use crate::services::{run, run_now};

use super::model::{self, SessionAction};

/// How much of an app's description its row shows.
const DESCRIPTION_CHARS: usize = 52;

#[derive(Clone)]
pub(super) enum Icon {
    App(DesktopApp),
    Glyph(char),
}

#[derive(Clone)]
pub(super) enum Action {
    Launch(DesktopApp),
    SessionAction(SessionAction),
    /// Opens in the default browser, which then gets the focus.
    Open(String),
    Copy(String),
}

#[derive(Clone)]
pub(super) struct Entry {
    pub(super) title: String,
    pub(super) subtitle: Option<String>,
    pub(super) icon: Icon,
    pub(super) action: Action,
}

/// The entries for `query` over the installed `apps`.
pub(super) fn build(query: &str, apps: &[DesktopApp]) -> Vec<Entry> {
    let query = query.trim();
    let needle = query.to_lowercase();
    let mut entries = Vec::new();

    if let Some(answer) = model::evaluate(query) {
        let value = model::format_number(answer);
        entries.push(Entry {
            title: format!("= {value}"),
            subtitle: Some(String::from("Copy to clipboard")),
            icon: Icon::Glyph(ICON_CALCULATE),
            action: Action::Copy(value),
        });
    }

    for app in apps
        .iter()
        .filter(|app| model::matches(app.name(), app.keywords(), &needle))
    {
        entries.push(Entry {
            title: app.name().to_string(),
            subtitle: app
                .description()
                .map(|text| truncate(text, DESCRIPTION_CHARS)),
            icon: Icon::App(app.clone()),
            action: Action::Launch(app.clone()),
        });
    }

    let session = Session::read();
    for power in SessionAction::ALL {
        let allowed = match power {
            SessionAction::Suspend => session.can_suspend(),
            SessionAction::Restart => session.can_reboot(),
            SessionAction::PowerOff => session.can_power_off(),
            SessionAction::Lock | SessionAction::LogOut => true,
        };
        if allowed && power.matches(&needle) {
            entries.push(Entry {
                title: power.label().to_string(),
                subtitle: Some(String::from("System")),
                icon: Icon::Glyph(glyph(power)),
                action: Action::SessionAction(power),
            });
        }
    }
    drop(session);

    if model::looks_like_url(query) {
        entries.push(Entry {
            title: format!("Open {query}"),
            subtitle: Some(String::from("Default browser")),
            icon: Icon::Glyph(ICON_OPEN_IN_NEW),
            action: Action::Open(model::normalize_url(query)),
        });
    }

    entries.push(Entry {
        title: format!("Search the web for \u{201c}{query}\u{201d}"),
        subtitle: Some(String::from("Google")),
        icon: Icon::Glyph(ICON_SEARCH),
        action: Action::Open(format!(
            "{}{}",
            model::SEARCH_URL,
            model::percent_encode(query)
        )),
    });

    entries
}

fn glyph(power: SessionAction) -> char {
    match power {
        SessionAction::Lock => ICON_LOCK,
        SessionAction::LogOut => ICON_LOG_OUT,
        SessionAction::Suspend => ICON_SLEEP,
        SessionAction::Restart => ICON_RESTART,
        SessionAction::PowerOff => ICON_POWER,
    }
}

/// Does what `action` says, and closes the launcher.
pub(super) fn run_action(action: &Action) {
    close();
    match action {
        Action::Launch(app) => app.launch(),
        Action::SessionAction(SessionAction::Lock) => Session::lock(),
        Action::SessionAction(SessionAction::LogOut) => Session::log_out(),
        Action::SessionAction(SessionAction::Suspend) => Session::suspend(),
        Action::SessionAction(SessionAction::Restart) => Session::reboot(),
        Action::SessionAction(SessionAction::PowerOff) => Session::power_off(),
        Action::Open(url) => open(url.clone()),
        Action::Copy(value) => copy(value),
    }
}

/// Puts `text` on the clipboard (`wl-copy`).
pub(super) fn copy(text: &str) {
    if !text.is_empty() {
        run("wl-copy", vec![String::from("--"), text.to_string()]);
    }
}

/// The clipboard's text (`wl-paste`). Blocks for as long as it takes.
pub(super) fn paste() -> Option<String> {
    run_now("wl-paste", &[String::from("--no-newline")])
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
fn truncate(text: &str, max: usize) -> String {
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
