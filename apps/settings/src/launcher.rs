//! Spotlight, the shell's launcher: which sources its search draws on, how
//! Applications lays the apps out, its clipboard history, and the quick
//! keys set for its actions.
//! Each control changes `launcher.json`, which the shell follows at once.

use amane::{Column, Widget};
use amane_kit::buttons::{PushButton, Radio};
use amane_kit::form::FormRow;
use amane_kit::layout::boxed;
use amane_kit::settings::{AppsLayout, change_launcher, launcher};

use crate::{group, groups, radios_height, switch_row};

/// The sources the search draws on, by the ids the launcher knows them by.
const PROVIDERS: [(&str, &str); 6] = [
    ("apps", "Applications"),
    ("actions", "Actions"),
    ("files", "Files"),
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

    let sources = PROVIDERS.into_iter().map(|(id, name)| {
        let on = !excluded.iter().any(|left_out| left_out == id);
        switch_row(
            &format!("settings.launcher.provider.{id}"),
            name,
            on,
            move |on| {
                change_launcher(|now| {
                    now.excluded_providers.retain(|left_out| left_out != id);
                    if !on {
                        now.excluded_providers.push(id.to_string());
                    }
                })
            },
        )
    });

    let mut keys = group().header("Quick keys");
    for (combo, action) in &quick_keys {
        let combo = combo.clone();
        let remove = combo.clone();
        keys = keys.row(
            FormRow::new(
                format!("settings.launcher.key-row.{combo}"),
                action.title.clone(),
            )
            .subtitle(combo.to_uppercase())
            .trailing(
                PushButton::new(format!("settings.launcher.key.{combo}"), "Remove")
                    .on_press(move || {
                        change_launcher(|now| {
                            now.quick_keys.remove(&remove);
                        })
                    })
                    .build(),
            ),
        );
    }
    let keys = if quick_keys.is_empty() {
        keys.row(
            FormRow::new("settings.launcher.keys.none", "None yet")
                .subtitle("Set them from Actions in the launcher"),
        )
    } else {
        keys.row(
            FormRow::new("settings.launcher.keys.reset-row", "Remove every quick key").trailing(
                PushButton::new("settings.launcher.keys.reset", "Reset All")
                    .on_press(|| change_launcher(|now| now.quick_keys.clear()))
                    .build(),
            ),
        )
    };

    groups(vec![
        group().header("Search").rows(sources),
        group().row(
            FormRow::new("settings.launcher.layout", "Show applications as")
                .detail(Column::new(layout_radios), radios_height(2)),
        ),
        group()
            .header("Clipboard history")
            .row(switch_row(
                "settings.launcher.clipboard",
                "Keep what's copied, for Clipboard",
                clipboard.enabled,
                |on| change_launcher(|now| now.clipboard.enabled = on),
            ))
            .row(
                switch_row(
                    "settings.launcher.clipboard.images",
                    "Keep copied images too",
                    clipboard.images,
                    |on| change_launcher(|now| now.clipboard.images = on),
                )
                .disabled(!clipboard.enabled),
            )
            .footer(format!(
                "Keeps the last {} copies. Copies from password managers, and from {}, are left out.",
                clipboard.max_items,
                clipboard.exclude_apps.join(", ")
            )),
        keys,
    ])
}
