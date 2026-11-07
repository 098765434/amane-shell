//! Quick Settings: its layout is the shell's own, moved and sized in the
//! shell's Customize window, so this page asks the running shell over IPC
//! (`quick-settings controls | show | hide | customize | reset-layout`)
//! rather than writing a settings file. It watches the shell's layout file
//! to keep its switches in step with what Customize does.

use std::fs;
use std::path::PathBuf;

use amane::{Column, Service, watch_file};
use amane_kit::app::Dialog;
use amane_kit::buttons::PushButton;
use amane_kit::color::Accent;
use amane_kit::form::FormRow;
use amane_kit::icons::{Glyph, ICON_TUNE};
use amane_kit::settings::QS_CONTROLS;
use amane_kit::shell_ipc::{QuickSettings as Ask, ShellCommand};

use crate::{group, groups, switch_row};

/// The controls this machine has and whether each is on the canvas, as the
/// running shell says; none while the shell doesn't answer.
struct QuickControls {
    controls: Option<Vec<(String, bool)>>,
    /// The shell has been asked once.
    asked: bool,
}

impl Service for QuickControls {
    // asked from listen(), so opening the page never waits on the shell
    fn new() -> Self {
        Self {
            controls: None,
            asked: false,
        }
    }

    // the shell saves the layout on every change, from here or Customize
    fn listen() {
        let path = layout_file();

        // the folder is watched, so it has to be there
        if let Some(folder) = path.parent() {
            // watching a folder that couldn't be made just hears nothing
            let _ = fs::create_dir_all(folder);
        }

        {
            let controls = ask();
            let mut now = Self::write();
            now.controls = controls;
            now.asked = true;
        }

        for _ in watch_file(&path.to_string_lossy()) {
            let controls = ask();
            let mut now = Self::write();

            if now.controls == controls {
                now.quiet();

                continue;
            }

            now.controls = controls;
        }
    }
}

/// Where the shell keeps the layout: `$XDG_STATE_HOME/amane-shell/
/// quick-settings.json`.
fn layout_file() -> PathBuf {
    let state = amane_kit::paths::state();

    state
        .join(amane_kit::app_ids::SHELL)
        .join("quick-settings.json")
}

fn ask() -> Option<Vec<(String, bool)>> {
    let answer = ShellCommand::QuickSettings(Ask::Controls).send().ok()?;

    Some(
        answer
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(name, shown)| (String::from(name), shown == "shown"))
            .collect(),
    )
}

/// Asks the shell to run `args`, then reads the controls again: a change
/// that leaves the file as it was (or a shell that just started) still
/// shows.
fn tell(asked: Ask) {
    std::thread::spawn(move || {
        if let Err(error) = ShellCommand::QuickSettings(asked).send() {
            log::warn!("quick-settings: {error}");
        }
        let controls = ask();
        QuickControls::write().controls = controls;
    });
}

/// A control's label and leading tile, by the shell's name for it; one the
/// table doesn't know shows under its own name on a grey tile.
fn named(name: &str) -> (&str, Glyph, Accent) {
    QS_CONTROLS.iter().find(|(id, ..)| *id == name).map_or(
        (name, Glyph::Font(ICON_TUNE), Accent::Gray),
        |(_, label, glyph, accent)| (label, *glyph, *accent),
    )
}

pub fn page() -> Column {
    let (controls, asked) = {
        let now = QuickControls::read();
        (now.controls.clone(), now.asked)
    };
    if !asked {
        return groups(vec![
            group().row(FormRow::new("settings.quick.asking", "Asking the shell…")),
        ]);
    }
    let Some(controls) = controls else {
        return groups(vec![
            group().row(
                FormRow::new("settings.quick.retry-row", "The shell isn't answering")
                    .icon(
                        amane_kit::icons::ICON_WARNING,
                        amane_kit::color::Accent::Orange,
                    )
                    .subtitle("Its Quick Settings can't be changed from here until it does")
                    .trailing(
                        PushButton::new("settings.quick.retry", "Try Again")
                            .on_press(|| {
                                std::thread::spawn(|| {
                                    let controls = ask();
                                    QuickControls::write().controls = controls;
                                });
                            })
                            .build(),
                    ),
            ),
        ]);
    };

    let reset = PushButton::new("settings.quick.reset", "Reset…")
        .on_press(|| {
            Dialog::new(
                "settings.quick.reset-dialog",
                "Reset the Quick Settings layout?",
            )
            .message("Every control goes back to its place and size as shipped.")
            .button("Cancel", || {})
            .destructive("Reset", || tell(Ask::ResetLayout))
            .open("main");
        })
        .build();

    let switches = controls.into_iter().map(|(name, shown)| {
        let (label, glyph, accent) = named(&name);
        let label = label.to_string();
        switch_row(
            &format!("settings.quick.control.{name}"),
            &label,
            shown,
            move |on| {
                tell(if on {
                    Ask::Show(name.clone())
                } else {
                    Ask::Hide(name.clone())
                })
            },
        )
        .icon(glyph, accent)
    });

    groups(vec![
        group()
            .header("Layout")
            .footer(
                "Customize… in the toolbar opens Quick Settings to move and resize its controls.",
            )
            .row(
                FormRow::new("settings.quick.reset-row", "Reset layout")
                    .subtitle("Every control back to its place and size as shipped")
                    .trailing(reset),
            ),
        group().header("Controls").rows(switches),
    ])
}

/// Customize…, in the toolbar while the shell answers.
pub fn toolbar() -> Option<Box<dyn amane::Widget>> {
    QuickControls::read().controls.as_ref()?;
    Some(Box::new(
        PushButton::new("settings.quick.customize", "Customize…")
            .on_press(|| tell(Ask::Customize))
            .build(),
    ))
}
