//! Shell Settings: changes amane-shell's settings file, which the running
//! shell follows at once, and, under System, the NixOS configuration the
//! machine is built from. One copy runs; launching it again brings it up.

mod about;
mod appearance;
mod desktop;
mod displays;
mod dock;
mod launcher;
mod quick_settings;
mod status_bar;
mod system;

use amane::{Center, Column, Key, Row, ScrollArea, Service, Size, Text, Widget, Window};
use amane_kit::app::{Dialog, Page, app, app_window, sidebar, standard_shortcut};
use amane_kit::icons::{
    ICON_APPS, ICON_COMPUTER, ICON_DARK_MODE, ICON_DISPLAY, ICON_DNS, ICON_HISTORY, ICON_INFO,
    ICON_LAYERS, ICON_LOCK, ICON_MORE_HORIZ, ICON_PACKAGE, ICON_SEARCH, ICON_SETTINGS,
    ICON_STORAGE, ICON_TUNE, ICON_UPDATE,
};
use amane_kit::layout::boxed;
use amane_kit::material::{self, Level};
use amane_kit::tokens::{Styled, sys};

use system::apply_bar::{self, Overlay};
use system::job::BuildJob;

// which page shows, kept while the app runs
pub struct Navigation {
    page: usize,
    /// A view in the page's place: the changes an Apply makes, or its log.
    pub overlay: Option<Overlay>,
    /// The generation picked in History.
    pub selected_generation: Option<u32>,
}

impl Service for Navigation {
    fn new() -> Self {
        Self {
            page: 1,
            overlay: None,
            selected_generation: None,
        }
    }

    // only clicks change it
    fn listen() {}
}

// the sidebar's rows: two sections, each over its pages
const DISPLAYS: usize = 3;
const SYS_ABOUT: usize = 10;
const SYS_GENERAL: usize = 11;
const SYS_APPS: usize = 12;
const SYS_SERVICES: usize = 13;
const SYS_POWER: usize = 14;
const SYS_STORAGE: usize = 15;
const SYS_HISTORY: usize = 16;
const SYS_UPDATE: usize = 17;

/// The page `--page <name>` opens: the sidebar's names, lowercased, or a
/// System page's short name.
fn page_named(name: &str) -> Option<usize> {
    Some(match name.trim().to_lowercase().as_str() {
        "appearance" => 1,
        "desktop" => 2,
        "displays" | "display" | "monitors" => DISPLAYS,
        "status-bar" | "bar" => 4,
        "dock" => 5,
        "quick-settings" => 6,
        "spotlight" | "launcher" => 7,
        "about-shell" => 8,
        "about" | "system" | "computer" => SYS_ABOUT,
        "general" => SYS_GENERAL,
        "apps" | "software" => SYS_APPS,
        "services" => SYS_SERVICES,
        "power" | "lock" => SYS_POWER,
        "storage" => SYS_STORAGE,
        "history" => SYS_HISTORY,
        "update" | "updates" => SYS_UPDATE,
        _ => return None,
    })
}

/// Shows the page `--page <name>` would, from a page's own button.
pub fn go(name: &str) {
    if let Some(page) = page_named(name) {
        let mut nav = Navigation::write();
        nav.page = page;
        nav.overlay = None;
    }
}

/// The page named after `--page` in `args`, if any.
fn page_arg(args: &[String]) -> Option<usize> {
    flag(args, "--page").and_then(|name| page_named(&name))
}

/// `--name value` or `--name=value`.
fn flag(args: &[String], name: &str) -> Option<String> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if let Some(value) = arg
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
        {
            return Some(value.to_string());
        }
        if arg == name {
            return args.next().cloned();
        }
    }
    None
}

/// `--package <attr>`: the Apps page, with that nixpkgs package's details.
fn package_arg(args: &[String]) {
    if let Some(attr) = flag(args, "--package") {
        Navigation::write().page = SYS_APPS;
        system::package::Details::open(&attr);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(page) = page_arg(&args) {
        Navigation::write().page = page;
    }
    package_arg(&args);
    app("amane-settings")
        .normal_window("main", main_window)
        // launched again (Spotlight, a notification) while running: show that page
        .on_activate(|args| {
            if let Some(page) = page_arg(args) {
                let mut nav = Navigation::write();
                nav.page = page;
                nav.overlay = None;
            }
            package_arg(args);
        })
        .run();
}

fn pages() -> [Page; 18] {
    [
        Page::section("Shell"),
        Page::new(ICON_DARK_MODE, "Appearance"),
        Page::new(ICON_APPS, "Desktop"),
        Page::new(ICON_DISPLAY, "Displays"),
        Page::new(ICON_MORE_HORIZ, "Status Bar"),
        Page::new(ICON_LAYERS, "Dock"),
        Page::new(ICON_TUNE, "Quick Settings"),
        Page::new(ICON_SEARCH, "Spotlight"),
        Page::new(ICON_COMPUTER, "About"),
        Page::section("System"),
        Page::new(ICON_INFO, "About This Computer"),
        Page::new(ICON_SETTINGS, "General"),
        Page::new(ICON_PACKAGE, "Apps"),
        Page::new(ICON_DNS, "Services"),
        Page::new(ICON_LOCK, "Power & Lock"),
        Page::new(ICON_STORAGE, "Storage"),
        Page::new(ICON_HISTORY, "History"),
        Page::new(ICON_UPDATE, "Software Update"),
    ]
}

/// Closing while a build runs would end it (nix stops a build whose client
/// goes away), so ask first.
fn may_close() -> bool {
    if !BuildJob::read().running() {
        return true;
    }
    Dialog::new("settings.close-dialog", "Stop the build?")
        .message("Closing Shell Settings stops the build. Nothing has been applied yet.")
        .button("Keep Building", || {})
        .destructive("Stop", || {
            BuildJob::cancel();
            amane::close_window("main");
        })
        .open("main");
    false
}

fn shortcut(key: Key) -> bool {
    // Ctrl+W goes through the same question as the compositor's close
    if key == Key::Character('w') && amane::modifiers().ctrl && !BuildJob::read().running() {
        return standard_shortcut(key);
    }
    if key == Key::Character('w') && amane::modifiers().ctrl {
        return !may_close();
    }
    standard_shortcut(key)
}

fn main_window() -> Window {
    let (page, overlay) = {
        let nav = Navigation::read();
        (nav.page, nav.overlay)
    };

    let pages = pages();
    // its poll rests while another page shows
    displays::showing(page == DISPLAYS && overlay.is_none());

    let nav = sidebar("settings.nav", &pages, page, |page| {
        let mut nav = Navigation::write();
        nav.page = page;
        nav.overlay = None;
    });

    // each page keeps where it was scrolled to, under its own name
    let (title, scroll, body): (&str, &'static str, Box<dyn Widget>) = match page {
        1 => (
            "Appearance",
            "settings.scroll.appearance",
            boxed(appearance::page()),
        ),
        2 => ("Desktop", "settings.scroll.desktop", boxed(desktop::page())),
        DISPLAYS => (
            "Displays",
            "settings.scroll.displays",
            boxed(displays::page()),
        ),
        4 => (
            "Status Bar",
            "settings.scroll.status-bar",
            boxed(status_bar::page()),
        ),
        5 => ("Dock", "settings.scroll.dock", boxed(dock::page())),
        6 => (
            "Quick Settings",
            "settings.scroll.quick-settings",
            boxed(quick_settings::page()),
        ),
        7 => (
            "Spotlight",
            "settings.scroll.spotlight",
            boxed(launcher::page()),
        ),
        SYS_ABOUT => (
            "About This Computer",
            "settings.scroll.system-about",
            boxed(system::about::page()),
        ),
        SYS_GENERAL => (
            "General",
            "settings.scroll.system-general",
            boxed(system::general::page()),
        ),
        SYS_APPS => (
            "Apps",
            "settings.scroll.system-apps",
            boxed(system::apps::page()),
        ),
        SYS_SERVICES => (
            "Services",
            "settings.scroll.system-services",
            boxed(system::services::page()),
        ),
        SYS_POWER => (
            "Power & Lock",
            "settings.scroll.system-power",
            boxed(system::power::page()),
        ),
        SYS_HISTORY => (
            "History",
            "settings.scroll.system-history",
            boxed(system::history::page()),
        ),
        SYS_STORAGE => (
            "Storage",
            "settings.scroll.system-storage",
            boxed(system::storage::page()),
        ),
        SYS_UPDATE => (
            "Software Update",
            "settings.scroll.system-update",
            boxed(system::update::page()),
        ),
        _ => ("About", "settings.scroll.about", boxed(about::page())),
    };

    // the changes, the log or a package take the page's place; the log
    // scrolls itself
    let package_title = if overlay == Some(Overlay::Package) {
        system::package::title()
    } else {
        String::new()
    };
    let (title, body): (&str, Box<dyn Widget>) = match overlay {
        Some(Overlay::Changes) => (
            "Changes",
            boxed(
                ScrollArea::new(
                    "settings.scroll.changes",
                    Column::new(vec![boxed(apply_bar::changes_view())]),
                )
                .width(Size::Parent)
                .height(Size::Parent),
            ),
        ),
        Some(Overlay::Log) => ("Build Log", boxed(apply_bar::log_view())),
        Some(Overlay::Package) => (
            package_title.as_str(),
            boxed(
                ScrollArea::new(
                    "settings.scroll.package",
                    Column::new(vec![boxed(system::package::view())]),
                )
                .width(Size::Parent)
                .height(Size::Parent),
            ),
        ),
        None => (
            title,
            boxed(
                ScrollArea::new(scroll, Column::new(vec![body]))
                    .width(Size::Parent)
                    .height(Size::Parent),
            ),
        ),
    };

    let mut column: Vec<Box<dyn Widget>> = vec![
        boxed(
            Text::new(title)
                .style(sys::r#type::title())
                .color(material::label(Level::Primary)),
        ),
        body,
    ];
    if let Some(bar) = apply_bar::apply_bar() {
        column.push(bar);
    }

    let content = Row::new(vec![
        boxed(
            Column::new(vec![boxed(nav)])
                .width(Size::Fixed(200.0))
                .height(Size::Parent),
        ),
        boxed(
            Column::new(column)
                .gap(sys::space::gap::l())
                .width(Size::Parent)
                .height(Size::Parent),
        ),
    ])
    .gap(sys::space::gap::xl())
    .width(Size::Parent)
    .height(Size::Parent);

    app_window(content)
        .title("Shell Settings")
        .size(760.0, 760.0)
        .min_size(600.0, 740.0)
        .on_shortcut(shortcut)
        .on_close(may_close)
}

/// A control with its name after it on one line, for a switch.
pub fn labelled(name: &str, control: impl Widget + 'static) -> Row {
    Row::new(vec![
        boxed(control),
        boxed(
            Text::new(name)
                .style(sys::r#type::label())
                .color(material::label(Level::Primary)),
        ),
    ])
    .gap(sys::space::gap::m())
    .align(Center)
}

/// How wide a list's rows are on a page: the window's default width less
/// its insets, the sidebar and the gap between.
pub const PAGE_WIDTH: f32 = 760.0 - 2.0 * 16.0 - 200.0 - 24.0;

/// A line of explanation under a setting, in the caption's grey.
pub fn note(text: impl Into<String>) -> Text {
    Text::new(text)
        .style(sys::r#type::caption())
        .color(material::label(Level::Secondary))
}

/// A setting's name over its control, the way every page lays them out.
pub fn setting(name: &str, control: impl Widget + 'static) -> Column {
    Column::new(vec![
        boxed(
            Text::new(name)
                .style(sys::r#type::caption())
                .color(material::label(Level::Secondary)),
        ),
        boxed(control),
    ])
    .gap(sys::space::gap::xs())
}
