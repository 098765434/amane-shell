//! Quick Settings: its layout is the shell's own, moved and sized in the
//! shell's Customize window, so this page asks the running shell over IPC
//! (`quick-settings controls | show | hide | customize | reset-layout`)
//! rather than writing a settings file. It watches the shell's layout file
//! to keep its switches in step with what Customize does.

use std::fs;
use std::path::PathBuf;

use amane::{Column, Service, ipc_call, watch_file};
use amane_kit::app::Dialog;
use amane_kit::buttons::PushButton;
use amane_kit::form::FormRow;

use crate::{group, groups, switch_row};

/// The shell's app id, whose IPC socket this page calls.
const SHELL: &str = "amane";

/// Each control's name, by the shell's name for it.
const NAMES: [(&str, &str); 20] = [
    ("wifi", "Wi-Fi"),
    ("bluetooth", "Bluetooth"),
    ("now-playing", "Now Playing"),
    ("vpn", "VPN"),
    ("wired", "Wired network"),
    ("focus", "Do Not Disturb"),
    ("dark-style", "Dark Style"),
    ("night-light", "Night Light"),
    ("display", "Display brightness"),
    ("sound", "Sound"),
    ("battery", "Battery"),
    ("power-mode", "Power Mode"),
    ("keyboard", "Keyboard brightness"),
    ("airplane", "Airplane Mode"),
    ("background-apps", "Background apps"),
    ("screenshot", "Screenshot"),
    ("lock", "Lock"),
    ("power-off", "Power"),
    ("tailscale", "Tailscale"),
    ("keep-awake", "Keep Awake"),
];

/// The controls this machine has and whether each is on the canvas, as the
/// running shell says; none while the shell doesn't answer.
struct QuickControls {
    controls: Option<Vec<(String, bool)>>,
}

impl Service for QuickControls {
    fn new() -> Self {
        Self { controls: ask() }
    }

    // the shell saves the layout on every change, from here or Customize
    fn listen() {
        let path = layout_file();

        // the folder is watched, so it has to be there
        if let Some(folder) = path.parent() {
            let _ = fs::create_dir_all(folder);
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
/// quick-settings`.
fn layout_file() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_default();

    state.join("amane-shell").join("quick-settings")
}

fn ask() -> Option<Vec<(String, bool)>> {
    let answer = ipc_call(SHELL, "quick-settings", &[String::from("controls")]).ok()?;

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
fn tell(args: &[&str]) {
    let args: Vec<String> = args.iter().map(|arg| String::from(*arg)).collect();
    let _ = ipc_call(SHELL, "quick-settings", &args);
    QuickControls::write().controls = ask();
}

fn named(name: &str) -> &str {
    NAMES
        .iter()
        .find(|(id, _)| *id == name)
        .map_or(name, |(_, label)| label)
}

pub fn page() -> Column {
    let Some(controls) = QuickControls::read().controls.clone() else {
        return groups(vec![
            group().row(
                FormRow::new("settings.quick.retry-row", "The shell isn't answering")
                    .subtitle("Its Quick Settings can't be changed from here until it does")
                    .trailing(
                        PushButton::new("settings.quick.retry", "Try Again")
                            .on_press(|| QuickControls::write().controls = ask())
                            .build(),
                    ),
            ),
        ]);
    };

    let customize = PushButton::new("settings.quick.customize", "Customize…")
        .on_press(|| tell(&["customize"]))
        .build();
    let reset = PushButton::new("settings.quick.reset", "Reset…")
        .on_press(|| {
            Dialog::new(
                "settings.quick.reset-dialog",
                "Reset the Quick Settings layout?",
            )
            .message("Every control goes back to its place and size as shipped.")
            .button("Cancel", || {})
            .destructive("Reset", || tell(&["reset-layout"]))
            .open("main");
        })
        .build();

    let switches = controls.into_iter().map(|(name, shown)| {
        let label = named(&name).to_string();
        switch_row(
            &format!("settings.quick.control.{name}"),
            &label,
            shown,
            move |on| tell(&[if on { "show" } else { "hide" }, &name]),
        )
    });

    groups(vec![
        group()
            .header("Layout")
            .row(
                FormRow::new("settings.quick.customize-row", "Customize")
                    .subtitle("Opens Quick Settings to move and resize its controls")
                    .trailing(customize),
            )
            .row(
                FormRow::new("settings.quick.reset-row", "Reset layout")
                    .subtitle("Every control back to its place and size as shipped")
                    .trailing(reset),
            ),
        group().header("Controls").rows(switches),
    ])
}
