//! Background Apps — GNOME's last row as a round button: how many apps run
//! without a window, opening a page that lists them, each with a button to
//! close it.

use amane::{ScrollArea, Service, Size, Widget};

use super::super::{Page, open};
use crate::components::buttons::{IconButton, Style};
use crate::components::control_size::ControlSize;
use crate::components::icons::{ICON_BACKGROUND_APPS, ICON_CLOSE};
use crate::components::layout::{boxed, caption};
use crate::components::list::{self, List, ListItem};
use crate::components::material::{self, Level};
use crate::surfaces::quick_settings::detail::DetailPage;
use crate::surfaces::quick_settings::tile::QuickToggle;
use crate::services::desktop::background_apps::BackgroundApps;
use crate::tokens::sys;

fn count(n: usize) -> String {
    match n {
        1 => String::from("1 Background App"),
        n => format!("{n} Background Apps"),
    }
}

/// "org.signal.Signal" → "Signal".
fn name(app_id: &str) -> &str {
    app_id.rsplit('.').next().unwrap_or(app_id)
}

pub(crate) fn toggle() -> Option<QuickToggle> {
    let n = BackgroundApps::read().apps().len();
    if n == 0 {
        return None;
    }

    Some(
        QuickToggle::new("amane.qs.background", ICON_BACKGROUND_APPS, count(n))
            .subtitle("Running without a window")
            .on_open(|| open(Page::BackgroundApps)),
    )
}

pub(crate) fn page() -> DetailPage {
    let apps = BackgroundApps::read().apps().to_vec();
    let n = apps.len();

    DetailPage::new("amane.qs.background.page", "Background Apps")
        .subtitle(count(n))
        .body(move |height| {
            let items: Vec<ListItem> = apps
                .iter()
                .map(|app| {
                    let id = app.app_id.clone();
                    let mut item =
                        ListItem::new(format!("amane.qs.background.{id}"), name(&app.app_id))
                            .leading(ICON_BACKGROUND_APPS);
                    if !app.message.is_empty() {
                        item = item.trailing(list::trailing_text(
                            app.message.clone(),
                            material::label(Level::Secondary),
                        ));
                    }
                    item.trailing(
                        IconButton::new(format!("amane.qs.background.{id}.close"), ICON_CLOSE)
                            .style(Style::Tonal)
                            .size(sys::size::icon::s())
                            .height(ControlSize::Medium.height())
                            .width(ControlSize::Medium.height())
                            .on_press(move || BackgroundApps::quit(&id))
                            .build(),
                    )
                })
                .collect();

            let body: Vec<Box<dyn Widget>> = if items.is_empty() {
                vec![boxed(caption("No background apps"))]
            } else {
                vec![boxed(
                    ScrollArea::new(
                        "amane.qs.background.list",
                        List::new(items).header("Running without a window").build(),
                    )
                    .height(Size::Fixed(height)),
                )]
            };
            body
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_app_by_its_id() {
        assert_eq!(name("org.signal.Signal"), "Signal");
        assert_eq!(name("steam"), "steam");
        assert_eq!(count(1), "1 Background App");
        assert_eq!(count(3), "3 Background Apps");
    }
}
