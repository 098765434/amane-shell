//! The shell's launcher: which sources its search draws on, how
//! Applications lays the apps out, its clipboard history, and the quick
//! keys set for its actions.
//! Each control changes `launcher.json`, which the shell follows at once.

use amane::Column;
use emaki_kit::app::Dialog;
use emaki_kit::buttons::PushButton;
use emaki_kit::form::FormRow;
use emaki_kit::settings::{AppsLayout, change_launcher, launcher};
use emaki_kit::style;
use emaki_kit::thumbnail_picker::ThumbnailPicker;

use crate::{group, groups, previews, switch_row};

/// The sources the search draws on, by the ids the launcher knows them by.
const PROVIDERS: [(&str, &str); 7] = [
    ("apps", "Applications"),
    ("phone-apps", "Phone apps"),
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
    let mut layout_picker = ThumbnailPicker::new(
        "settings.launcher.layout",
        previews::WIDTH,
        previews::HEIGHT,
    )
    .width(crate::page_width() - 2.0 * style::row_pad_h())
    .chosen(layouts.iter().position(|(_, choice)| *choice == layout))
    .on_select(move |index| {
        let choice = layouts[index].1;
        change_launcher(|now| now.apps_layout = choice);
    });
    for (label, choice) in layouts {
        layout_picker = layout_picker.option(previews::launcher(choice == AppsLayout::Grid), label);
    }
    let layout_height = layout_picker.height();

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
                PushButton::new("settings.launcher.keys.reset", "Remove All…")
                    .on_press(|| {
                        Dialog::new("settings.launcher.keys.dialog", "Remove every quick key?")
                            .message("Actions keep working from the launcher; only their keys go.")
                            .button("Cancel", || {})
                            .destructive("Remove All", || {
                                change_launcher(|now| now.quick_keys.clear())
                            })
                            .open("main");
                    })
                    .build(),
            ),
        )
    };

    groups(vec![
        group().header("Search").rows(sources),
        group().row(
            FormRow::new("settings.launcher.layout", "Show applications as")
                .detail(layout_picker.build(), layout_height),
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
                .child()
                .disabled(!clipboard.enabled),
            )
            .footer(clipboard_note(clipboard.max_items, &clipboard.exclude_apps)),
        keys,
    ])
}

/// The apps whose copies are left out, by name: an app id's last part
/// ("org.keepassxc.KeePassXC" is KeePassXC), each once whatever its case.
fn app_names(ids: &[String]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for id in ids {
        let name = id.rsplit('.').next().unwrap_or(id).trim();
        if name.is_empty() {
            continue;
        }
        match names
            .iter_mut()
            .find(|known| known.eq_ignore_ascii_case(name))
        {
            // the one with capitals reads better
            Some(known) if known.chars().all(|c| !c.is_uppercase()) => *known = name.to_string(),
            Some(_) => {}
            None => names.push(name.to_string()),
        }
    }
    names
}

fn clipboard_note(max_items: usize, excluded: &[String]) -> String {
    let names = app_names(excluded);
    let left_out = match names.as_slice() {
        [] => String::from("Copies from password managers are left out."),
        [only] => format!("Copies from password managers like {only} are left out."),
        [rest @ .., last] => format!(
            "Copies from password managers like {} and {last} are left out.",
            rest.join(", ")
        ),
    };
    format!("Keeps the last {max_items} copies. {left_out}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_out_apps_read_by_name_once() {
        let ids: Vec<String> = [
            "org.keepassxc.KeePassXC",
            "keepassxc",
            "bitwarden",
            "Bitwarden",
            "1password",
            "1Password",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(app_names(&ids), ["KeePassXC", "Bitwarden", "1Password"]);
        assert_eq!(
            clipboard_note(200, &ids),
            "Keeps the last 200 copies. Copies from password managers like KeePassXC, Bitwarden \
             and 1Password are left out."
        );
    }
}
