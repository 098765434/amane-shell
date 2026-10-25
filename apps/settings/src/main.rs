//! Shell Settings: changes amane-shell's settings file, which the running
//! shell follows at once. One copy runs; launching it again brings it up.

mod about;
mod appearance;

use amane::{Column, Row, Service, Size, Text, Widget, Window};
use amane_kit::app::{Page, app, app_window, sidebar, standard_shortcut};
use amane_kit::icons::{ICON_COMPUTER, ICON_DARK_MODE};
use amane_kit::layout::boxed;
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

fn pages() -> [Page; 2] {
    [
        Page::new(ICON_DARK_MODE, "Appearance"),
        Page::new(ICON_COMPUTER, "About"),
    ]
}

fn main_window() -> Window {
    let page = Navigation::read().page;

    let pages = pages();

    let nav = sidebar("settings.nav", &pages, page, |page| {
        Navigation::write().page = page;
    });

    let (title, body): (&str, Box<dyn Widget>) = match page {
        0 => ("Appearance", boxed(appearance::page())),
        _ => ("About", boxed(about::page())),
    };

    let content = Row::new(vec![
        boxed(Column::new(vec![boxed(nav)]).width(Size::Fixed(200.0))),
        boxed(
            Column::new(vec![
                boxed(
                    Text::new(title)
                        .style(sys::r#type::title())
                        .color(sys::color::surface::popover::fg()),
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

/// A setting's name over its control, the way every page lays them out.
pub fn setting(name: &str, control: impl Widget + 'static) -> Column {
    Column::new(vec![
        boxed(
            Text::new(name)
                .style(sys::r#type::caption())
                .color(sys::color::content::secondary()),
        ),
        boxed(control),
    ])
    .gap(sys::space::gap::xs())
}
