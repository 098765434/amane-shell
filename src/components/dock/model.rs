//! What the dock shows: a page per workspace on its monitor, each the apps
//! open on it, one icon an app however many windows it has.
//!
//! Plain data from niri's windows and workspaces, so it can be tested
//! without a compositor. An app is its windows' app_id (the title, for a
//! window without one); apps keep the order they opened in (their oldest
//! window's id), so icons don't shuffle as focus moves.

use amane::{WindowInfo, Workspace};

/// A window, as much of it as the dock needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Win {
    pub id: u64,
    pub app_id: String,
    pub title: String,
    pub workspace: Option<i64>,
    pub focused: bool,
    pub floating: bool,
}

impl From<&WindowInfo> for Win {
    fn from(window: &WindowInfo) -> Self {
        Self {
            id: window.id(),
            app_id: window.app_id().unwrap_or("").to_string(),
            title: window.title().unwrap_or("").to_string(),
            workspace: window.workspace(),
            focused: window.focused(),
            floating: window.floating(),
        }
    }
}

/// One icon: an app and its windows on one workspace, oldest first.
#[derive(Clone, Debug, PartialEq)]
pub struct App {
    pub key: String,
    pub app_id: String,
    pub title: String,
    pub windows: Vec<Win>,
}

impl App {
    /// Whether one of its windows has the focus.
    pub fn focused(&self) -> bool {
        self.windows.iter().any(|window| window.focused)
    }
}

/// One workspace's apps.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub workspace: i64,
    /// The workspace showing on the monitor.
    pub active: bool,
    pub apps: Vec<App>,
}

/// A workspace, as much of it as the dock needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Space {
    pub id: i64,
    pub index: u32,
    pub output: String,
    pub active: bool,
}

impl From<&Workspace> for Space {
    fn from(workspace: &Workspace) -> Self {
        Self {
            id: workspace.id(),
            index: workspace.index(),
            output: workspace.output().unwrap_or("").to_string(),
            active: workspace.active(),
        }
    }
}

/// The pages for monitor `output`, in workspace order: every workspace
/// with a window open, and the active one even when empty (niri keeps an
/// empty one at the end, which needs no page of its own).
pub fn pages(spaces: &[Space], windows: &[Win], output: &str) -> Vec<Page> {
    let mut mine: Vec<&Space> = spaces
        .iter()
        .filter(|space| space.output == output)
        .collect();
    mine.sort_by_key(|space| space.index);

    mine.into_iter()
        .map(|space| Page {
            workspace: space.id,
            active: space.active,
            apps: apps(windows, space.id),
        })
        .filter(|page| page.active || !page.apps.is_empty())
        .collect()
}

/// The apps open on `workspace`, in the order they opened.
pub fn apps(windows: &[Win], workspace: i64) -> Vec<App> {
    let mut mine: Vec<&Win> = windows
        .iter()
        .filter(|window| window.workspace == Some(workspace))
        .collect();
    mine.sort_by_key(|window| window.id);

    let mut apps: Vec<App> = Vec::new();
    for window in mine {
        let key = key(window);
        match apps.iter_mut().find(|app| app.key == key) {
            Some(app) => app.windows.push(window.clone()),
            None => apps.push(App {
                key,
                app_id: window.app_id.clone(),
                title: window.title.clone(),
                windows: vec![window.clone()],
            }),
        }
    }
    apps
}

/// Which app a window counts as: its app_id, or its title without one.
fn key(window: &Win) -> String {
    if window.app_id.is_empty() {
        format!("title:{}", window.title)
    } else {
        window.app_id.clone()
    }
}

/// Which of `app`'s windows a click focuses: the next one after the
/// focused one, when it has the focus (clicking again goes round its
/// windows), else the one focused most recently (`recent`, latest first).
pub fn target(app: &App, recent: &[u64]) -> Option<u64> {
    let windows = &app.windows;
    if let Some(at) = windows.iter().position(|window| window.focused) {
        return windows
            .get((at + 1) % windows.len())
            .map(|window| window.id);
    }
    recent
        .iter()
        .find(|id| windows.iter().any(|window| window.id == **id))
        .copied()
        .or_else(|| windows.first().map(|window| window.id))
}

/// `recent` with `focused` moved to the front: the windows by when they
/// last had the focus, kept to those still open.
pub fn remember(recent: &mut Vec<u64>, focused: Option<u64>, open: &[Win]) {
    if let Some(id) = focused
        && recent.first() != Some(&id)
    {
        recent.retain(|other| *other != id);
        recent.insert(0, id);
    }
    recent.retain(|id| open.iter().any(|window| window.id == *id));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(id: u64, app_id: &str, workspace: i64) -> Win {
        Win {
            id,
            app_id: app_id.to_string(),
            title: format!("{app_id} {id}"),
            workspace: Some(workspace),
            focused: false,
            floating: false,
        }
    }

    fn space(id: i64, index: u32, output: &str, active: bool) -> Space {
        Space {
            id,
            index,
            output: output.to_string(),
            active,
        }
    }

    #[test]
    fn an_apps_windows_share_one_icon_in_the_order_apps_opened() {
        let windows = [
            win(5, "firefox", 1),
            win(2, "foot", 1),
            win(3, "firefox", 1),
            win(4, "foot", 2),
        ];
        let apps = apps(&windows, 1);
        let keys: Vec<&str> = apps.iter().map(|app| app.key.as_str()).collect();
        assert_eq!(keys, ["foot", "firefox"]);
        let ids: Vec<u64> = apps[1].windows.iter().map(|window| window.id).collect();
        assert_eq!(ids, [3, 5]);
    }

    #[test]
    fn a_window_without_an_app_id_goes_by_its_title() {
        let mut untitled = win(1, "", 1);
        untitled.title = String::from("scratch");
        let apps = apps(&[untitled], 1);
        assert_eq!(apps[0].key, "title:scratch");
    }

    #[test]
    fn pages_are_this_monitors_workspaces_with_windows_or_showing() {
        let spaces = [
            space(30, 3, "eDP-1", false),
            space(10, 1, "eDP-1", false),
            space(20, 2, "eDP-1", true),
            space(40, 1, "HDMI-A-1", true),
        ];
        let windows = [win(1, "foot", 10), win(2, "foot", 40)];
        let pages = pages(&spaces, &windows, "eDP-1");
        let ids: Vec<i64> = pages.iter().map(|page| page.workspace).collect();
        // 20 is empty but showing; 30 is empty and hidden
        assert_eq!(ids, [10, 20]);
        assert!(pages[1].active && pages[1].apps.is_empty());
    }

    #[test]
    fn a_click_goes_round_the_windows_of_the_focused_app() {
        let mut windows = vec![win(1, "foot", 1), win(2, "foot", 1), win(3, "foot", 1)];
        windows[1].focused = true;
        let app = &apps(&windows, 1)[0];
        assert_eq!(target(app, &[]), Some(3));

        windows[1].focused = false;
        windows[2].focused = true;
        let app = &apps(&windows, 1)[0];
        assert_eq!(target(app, &[]), Some(1), "after the last comes the first");
    }

    #[test]
    fn a_click_on_another_app_focuses_its_latest_window() {
        let windows = [win(1, "foot", 1), win(2, "foot", 1), win(9, "firefox", 1)];
        let foot = &apps(&windows, 1)[0];
        assert_eq!(target(foot, &[9, 2, 1]), Some(2));
        assert_eq!(target(foot, &[]), Some(1), "never focused: the oldest");
    }

    #[test]
    fn the_latest_focus_comes_first_and_closed_windows_drop_out() {
        let open = [win(1, "foot", 1), win(2, "foot", 1)];
        let mut recent = vec![1, 7];
        remember(&mut recent, Some(2), &open);
        assert_eq!(recent, [2, 1]);
        remember(&mut recent, Some(2), &open);
        assert_eq!(recent, [2, 1]);
    }
}
