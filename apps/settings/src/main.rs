//! Shell Settings: changes amane-shell's settings file, which the running
//! shell follows at once. One copy runs; launching it again brings it up.

mod about;
mod appearance;
mod desktop;
mod dock;
mod launcher;
mod quick_settings;
mod status_bar;

use amane::{Center, Column, Row, ScrollArea, Service, Size, Text, Widget, Window};
use amane_kit::app::{Page, app, app_window, sidebar, standard_shortcut};
use amane_kit::icons::{
    ICON_APPS, ICON_COMPUTER, ICON_DARK_MODE, ICON_LAYERS, ICON_MORE_HORIZ, ICON_SEARCH, ICON_TUNE,
};
use amane_kit::layout::boxed;
use amane_kit::material::{self, Level};
use amane_kit::tokens::{Styled, sys};

// which page shows, kept while the app runs
struct Navigation {
    page: usize,
}

impl Service for Navigation {
    fn new() -> Self {
        Self { page: 0 }
    }

    // only clicks change it
    fn listen() {}
}

fn main() {
    app("amane-settings")
        .normal_window("main", main_window)
        .run();
}

fn pages() -> [Page; 7] {
    [
        Page::new(ICON_DARK_MODE, "Appearance"),
        Page::new(ICON_APPS, "Desktop"),
        Page::new(ICON_MORE_HORIZ, "Status Bar"),
        Page::new(ICON_LAYERS, "Dock"),
        Page::new(ICON_TUNE, "Quick Settings"),
        Page::new(ICON_SEARCH, "Spotlight"),
        Page::new(ICON_COMPUTER, "About"),
    ]
}

fn main_window() -> Window {
    let page = Navigation::read().page;

    let pages = pages();

    let nav = sidebar("settings.nav", &pages, page, |page| {
        Navigation::write().page = page;
    });

    // each page keeps where it was scrolled to, under its own name
    let (title, scroll, body): (&str, &'static str, Box<dyn Widget>) = match page {
        0 => (
            "Appearance",
            "settings.scroll.appearance",
            boxed(appearance::page()),
        ),
        1 => ("Desktop", "settings.scroll.desktop", boxed(desktop::page())),
        2 => (
            "Status Bar",
            "settings.scroll.status-bar",
            boxed(status_bar::page()),
        ),
        3 => ("Dock", "settings.scroll.dock", boxed(dock::page())),
        4 => (
            "Quick Settings",
            "settings.scroll.quick-settings",
            boxed(quick_settings::page()),
        ),
        5 => (
            "Spotlight",
            "settings.scroll.spotlight",
            boxed(launcher::page()),
        ),
        _ => ("About", "settings.scroll.about", boxed(about::page())),
    };
    let body = boxed(
        ScrollArea::new(scroll, Column::new(vec![body]))
            .width(Size::Parent)
            .height(Size::Parent),
    );

    let content = Row::new(vec![
        boxed(
            Column::new(vec![boxed(nav)])
                .width(Size::Fixed(200.0))
                .height(Size::Parent),
        ),
        boxed(
            Column::new(vec![
                boxed(
                    Text::new(title)
                        .style(sys::r#type::title())
                        .color(material::label(Level::Primary)),
                ),
                body,
            ])
            .gap(sys::space::gap::l())
            .width(Size::Parent),
        ),
    ])
    .gap(sys::space::gap::xl())
    .width(Size::Parent)
    .height(Size::Parent);

    app_window(content)
        .title("Shell Settings")
        .size(760.0, 520.0)
        .min_size(600.0, 400.0)
        .on_shortcut(standard_shortcut)
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
