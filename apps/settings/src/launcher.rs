//! Spotlight, the shell's launcher: which sources its search draws on, how
//! Applications lays the apps out, its clipboard history, and the quick
//! keys set for its actions.
//! Each control changes `launcher.json`, which the shell follows at once.

use amane::{Center, Column, Row, Text, Widget};
use amane_kit::buttons::{PushButton, Radio, Switch};
use amane_kit::layout::boxed;
use amane_kit::material::{self, Level};
use amane_kit::settings::{AppsLayout, change_launcher, launcher};
use amane_kit::tokens::{Styled, sys};

use crate::setting;

/// The sources the search draws on, by the ids the launcher knows them by.
const PROVIDERS: [(&str, &str); 5] = [
    ("apps", "Applications"),
    ("actions", "Actions"),
    ("answers", "Calculations and conversions"),
    ("url", "Web addresses"),
    ("web", "Web search"),
];

pub fn page() -> Column {
    let (excluded, layout, quick_keys, clipboard) = {
        let now = launcher();
        (
            now.excluded_providers.clone(),
            now.apps_layout,
            now.quick_keys.clone(),
            now.clipboard.clone(),
        )
    };

    let history = Column::new(vec![
        boxed(labelled(
            "Keep what's copied, for Clipboard",
            Switch::new("settings.launcher.clipboard", clipboard.enabled)
                .on_toggle(|on| change_launcher(|now| now.clipboard.enabled = on))
                .build(),
        )),
        boxed(labelled(
            "Keep copied images too",
            Switch::new("settings.launcher.clipboard.images", clipboard.images)
                .on_toggle(|on| change_launcher(|now| now.clipboard.images = on))
                .build(),
        )),
        boxed(
            Text::new(format!(
                "Keeps the last {} copies. Copies from password managers, and from {}, are left out.",
                clipboard.max_items,
                clipboard.exclude_apps.join(", ")
            ))
            .style(sys::r#type::caption())
            .color(material::label(Level::Secondary)),
        ),
    ])
    .gap(sys::space::gap::s());

    let sources: Vec<Box<dyn Widget>> = PROVIDERS
        .into_iter()
        .map(|(id, name)| {
            let on = !excluded.iter().any(|left_out| left_out == id);
            boxed(labelled(
                name,
                Switch::new(format!("settings.launcher.provider.{id}"), on)
                    .on_toggle(move |on| {
                        change_launcher(|now| {
                            now.excluded_providers.retain(|left_out| left_out != id);
                            if !on {
                                now.excluded_providers.push(id.to_string());
                            }
                        })
                    })
                    .build(),
            ))
        })
        .collect();

    let layouts = [("Grid", AppsLayout::Grid), ("List", AppsLayout::List)];
    let layout_radios: Vec<Box<dyn Widget>> = layouts
        .into_iter()
        .enumerate()
        .map(|(index, (label, choice))| {
            boxed(
                Radio::new(
                    format!("settings.launcher.layout.{index}"),
                    layout == choice,
                )
                .label(label)
                .on_select(move || change_launcher(|now| now.apps_layout = choice))
                .build(),
            )
        })
        .collect();

    let mut keys: Vec<Box<dyn Widget>> = quick_keys
        .iter()
        .map(|(keys, action)| {
            let keys = keys.clone();
            boxed(labelled(
                &format!("{}  ·  {}", keys.to_uppercase(), action.title),
                PushButton::new(format!("settings.launcher.key.{keys}"), "Remove")
                    .on_press(move || {
                        change_launcher(|now| {
                            now.quick_keys.remove(&keys);
                        })
                    })
                    .build(),
            ))
        })
        .collect();
    if keys.is_empty() {
        keys.push(boxed(
            Text::new("None yet: set them from Actions in the launcher.")
                .style(sys::r#type::label())
                .color(material::label(Level::Secondary)),
        ));
    } else {
        keys.push(boxed(
            PushButton::new("settings.launcher.keys.reset", "Reset All Quick Keys")
                .on_press(|| change_launcher(|now| now.quick_keys.clear()))
                .build(),
        ));
    }

    Column::new(vec![
        boxed(setting(
            "Search",
            Column::new(sources).gap(sys::space::gap::s()),
        )),
        boxed(setting("Show applications as", Column::new(layout_radios))),
        boxed(setting("Clipboard history", history)),
        boxed(setting(
            "Quick keys",
            Column::new(keys).gap(sys::space::gap::s()),
        )),
    ])
    .gap(sys::space::gap::l())
}

/// `name` beside `control`, on one line.
fn labelled(name: &str, control: impl Widget + 'static) -> Row {
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
