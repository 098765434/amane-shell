//! What the dock shows: a section for each workspace on its monitor, side
//! by side, each an icon for every window open on it, in niri's column
//! order, then its pinned apps that aren't open there.
//!
//! Plain data from niri's windows and workspaces and the pins, so it can
//! be tested without a compositor.

use amane::{WindowInfo, Workspace};

use super::pins::{Pin, Place};

/// A window, as much of it as the dock needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Win {
    pub id: u64,
    pub app_id: String,
    pub title: String,
    pub workspace: Option<i64>,
    pub focused: bool,
    pub floating: bool,
    /// Its column and its place in the column, from 1, while tiled.
    pub column: Option<(usize, usize)>,
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
            column: window.column(),
        }
    }
}

/// One icon: a window, or an app pinned to the section's workspace that
/// isn't open there.
#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    Window(Win),
    Pin(Pin),
}

impl Icon {
    /// What it's known by from frame to frame, so it can slide as others
    /// come and go: a window keeps its across workspaces.
    pub fn key(&self, section: &str) -> String {
        match self {
            Self::Window(window) => format!("win:{}", window.id),
            Self::Pin(pin) => format!("pin:{section}:{}", pin.app_id),
        }
    }

    pub fn app_id(&self) -> &str {
        match self {
            Self::Window(window) => &window.app_id,
            Self::Pin(pin) => &pin.app_id,
        }
    }

    /// The window's title, for an icon without an app_id.
    pub fn title(&self) -> &str {
        match self {
            Self::Window(window) => &window.title,
            Self::Pin(_) => "",
        }
    }

    pub fn focused(&self) -> bool {
        matches!(self, Self::Window(window) if window.focused)
    }
}

/// One workspace's icons.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    /// None for a workspace only a pin knows of, not open yet.
    pub workspace: Option<i64>,
    /// Its place on the monitor, from 1.
    pub index: u32,
    pub name: Option<String>,
    /// The workspace showing on the monitor.
    pub active: bool,
    pub icons: Vec<Icon>,
}

impl Section {
    /// What it's known by from frame to frame.
    pub fn key(&self) -> String {
        match (&self.name, self.workspace) {
            (Some(name), _) => format!("name:{name}"),
            (None, Some(id)) => format!("ws:{id}"),
            (None, None) => format!("index:{}", self.index),
        }
    }
}

/// A workspace, as much of it as the dock needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Space {
    pub id: i64,
    pub index: u32,
    pub name: Option<String>,
    pub output: String,
    pub active: bool,
}

impl From<&Workspace> for Space {
    fn from(workspace: &Workspace) -> Self {
        Self {
            id: workspace.id(),
            index: workspace.index(),
            name: workspace.name().map(String::from),
            output: workspace.output().unwrap_or("").to_string(),
            active: workspace.active(),
        }
    }
}

/// Where `pin` is: the workspace by its name wherever it is, else the one
/// at its place on its monitor. None for a named one that's gone.
pub fn resolve<'a>(pin: &Pin, spaces: &'a [Space]) -> Option<&'a Space> {
    match &pin.place {
        Place::Named(name) => spaces
            .iter()
            .find(|space| space.name.as_deref() == Some(name.as_str())),
        Place::Index(index) => spaces
            .iter()
            .find(|space| space.output == pin.output && space.index == *index),
    }
}

/// The sections for monitor `output`, in workspace order: every workspace
/// with a window or a pin on it, and the active one even when empty (niri
/// keeps an empty one at the end, which needs no section of its own). A
/// pin by place on a workspace that isn't open yet has a section where the
/// workspace would be.
pub fn sections(spaces: &[Space], windows: &[Win], pins: &[Pin], output: &str) -> Vec<Section> {
    let mut mine: Vec<&Space> = spaces
        .iter()
        .filter(|space| space.output == output)
        .collect();
    mine.sort_by_key(|space| space.index);

    let mut sections: Vec<Section> = mine
        .into_iter()
        .map(|space| {
            let on: Vec<&Pin> = pins
                .iter()
                .filter(|pin| resolve(pin, spaces).is_some_and(|at| at.id == space.id))
                .collect();
            Section {
                workspace: Some(space.id),
                index: space.index,
                name: space.name.clone(),
                active: space.active,
                icons: icons(windows, space.id, &on),
            }
        })
        .filter(|section| section.active || !section.icons.is_empty())
        .collect();

    // pins on workspaces that aren't open
    for pin in pins {
        let Place::Index(index) = pin.place else {
            continue;
        };
        if pin.output != output || resolve(pin, spaces).is_some() {
            continue;
        }
        let at = match sections.iter().position(|section| section.index >= index) {
            Some(at) if sections[at].index == index => at,
            Some(at) => {
                sections.insert(at, unopened(index));
                at
            }
            None => {
                sections.push(unopened(index));
                sections.len() - 1
            }
        };
        if !sections[at].icons.contains(&Icon::Pin(pin.clone())) {
            sections[at].icons.push(Icon::Pin(pin.clone()));
        }
    }
    sections
}

/// `sections` with the empty workspace niri keeps after the last on
/// `output`, for a window to be dropped on, if it isn't there already.
pub fn with_last(mut sections: Vec<Section>, spaces: &[Space], output: &str) -> Vec<Section> {
    let last = spaces
        .iter()
        .filter(|space| space.output == output)
        .max_by_key(|space| space.index);
    if let Some(last) = last
        && !sections
            .iter()
            .any(|section| section.workspace == Some(last.id))
    {
        let at = sections
            .iter()
            .position(|section| section.index > last.index)
            .unwrap_or(sections.len());
        sections.insert(
            at,
            Section {
                workspace: Some(last.id),
                index: last.index,
                name: last.name.clone(),
                active: last.active,
                icons: Vec::new(),
            },
        );
    }
    sections
}

fn unopened(index: u32) -> Section {
    Section {
        workspace: None,
        index,
        name: None,
        active: false,
        icons: Vec::new(),
    }
}

/// The icons on `workspace`: its windows, tiled ones in column order and
/// then the floating ones, then the `pins` whose app has no window there.
pub fn icons(windows: &[Win], workspace: i64, pins: &[&Pin]) -> Vec<Icon> {
    let mut mine: Vec<&Win> = windows
        .iter()
        .filter(|window| window.workspace == Some(workspace))
        .collect();
    mine.sort_by_key(|window| (window.column.is_none(), window.column, window.id));

    let mut icons: Vec<Icon> = mine
        .iter()
        .map(|window| Icon::Window((*window).clone()))
        .collect();
    for pin in pins {
        if !mine.iter().any(|window| window.app_id == pin.app_id) {
            icons.push(Icon::Pin((*pin).clone()));
        }
    }
    icons
}

/// The window of `app_id` that had the focus last (`recent`, latest first),
/// or its oldest open one, wherever it is.
pub fn latest(app_id: &str, windows: &[Win], recent: &[u64]) -> Option<u64> {
    let of = |id: &u64| windows.iter().any(|w| w.id == *id && w.app_id == app_id);
    recent.iter().find(|id| of(id)).copied().or_else(|| {
        windows
            .iter()
            .filter(|window| window.app_id == app_id)
            .map(|window| window.id)
            .min()
    })
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

    fn win(id: u64, app_id: &str, workspace: i64, column: usize) -> Win {
        Win {
            id,
            app_id: app_id.to_string(),
            title: format!("{app_id} {id}"),
            workspace: Some(workspace),
            focused: false,
            floating: false,
            column: Some((column, 1)),
        }
    }

    fn space(id: i64, index: u32, output: &str, active: bool) -> Space {
        Space {
            id,
            index,
            name: None,
            output: output.to_string(),
            active,
        }
    }

    fn ids(icons: &[Icon]) -> Vec<String> {
        icons.iter().map(|icon| icon.key("s")).collect()
    }

    #[test]
    fn windows_go_in_column_order_and_floating_ones_last() {
        let mut floating = win(1, "mpv", 1, 0);
        floating.column = None;
        floating.floating = true;
        let windows = [
            win(5, "firefox", 1, 1),
            floating,
            win(2, "foot", 1, 3),
            win(3, "foot", 1, 2),
        ];
        assert_eq!(
            ids(&icons(&windows, 1, &[])),
            ["win:5", "win:3", "win:2", "win:1"]
        );
    }

    #[test]
    fn a_pin_shows_after_the_windows_unless_its_app_is_open_there() {
        let windows = [win(1, "foot", 1, 1), win(2, "firefox", 2, 1)];
        let firefox = Pin::new("firefox", "eDP-1", None, 1);
        let foot = Pin::new("foot", "eDP-1", None, 1);
        let icons = icons(&windows, 1, &[&firefox, &foot]);
        // open on another workspace doesn't count
        assert_eq!(ids(&icons), ["win:1", "pin:s:firefox"]);
    }

    #[test]
    fn sections_are_this_monitors_workspaces_with_icons_or_showing() {
        let spaces = [
            space(30, 3, "eDP-1", false),
            space(10, 1, "eDP-1", false),
            space(20, 2, "eDP-1", true),
            space(40, 1, "HDMI-A-1", true),
        ];
        let windows = [win(1, "foot", 10, 1), win(2, "foot", 40, 1)];
        let sections = sections(&spaces, &windows, &[], "eDP-1");
        let ws: Vec<Option<i64>> = sections.iter().map(|s| s.workspace).collect();
        // 20 is empty but showing; 30 is empty and hidden
        assert_eq!(ws, [Some(10), Some(20)]);
        assert!(sections[1].active && sections[1].icons.is_empty());
    }

    #[test]
    fn a_pin_keeps_its_workspace_in_the_dock() {
        let spaces = [space(10, 1, "eDP-1", true), space(30, 2, "eDP-1", false)];
        let pins = [
            Pin::new("firefox", "eDP-1", None, 2),
            Pin::new("gimp", "eDP-1", None, 4),
            Pin::new("foot", "HDMI-A-1", None, 2),
        ];
        let sections = sections(&spaces, &[], &pins, "eDP-1");
        let at: Vec<(u32, Option<i64>)> = sections.iter().map(|s| (s.index, s.workspace)).collect();
        // the empty workspace 2 shows for its pin; 4 isn't open yet
        assert_eq!(at, [(1, Some(10)), (2, Some(30)), (4, None)]);
        assert_eq!(sections[2].icons, [Icon::Pin(pins[1].clone())]);
    }

    #[test]
    fn a_named_pin_follows_its_workspace_to_where_it_is() {
        let mut web = space(20, 3, "HDMI-A-1", false);
        web.name = Some(String::from("web"));
        let spaces = [space(10, 1, "eDP-1", true), web];
        let pin = Pin::new("firefox", "eDP-1", Some("web"), 1);
        assert!(
            sections(&spaces, &[], std::slice::from_ref(&pin), "eDP-1")[0]
                .icons
                .is_empty()
        );
        let there = sections(&spaces, &[], &[pin], "HDMI-A-1");
        assert_eq!(there[0].workspace, Some(20));
        assert_eq!(there[0].key(), "name:web");
    }

    #[test]
    fn a_pin_opens_the_latest_window_of_its_app_anywhere() {
        let windows = [
            win(1, "foot", 1, 1),
            win(2, "foot", 2, 1),
            win(9, "firefox", 1, 2),
        ];
        assert_eq!(latest("foot", &windows, &[9, 2, 1]), Some(2));
        assert_eq!(
            latest("foot", &windows, &[]),
            Some(1),
            "never focused: the oldest"
        );
        assert_eq!(latest("gimp", &windows, &[9]), None);
    }

    #[test]
    fn the_latest_focus_comes_first_and_closed_windows_drop_out() {
        let open = [win(1, "foot", 1, 1), win(2, "foot", 1, 2)];
        let mut recent = vec![1, 7];
        remember(&mut recent, Some(2), &open);
        assert_eq!(recent, [2, 1]);
        remember(&mut recent, Some(2), &open);
        assert_eq!(recent, [2, 1]);
    }
}
