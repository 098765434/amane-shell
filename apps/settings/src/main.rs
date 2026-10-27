//! Shell Settings: changes amane-shell's settings file, which the running
//! shell follows at once, and, under System, the NixOS configuration the
//! machine is built from. One copy runs; launching it again brings it up.

mod about;
mod appearance;
mod desktop;
mod dock;
mod launcher;
mod quick_settings;
mod status_bar;
mod system;

use amane::{Center, Column, Key, Row, ScrollArea, Service, Size, Text, Widget, Window};
use amane_kit::app::{Dialog, Page, app, app_window, sidebar, standard_shortcut};
use amane_kit::icons::{
    ICON_APPS, ICON_COMPUTER, ICON_DARK_MODE, ICON_HISTORY, ICON_INFO, ICON_LAYERS,
    ICON_MORE_HORIZ, ICON_SEARCH, ICON_STORAGE, ICON_TUNE, ICON_UPDATE,
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
const SYS_ABOUT: usize = 9;
const SYS_HISTORY: usize = 10;
const SYS_STORAGE: usize = 11;
const SYS_UPDATE: usize = 12;

fn main() {
    app("amane-settings")
        .normal_window("main", main_window)
        .run();
}

fn pages() -> [Page; 13] {
    [
        Page::section("Shell"),
        Page::new(ICON_DARK_MODE, "Appearance"),
        Page::new(ICON_APPS, "Desktop"),
        Page::new(ICON_MORE_HORIZ, "Status Bar"),
        Page::new(ICON_LAYERS, "Dock"),
        Page::new(ICON_TUNE, "Quick Settings"),
        Page::new(ICON_SEARCH, "Spotlight"),
        Page::new(ICON_COMPUTER, "About"),
        Page::section("System"),
        Page::new(ICON_INFO, "About This Computer"),
        Page::new(ICON_HISTORY, "History"),
        Page::new(ICON_STORAGE, "Storage"),
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
        3 => (
            "Status Bar",
            "settings.scroll.status-bar",
            boxed(status_bar::page()),
        ),
        4 => ("Dock", "settings.scroll.dock", boxed(dock::page())),
        5 => (
            "Quick Settings",
            "settings.scroll.quick-settings",
            boxed(quick_settings::page()),
        ),
        6 => (
            "Spotlight",
            "settings.scroll.spotlight",
            boxed(launcher::page()),
        ),
        SYS_ABOUT => (
            "About This Computer",
            "settings.scroll.system-about",
            boxed(system::about::page()),
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

    // the changes or the log take the page's place; the log scrolls itself
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
        .size(760.0, 520.0)
        .min_size(600.0, 460.0)
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
