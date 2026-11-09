//! The widgets on the desktop, for the Desktop page: each monitor's, with
//! Remove, and Remove All. The layout is the shell's own (Edit Widgets
//! arranges it), so this asks the running shell over IPC (`widgets list
//! --json | remove | reset-layout`) and watches its state file
//! (`widgets.json`) to keep in step with what Edit Widgets does.

use std::fs;
use std::path::PathBuf;

use amane::{Service, watch_file};
use emaki_kit::app::Dialog;
use emaki_kit::buttons::PushButton;
use emaki_kit::color::Accent;
use emaki_kit::form::{FormRow, Group};
use emaki_kit::icons::Glyph;
use emaki_kit::shell_ipc::{ShellCommand, Widgets};
use serde::Deserialize;

use crate::group;

/// One widget as the shell lists it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct Listed {
    output: String,
    id: u32,
    title: String,
    #[serde(default)]
    glyph: String,
    #[serde(default)]
    category: String,
    size: String,
    /// Its cell, or none while the screen has no room for it.
    at: Option<[u8; 2]>,
}

/// The widgets the running shell has; none while it doesn't answer.
struct DesktopWidgets {
    widgets: Option<Vec<Listed>>,
}

impl Service for DesktopWidgets {
    // asked from listen(), so opening the page never waits on the shell
    fn new() -> Self {
        Self { widgets: None }
    }

    // the shell saves the layout on every change, from here or Edit Widgets
    fn listen() {
        let path = layout_file();
        if let Some(folder) = path.parent() {
            // watching a folder that couldn't be made just hears nothing
            let _ = fs::create_dir_all(folder);
        }
        let widgets = ask();
        Self::write().widgets = widgets;
        for _ in watch_file(&path.to_string_lossy()) {
            let widgets = ask();
            let mut now = Self::write();
            if now.widgets == widgets {
                now.quiet();
                continue;
            }
            now.widgets = widgets;
        }
    }
}

/// Where the shell keeps the layout: `$XDG_STATE_HOME/emaki/widgets.json`.
fn layout_file() -> PathBuf {
    emaki_kit::paths::state()
        .join(emaki_kit::app_ids::SHELL)
        .join("widgets.json")
}

fn ask() -> Option<Vec<Listed>> {
    let answer = ShellCommand::Widgets(Widgets::ListJson).send().ok()?;
    serde_json::from_str(&answer).ok()
}

/// Asks the shell to run `command`, then lists the widgets again, so a
/// change that leaves the file as it was still shows.
fn tell(command: Widgets) {
    std::thread::spawn(move || {
        if let Err(error) = ShellCommand::Widgets(command).send() {
            log::warn!("widgets: {error}");
        }
        let widgets = ask();
        DesktopWidgets::write().widgets = widgets;
    });
}

/// "extra-large" as a person reads it: "Extra Large".
fn size_title(size: &str) -> String {
    size.split('-')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A kind's tile colour, by its gallery category.
fn accent(category: &str) -> Accent {
    match category {
        "clock" => Accent::Orange,
        "battery" => Accent::Green,
        _ => Accent::Blue,
    }
}

/// The groups listing what's on each monitor's desktop, after the page's
/// own; one saying so while the shell doesn't answer.
pub fn groups(enabled: bool) -> Vec<Group> {
    let Some(widgets) = DesktopWidgets::read().widgets.clone() else {
        return vec![
            group().header("On the Desktop").row(
                FormRow::new("settings.widgets.unreachable", "The shell isn't answering")
                    .subtitle("The widgets on the desktop show here while it runs"),
            ),
        ];
    };
    let mut outputs: Vec<String> = widgets.iter().map(|w| w.output.clone()).collect();
    outputs.dedup();
    if widgets.is_empty() {
        return vec![
            group().header("On the Desktop").row(
                FormRow::new("settings.widgets.none", "No widgets")
                    .subtitle("Edit Widgets… adds them from the gallery"),
            ),
        ];
    }
    let several = outputs.len() > 1;
    outputs
        .into_iter()
        .map(|output| {
            let header = if several {
                format!("On {output}")
            } else {
                String::from("On the Desktop")
            };
            let rows = widgets.iter().filter(|w| w.output == output).map(|widget| {
                let id = widget.id;
                let remove = PushButton::new(format!("settings.widgets.{id}.remove"), "Remove")
                    .disabled(!enabled)
                    .on_press(move || tell(Widgets::Remove(id)))
                    .build();
                let mut subtitle = size_title(&widget.size);
                if widget.at.is_none() {
                    subtitle.push_str(" · No room on this screen");
                }
                let mut row = FormRow::new(format!("settings.widgets.{id}"), &widget.title)
                    .subtitle(subtitle)
                    .trailing(remove);
                if let Some(glyph) = widget.glyph.chars().next() {
                    row = row.icon(Glyph::Font(glyph), accent(&widget.category));
                }
                row.disabled(!enabled)
            });
            let on = output.clone();
            let remove_all = PushButton::new(
                format!("settings.widgets.{output}.remove-all"),
                "Remove All…",
            )
            .disabled(!enabled)
            .on_press(move || {
                let output = on.clone();
                Dialog::new(
                    "settings.widgets.remove-all-dialog",
                    format!("Remove every widget from {output}?"),
                )
                .message("Their places and settings go too. Edit Widgets… adds new ones.")
                .button("Cancel", || {})
                .destructive("Remove All", move || {
                    tell(Widgets::ResetLayout(Some(output.clone())))
                })
                .open("main");
            })
            .build();
            group().header(&header).rows(rows).row(
                FormRow::new(
                    format!("settings.widgets.{output}.remove-all-row"),
                    "Remove all widgets",
                )
                .trailing(remove_all)
                .disabled(!enabled),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shells_list_reads() {
        let json = r#"[{"output":"eDP-1","id":3,"kind":"cpu","title":"CPU","glyph":"",
            "category":"system","size":"extra-large","at":null,"settings":{"range":"15"}}]"#;
        let listed: Vec<Listed> = serde_json::from_str(json).unwrap();
        assert_eq!(listed[0].id, 3);
        assert_eq!(listed[0].at, None);
        assert_eq!(size_title(&listed[0].size), "Extra Large");
        assert_eq!(size_title("small"), "Small");
    }
}
