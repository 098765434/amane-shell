//! Emaki Settings' pages, written once: the sidebar, `--page`, the search
//! index, the toolbar and what shows in the page's place all go by
//! [`SIDEBAR`]. Adding a page is adding its entry here.

use amane::{Column, Widget};
use emaki_kit::app::Page;
use emaki_kit::color::Accent;
use emaki_kit::icons::{
    ICON_APPS, ICON_COMPUTER, ICON_DARK_MODE, ICON_DISPLAY, ICON_DNS, ICON_HISTORY, ICON_INFO,
    ICON_KEYBOARD, ICON_LAYERS, ICON_LOCK, ICON_MORE_HORIZ, ICON_NOTIFICATIONS, ICON_OPEN_IN_NEW,
    ICON_PACKAGE, ICON_RESIZE, ICON_SEARCH, ICON_SETTINGS, ICON_STORAGE, ICON_TRAY, ICON_TUNE,
    ICON_UPDATE, ICON_VOLUME_UP, ICON_WIFI,
};

use crate::{
    about, appearance, desktop, displays, dock, drop, input, launcher, network, notifications,
    passwords, quick_settings, sound, status_bar, system, windows,
};

/// A page's sub-page by name: its title and body, or none for a name it
/// doesn't have.
pub type Sub = fn(&str) -> Option<(String, Box<dyn Widget>)>;

/// A button of a page's own at the toolbar's trailing edge, when it has one
/// now.
pub type Toolbar = fn() -> Option<Box<dyn Widget>>;

/// One page.
pub struct Def {
    /// Its `--page` name, and what its scroll offset is kept under.
    pub name: &'static str,
    /// Other names `--page` takes for it.
    pub aliases: &'static [&'static str],
    /// Its title, in the sidebar and over the page.
    pub title: &'static str,
    /// Other words a search finds it by.
    pub words: &'static [&'static str],
    /// The glyph on its tile.
    pub glyph: char,
    /// Its tile's colour.
    pub accent: Accent,
    /// What it shows.
    pub body: fn() -> Column,
    /// The sub-pages `--page <name>/<sub>` reaches, and how to draw one.
    pub subs: (&'static [&'static str], Sub),
    /// A button of its own at the toolbar's trailing edge.
    pub toolbar: Option<Toolbar>,
}

/// A row of the sidebar.
pub enum Entry {
    /// A title over a run of pages.
    Section(&'static str),
    /// A page.
    Page(Def),
}

const NO_SUBS: (&[&str], Sub) = (&[], |_| None);

/// A page with nothing but its body.
const fn page(
    name: &'static str,
    title: &'static str,
    glyph: char,
    accent: Accent,
    body: fn() -> Column,
) -> Entry {
    Entry::Page(Def {
        name,
        aliases: &[],
        title,
        words: &[],
        glyph,
        accent,
        body,
        subs: NO_SUBS,
        toolbar: None,
    })
}

const fn found_by(entry: Entry, words: &'static [&'static str]) -> Entry {
    match entry {
        Entry::Page(def) => Entry::Page(Def { words, ..def }),
        section => section,
    }
}

const fn aliased(entry: Entry, aliases: &'static [&'static str]) -> Entry {
    match entry {
        Entry::Page(def) => Entry::Page(Def { aliases, ..def }),
        section => section,
    }
}

const fn with_subs(entry: Entry, names: &'static [&'static str], sub: Sub) -> Entry {
    match entry {
        Entry::Page(def) => Entry::Page(Def {
            subs: (names, sub),
            ..def
        }),
        section => section,
    }
}

const fn with_toolbar(entry: Entry, toolbar: Toolbar) -> Entry {
    match entry {
        Entry::Page(def) => Entry::Page(Def {
            toolbar: Some(toolbar),
            ..def
        }),
        section => section,
    }
}

/// The sidebar, top to bottom; a page is its place in this list.
pub const SIDEBAR: [Entry; 26] = [
    Entry::Section("Shell"),
    found_by(
        page(
            "appearance",
            "Appearance",
            ICON_DARK_MODE,
            Accent::Blue,
            appearance::page,
        ),
        &["look", "theme"],
    ),
    found_by(
        page("desktop", "Desktop", ICON_APPS, Accent::Cyan, desktop::page),
        &["wallpaper"],
    ),
    with_subs(
        aliased(
            found_by(
                page(
                    "displays",
                    "Displays",
                    ICON_DISPLAY,
                    Accent::Teal,
                    displays::page,
                ),
                &["monitor", "screen"],
            ),
            &["display", "monitors"],
        ),
        &["advanced"],
        displays::sub,
    ),
    aliased(
        found_by(
            page(
                "windows",
                "Windows",
                ICON_RESIZE,
                Accent::Indigo,
                windows::page,
            ),
            &[
                "gaps",
                "focus ring",
                "border",
                "shadow",
                "title bars",
                "alt tab",
                "window switcher",
                "overview",
                "hot corner",
                "column width",
                "tiling",
            ],
        ),
        &["layout", "multitasking"],
    ),
    with_subs(
        aliased(
            found_by(
                page("input", "Input", ICON_KEYBOARD, Accent::Gray, input::page),
                &[
                    "keyboard",
                    "mouse",
                    "touchpad",
                    "trackpad",
                    "key repeat",
                    "scrolling",
                    "shortcuts",
                    "keybinds",
                    "hotkeys",
                ],
            ),
            &["keyboard", "mouse", "touchpad"],
        ),
        &["shortcuts"],
        input::sub,
    ),
    aliased(
        found_by(
            page("sound", "Sound", ICON_VOLUME_UP, Accent::Pink, sound::page),
            &[
                "volume",
                "speakers",
                "headphones",
                "microphone",
                "balance",
                "alert sound",
                "audio output",
                "audio input",
                "app volume",
            ],
        ),
        &["audio"],
    ),
    with_subs(
        aliased(
            found_by(
                page(
                    "status-bar",
                    "Status Bar",
                    ICON_MORE_HORIZ,
                    Accent::Gray,
                    status_bar::page,
                ),
                &["menu bar", "panel", "taskbar", "top bar"],
            ),
            &["bar"],
        ),
        &["clock", "weather", "items", "activities"],
        status_bar::sub,
    ),
    found_by(
        page("dock", "Dock", ICON_LAYERS, Accent::Indigo, dock::page),
        &["taskbar", "launcher bar"],
    ),
    found_by(
        page(
            "notifications",
            "Notifications",
            ICON_NOTIFICATIONS,
            Accent::Red,
            notifications::page,
        ),
        &["banners", "alerts", "toasts", "notification center"],
    ),
    with_toolbar(
        found_by(
            page(
                "quick-settings",
                "Quick Settings",
                ICON_TUNE,
                Accent::Gray,
                quick_settings::page,
            ),
            &["controls", "wifi", "bluetooth", "volume", "toggles"],
        ),
        quick_settings::toolbar,
    ),
    aliased(
        found_by(
            page(
                "launcher",
                "Launcher",
                ICON_SEARCH,
                Accent::Purple,
                launcher::page,
            ),
            &["search", "app launcher", "run", "apps"],
        ),
        &["spotlight"], // lint-terms: allow (the old name, for keybinds)
    ),
    aliased(
        found_by(
            page("drop", "Drop", ICON_TRAY, Accent::Orange, drop::page),
            &["emaki-drop", "grid", "shelf", "share"],
        ),
        &["emaki-drop"],
    ),
    aliased(
        found_by(
            page(
                "passwords",
                "Passwords",
                ICON_LOCK,
                Accent::Gray,
                passwords::page,
            ),
            &[
                "bitwarden",
                "vaultwarden",
                "rbw",
                "autofill",
                "fill",
                "vault",
                "login",
                "keychain",
            ],
        ),
        &["bitwarden"],
    ),
    with_subs(
        found_by(
            page(
                "about-shell",
                "About",
                ICON_COMPUTER,
                Accent::Gray,
                about::page,
            ),
            &["version"],
        ),
        &["files"],
        about::sub,
    ),
    Entry::Section("System"),
    with_subs(
        aliased(
            found_by(
                page(
                    "about",
                    "About This Computer",
                    ICON_INFO,
                    Accent::Gray,
                    system::about::page,
                ),
                &["nixos", "kernel", "version", "generation", "system"],
            ),
            &["system", "computer"],
        ),
        &["advanced"],
        system::about::sub,
    ),
    found_by(
        page(
            "general",
            "General",
            ICON_SETTINGS,
            Accent::Gray,
            system::general::page,
        ),
        &[
            "computer name",
            "hostname",
            "language",
            "keyboard",
            "layout",
        ],
    ),
    with_subs(
        aliased(
            found_by(
                page("network", "Network", ICON_WIFI, Accent::Blue, network::page),
                &[
                    "wifi",
                    "wi-fi",
                    "ethernet",
                    "vpn",
                    "wireguard",
                    "openvpn",
                    "hotspot",
                    "dns",
                    "ip address",
                    "internet",
                ],
            ),
            &["wifi", "vpn", "ethernet", "hotspot"],
        ),
        &["saved", "hidden", "connection"],
        network::sub,
    ),
    aliased(
        found_by(
            page(
                "apps",
                "Apps",
                ICON_PACKAGE,
                Accent::Blue,
                system::apps::page,
            ),
            &[
                "install",
                "software",
                "packages",
                "nixpkgs",
                "flatpak",
                "uninstall",
            ],
        ),
        &["software"],
    ),
    with_subs(
        aliased(
            found_by(
                page(
                    "default-apps",
                    "Default Apps",
                    ICON_OPEN_IN_NEW,
                    Accent::Purple,
                    system::defaults::page,
                ),
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
            &["defaults", "open-with"],
        ),
        &["file-types"],
        system::defaults::sub,
    ),
    found_by(
        page(
            "services",
            "Services",
            ICON_DNS,
            Accent::Green,
            system::services::page,
        ),
        &["ssh", "tailscale", "syncthing", "bluetooth", "daemons"],
    ),
    aliased(
        found_by(
            page(
                "power",
                "Power & Lock",
                ICON_LOCK,
                Accent::Orange,
                system::power::page,
            ),
            &["sleep", "lock screen", "idle", "screen off", "dim"],
        ),
        &["lock"],
    ),
    found_by(
        page(
            "storage",
            "Storage",
            ICON_STORAGE,
            Accent::Gray,
            system::storage::page,
        ),
        &["disk", "space", "clean up", "garbage", "free space"],
    ),
    found_by(
        page(
            "history",
            "History",
            ICON_HISTORY,
            Accent::Brown,
            system::history::page,
        ),
        &["rollback", "generations", "go back", "restore"],
    ),
    aliased(
        found_by(
            page(
                "update",
                "Software Update",
                ICON_UPDATE,
                Accent::Red,
                system::update::page,
            ),
            &["upgrade", "updates", "flake.lock"],
        ),
        &["updates"],
    ),
];

/// The page at `index`, if it is one.
pub fn def(index: usize) -> Option<&'static Def> {
    match SIDEBAR.get(index)? {
        Entry::Page(def) => Some(def),
        Entry::Section(_) => None,
    }
}

/// Every page and where it is, in the sidebar's order.
pub fn all() -> impl Iterator<Item = (usize, &'static Def)> {
    (0..SIDEBAR.len()).filter_map(|index| def(index).map(|def| (index, def)))
}

/// The page `--page <name>` opens: its name or one of its aliases, in any
/// case.
pub fn named(name: &str) -> Option<usize> {
    let name = name.trim().to_lowercase();
    all()
        .find(|(_, def)| def.name == name || def.aliases.contains(&name.as_str()))
        .map(|(index, _)| index)
}

/// Whether `index` is the page `name`.
pub fn is(index: usize, name: &str) -> bool {
    def(index).is_some_and(|def| def.name == name)
}

/// The sidebar's rows, for the window.
pub fn sidebar() -> Vec<Page> {
    SIDEBAR
        .iter()
        .map(|entry| match entry {
            Entry::Section(title) => Page::section(*title),
            Entry::Page(def) => Page::new(def.glyph, def.title).accent(def.accent),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_and_alias_opens_its_own_page() {
        let mut seen = std::collections::HashSet::new();
        for (index, def) in all() {
            for name in std::iter::once(&def.name).chain(def.aliases) {
                assert!(seen.insert(*name), "{name} names two pages");
                assert_eq!(named(name), Some(index), "{name} isn't its page");
            }
        }
        assert_eq!(named(" Status-Bar "), named("bar"));
        assert_eq!(named("nowhere"), None);
    }
}
