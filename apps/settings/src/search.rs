//! Searching Shell Settings, as the field over macOS's sidebar does: what's
//! typed finds pages and the settings on them, by their titles and by
//! other words people use for them ("dark mode", "wifi"). A result opens
//! its page (or sub-page) and lights the row it names for a moment.
//!
//! The Shell pages' settings are listed here by hand; a test checks that
//! every page has some. The System pages' settings come from the
//! configuration as it's read, so they're found by their labels too.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use amane::{Service, TextInput};

use crate::system::nixos::Snapshot;

/// The search field's id.
pub const FIELD: &str = "settings.search";

/// One thing to find: where it is (`--page` names, `page/sub` for a
/// sub-page), its title, the row it lights, and other words for it.
pub struct Entry {
    pub place: &'static str,
    pub title: &'static str,
    pub row: Option<&'static str>,
    pub words: &'static [&'static str],
}

const fn page(place: &'static str, title: &'static str, words: &'static [&'static str]) -> Entry {
    Entry {
        place,
        title,
        row: None,
        words,
    }
}

const fn row(
    place: &'static str,
    title: &'static str,
    row: &'static str,
    words: &'static [&'static str],
) -> Entry {
    Entry {
        place,
        title,
        row: Some(row),
        words,
    }
}

/// Every page and the Shell pages' settings.
pub const INDEX: &[Entry] = &[
    // Appearance
    page("appearance", "Appearance", &["look", "theme"]),
    row(
        "appearance",
        "Light, dark or auto",
        "settings.appearance.mode",
        &["dark mode", "light mode", "night", "theme"],
    ),
    row(
        "appearance",
        "Accent color",
        "settings.appearance.accent",
        &["colour", "highlight", "tint"],
    ),
    row(
        "appearance",
        "Text size",
        "settings.appearance.text",
        &["font", "bigger", "zoom", "scale text"],
    ),
    row(
        "appearance",
        "Density",
        "settings.appearance.density",
        &["compact", "spacing", "comfortable"],
    ),
    row(
        "appearance",
        "Glass",
        "settings.appearance.glass",
        &["transparency", "translucent", "blur", "material"],
    ),
    row(
        "appearance",
        "Adaptive glass",
        "settings.appearance.adaptive",
        &[],
    ),
    row(
        "appearance",
        "Vibrant labels",
        "settings.appearance.vibrant",
        &["vibrancy"],
    ),
    row(
        "appearance",
        "Motion",
        "settings.appearance.motion",
        &["reduce motion", "animations"],
    ),
    // Desktop
    page("desktop", "Desktop", &["wallpaper"]),
    row(
        "desktop",
        "Show widgets on the desktop",
        "settings.show-widgets",
        &["widgets"],
    ),
    row(
        "desktop",
        "Widgets while windows are open",
        "settings.widget-style",
        &["dim widgets"],
    ),
    row(
        "desktop",
        "Edit widgets",
        "settings.widgets.edit-row",
        &["add widget", "move widget"],
    ),
    // Displays
    page("displays", "Displays", &["monitor", "screen"]),
    row(
        "displays",
        "Resolution",
        "settings.displays.resolution-row",
        &["size", "pixels"],
    ),
    row(
        "displays",
        "Refresh rate",
        "settings.displays.refresh-row",
        &["hz", "fps"],
    ),
    row(
        "displays",
        "Scale",
        "settings.displays.scale-row",
        &["zoom", "hidpi", "bigger"],
    ),
    row(
        "displays",
        "Rotation",
        "settings.displays.rotation-row",
        &["rotate", "portrait"],
    ),
    row(
        "displays",
        "Variable refresh rate",
        "settings.displays.vrr-row",
        &["vrr", "freesync", "adaptive sync", "gsync"],
    ),
    row(
        "displays",
        "HDR",
        "settings.displays.hdr-row",
        &["high dynamic range"],
    ),
    row(
        "displays",
        "Use this display",
        "settings.displays.enabled-row",
        &["turn off display"],
    ),
    row(
        "displays",
        "Main display",
        "settings.displays.main-row",
        &["primary"],
    ),
    row(
        "displays",
        "Slower on battery",
        "settings.displays.battery-refresh",
        &["power saving"],
    ),
    row(
        "displays",
        "Night Light",
        "settings.displays.night.schedule-row",
        &["blue light", "warm", "night shift", "redshift"],
    ),
    row(
        "displays",
        "OLED care",
        "settings.displays.oled.dark",
        &["burn in"],
    ),
    row(
        "displays/advanced",
        "Custom scale",
        "settings.displays.custom-scale-row",
        &[],
    ),
    row(
        "displays/advanced",
        "Custom mode",
        "settings.displays.custom-mode-row",
        &["modeline"],
    ),
    row(
        "displays/advanced",
        "Overview backdrop",
        "settings.displays.backdrop-row",
        &["background color"],
    ),
    // Status Bar
    page(
        "status-bar",
        "Status Bar",
        &["menu bar", "panel", "taskbar", "top bar"],
    ),
    row(
        "status-bar",
        "Bar position",
        "settings.bar.position",
        &["top", "bottom"],
    ),
    row(
        "status-bar",
        "Hide the bar automatically",
        "settings.bar.auto-hide",
        &["auto hide", "full screen"],
    ),
    row(
        "status-bar",
        "Bar material",
        "settings.bar.material",
        &["transparent bar", "clear", "solid"],
    ),
    row(
        "status-bar",
        "Workspaces in the bar",
        "settings.bar.workspaces",
        &["pips"],
    ),
    row(
        "status-bar/activities",
        "Now Playing",
        "settings.bar.media-always",
        &["music", "media", "spotify"],
    ),
    row(
        "status-bar/activities",
        "Live Activities",
        "settings.bar.activities",
        &["activity", "timer", "screen sharing", "progress", "upload"],
    ),
    row(
        "status-bar/activities",
        "Allow Live Activities",
        "settings.bar.activities-compact",
        &["apps", "permission"],
    ),
    row(
        "status-bar/clock",
        "Clock",
        "settings.bar.clock-style-row",
        &["time", "analog", "digital"],
    ),
    row(
        "status-bar/clock",
        "24-hour time",
        "settings.bar.clock-24h",
        &["am pm"],
    ),
    row(
        "status-bar/clock",
        "Show seconds",
        "settings.bar.clock-seconds",
        &[],
    ),
    row(
        "status-bar/clock",
        "Flash the time separators",
        "settings.bar.flash",
        &["blink", "colon"],
    ),
    row(
        "status-bar/clock",
        "Show the date",
        "settings.bar.show-date",
        &["day"],
    ),
    row(
        "status-bar/clock",
        "Show the day of the week",
        "settings.bar.weekday",
        &["weekday"],
    ),
    row(
        "status-bar/weather",
        "Weather",
        "settings.bar.weather",
        &["temperature", "forecast"],
    ),
    row(
        "status-bar/items",
        "Status bar items",
        "settings.bar.order",
        &["icons", "order", "reorder"],
    ),
    row(
        "status-bar/items",
        "Allow in the Status Bar",
        "settings.bar.order",
        &["tray", "system tray", "app icons"],
    ),
    // Dock
    page("dock", "Dock", &["taskbar", "launcher bar"]),
    row(
        "dock",
        "Dock size",
        "settings.dock.magnify",
        &["icon size", "magnification"],
    ),
    row(
        "dock",
        "Hide the dock automatically",
        "settings.dock.auto-hide",
        &["auto hide"],
    ),
    row(
        "dock",
        "Dots under open apps",
        "settings.dock.dots",
        &["indicators"],
    ),
    row(
        "dock",
        "App names over icons",
        "settings.dock.names",
        &["tooltips"],
    ),
    row(
        "dock",
        "Animate the dock",
        "settings.dock.animations",
        &["bounce"],
    ),
    // Quick Settings
    page(
        "quick-settings",
        "Quick Settings",
        &["control center", "wifi", "bluetooth", "volume", "toggles"],
    ),
    row(
        "quick-settings",
        "Reset the Quick Settings layout",
        "settings.quick.reset-row",
        &["customize"],
    ),
    // Spotlight
    page(
        "spotlight",
        "Spotlight",
        &["launcher", "search", "app launcher", "run"],
    ),
    row(
        "spotlight",
        "Show applications as",
        "settings.launcher.layout",
        &["grid", "list"],
    ),
    row(
        "spotlight",
        "Clipboard history",
        "settings.launcher.clipboard",
        &["copy", "paste"],
    ),
    row(
        "spotlight",
        "Quick keys",
        "settings.launcher.keys.none",
        &["shortcuts", "hotkeys"],
    ),
    // Drop
    page("drop", "Drop", &["dropzone", "amane-drop", "grid", "shelf", "share"]),
    row(
        "drop",
        "Tiles a row",
        "settings.drop.columns",
        &["columns", "grid size"],
    ),
    row(
        "drop",
        "Copy the link",
        "settings.drop.copy-link",
        &["clipboard", "upload link", "recently shared"],
    ),
    row(
        "drop",
        "Send to Phone",
        "settings.drop.phone",
        &["kde connect", "phone", "share"],
    ),
    row(
        "drop",
        "Imgur",
        "settings.drop.imgur.id.row",
        &["client id", "upload pictures"],
    ),
    row(
        "drop",
        "SFTP server",
        "settings.drop.sftp.host.row",
        &["ssh", "scp", "upload", "server"],
    ),
    row(
        "drop",
        "S3 bucket",
        "settings.drop.s3.bucket.row",
        &["aws", "r2", "minio", "upload", "bucket"],
    ),
    // About
    page("about-shell", "About Shell Settings", &["version"]),
    page(
        "about-shell/files",
        "Settings files",
        &["json", "config files"],
    ),
    // System
    page(
        "about",
        "About This Computer",
        &["nixos", "kernel", "version", "generation", "system"],
    ),
    page(
        "about/advanced",
        "Configuration folder",
        &["flake", "repository", "nixos-config"],
    ),
    page(
        "general",
        "General",
        &[
            "computer name",
            "hostname",
            "language",
            "keyboard",
            "layout",
        ],
    ),
    page(
        "apps",
        "Apps",
        &[
            "install",
            "software",
            "packages",
            "nixpkgs",
            "flatpak",
            "uninstall",
        ],
    ),
    page(
        "default-apps",
        "Default Apps",
        &[
            "default browser",
            "open with",
            "file types",
            "associations",
            "mime",
            "terminal",
            "mail",
            "pdf",
        ],
    ),
    row(
        "default-apps/file-types",
        "File Types",
        "settings.defaults.type-row",
        &["extension", "kind of file", "reset"],
    ),
    page(
        "services",
        "Services",
        &["ssh", "tailscale", "syncthing", "bluetooth", "daemons"],
    ),
    page(
        "power",
        "Power & Lock",
        &["sleep", "lock screen", "idle", "screen off", "dim"],
    ),
    page(
        "storage",
        "Storage",
        &["disk", "space", "clean up", "garbage", "free space"],
    ),
    page(
        "history",
        "History",
        &["rollback", "generations", "go back", "restore"],
    ),
    page(
        "update",
        "Software Update",
        &["upgrade", "updates", "flake.lock"],
    ),
];

/// The page and sub-page a place is on, for a result's second line.
fn where_is(place: &str) -> String {
    let names = [
        ("appearance", "Appearance"),
        ("desktop", "Desktop"),
        ("displays", "Displays"),
        ("status-bar", "Status Bar"),
        ("dock", "Dock"),
        ("quick-settings", "Quick Settings"),
        ("spotlight", "Spotlight"),
        ("drop", "Drop"),
        ("about-shell", "About"),
        ("about", "About This Computer"),
        ("general", "General"),
        ("apps", "Apps"),
        ("default-apps", "Default Apps"),
        ("services", "Services"),
        ("power", "Power & Lock"),
        ("storage", "Storage"),
        ("history", "History"),
        ("update", "Software Update"),
    ];
    let (page, sub) = match place.split_once('/') {
        Some((page, sub)) => (page, Some(sub)),
        None => (place, None),
    };
    let page = names
        .iter()
        .find(|(name, _)| *name == page)
        .map_or(page, |(_, title)| title)
        .to_string();
    match sub {
        Some(sub) => {
            let mut sub = sub.to_string();
            if let Some(first) = sub.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            format!("{page} › {sub}")
        }
        None => page,
    }
}

/// A result: where it goes and what it lights.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub place: String,
    pub title: String,
    pub row: Option<String>,
    pub path: String,
    score: u8,
}

/// How well `text` matches `query`: a title starting with it beats one
/// containing it, which beats another word for it.
fn score(query: &str, title: &str, words: &[&str]) -> Option<u8> {
    let title = title.to_lowercase();
    if title.starts_with(query) {
        return Some(0);
    }
    if title.split_whitespace().any(|word| word.starts_with(query)) {
        return Some(1);
    }
    if title.contains(query) {
        return Some(2);
    }
    words.iter().any(|word| word.contains(query)).then_some(3)
}

/// What `query` finds, best first; nothing for an empty query.
pub fn find(query: &str, system: &[(String, String, String)]) -> Vec<Found> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<Found> = INDEX
        .iter()
        .filter_map(|entry| {
            Some(Found {
                place: entry.place.to_string(),
                title: entry.title.to_string(),
                row: entry.row.map(String::from),
                path: where_is(entry.place),
                score: score(&query, entry.title, entry.words)?,
            })
        })
        .collect();
    // the System pages' settings, by their labels
    for (page, id, label) in system {
        if let Some(score) = score(&query, label, &[]) {
            found.push(Found {
                place: page.clone(),
                title: label.clone(),
                row: Some(format!("settings.system.row.{id}")),
                path: where_is(page),
                score,
            });
        }
    }
    found.sort_by_key(|found| found.score);
    found.dedup_by(|a, b| a.title == b.title && a.place == b.place);
    found.truncate(30);
    found
}

/// The configuration's settings now, as (page, id, label).
pub fn system_entries() -> Vec<(String, String, String)> {
    Snapshot::read()
        .entries
        .iter()
        .filter(|entry| !entry.page.is_empty())
        .map(|entry| (entry.page.clone(), entry.id.clone(), entry.label.clone()))
        .collect()
}

/// How long a row stays lit after a search led to it.
pub const LIT: Duration = Duration::from_millis(1600);

thread_local! {
    /// The row a search led to, and when.
    static LIT_ROW: RefCell<Option<(String, Instant)>> = const { RefCell::new(None) };
    /// Ctrl+F asked for the field.
    static FOCUS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Opens a result: its place, the row lit, the search put away.
pub fn open(found: &Found) {
    crate::go(&found.place);
    LIT_ROW.with_borrow_mut(|lit| *lit = found.row.clone().map(|row| (row, Instant::now())));
    TextInput::set_text(FIELD, "");
    // the light goes out by itself
    amane_kit::motion::redraw_after::<crate::Navigation>(LIT);
}

/// The row to light while drawing, if it's still lit.
pub fn lit_row() -> Option<String> {
    LIT_ROW.with_borrow_mut(|lit| {
        if lit.as_ref().is_some_and(|(_, at)| at.elapsed() >= LIT) {
            *lit = None;
        }
        lit.as_ref().map(|(row, _)| row.clone())
    })
}

/// Ctrl+F: the field takes the keys at the next drawing.
pub fn focus() {
    FOCUS.set(true);
}

pub fn take_focus() -> bool {
    FOCUS.replace(false)
}

/// Escape: clears what's typed, if anything was.
pub fn clear() -> bool {
    if TextInput::text(FIELD).is_empty() {
        return false;
    }
    TextInput::set_text(FIELD, "");
    drop(crate::Navigation::write());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_can_be_found() {
        for name in [
            "appearance",
            "desktop",
            "displays",
            "status-bar",
            "dock",
            "quick-settings",
            "spotlight",
            "drop",
            "about-shell",
            "about",
            "general",
            "apps",
            "default-apps",
            "services",
            "power",
            "storage",
            "history",
            "update",
        ] {
            assert!(
                INDEX.iter().any(|entry| entry.place == name),
                "{name} has nothing to find"
            );
            assert!(crate::page_named(name).is_some(), "{name} isn't a page");
        }
        for entry in INDEX {
            let page = entry.place.split('/').next().unwrap();
            assert!(
                crate::page_named(page).is_some(),
                "{} isn't a page",
                entry.place
            );
        }
    }

    #[test]
    fn titles_beat_other_words() {
        let found = find("dark", &[]);
        assert_eq!(found[0].place, "appearance");
        let seconds = find("seconds", &[]);
        assert_eq!(seconds[0].place, "status-bar/clock");
        assert_eq!(
            seconds[0].row.as_deref(),
            Some("settings.bar.clock-seconds")
        );
        assert_eq!(seconds[0].path, "Status Bar › Clock");
        assert!(
            find("wifi", &[])
                .iter()
                .any(|f| f.place == "quick-settings")
        );
        assert!(find("  ", &[]).is_empty());
        let system = vec![(
            String::from("general"),
            String::from("hostName"),
            String::from("Computer name"),
        )];
        assert!(
            find("computer n", &system)
                .iter()
                .any(|f| f.row.as_deref() == Some("settings.system.row.hostName"))
        );
    }
}
