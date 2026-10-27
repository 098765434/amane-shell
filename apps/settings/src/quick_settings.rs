//! Quick Settings: its layout is the shell's own, moved and sized in the
//! shell's Customize window, so this page asks the running shell over IPC
//! (`quick-settings controls | show | hide | customize | reset-layout`)
//! rather than writing a settings file. It watches the shell's layout file
//! to keep its switches in step with what Customize does.

use std::fs;
use std::path::PathBuf;

use amane::{Column, Row, Service, ipc_call, watch_file};
use amane_kit::app::Dialog;
use amane_kit::buttons::{PushButton, Switch};
use amane_kit::layout::boxed;
use amane_kit::tokens::sys;

use crate::{labelled, note, setting};

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
        return Column::new(vec![
            boxed(note(
                "The shell isn't answering, so its Quick Settings can't be changed from here.",
            )),
            boxed(
                PushButton::new("settings.quick.retry", "Try Again")
                    .on_press(|| QuickControls::write().controls = ask())
                    .build(),
            ),
        ])
        .gap(sys::space::gap::m());
    };

    let layout = Row::new(vec![
        boxed(
            PushButton::new("settings.quick.customize", "Customize…")
                .on_press(|| tell(&["customize"]))
                .build(),
        ),
        boxed(
            PushButton::new("settings.quick.reset", "Reset Layout…")
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
                .build(),
        ),
    ])
    .gap(sys::space::gap::m());

    let switches: Vec<_> = controls
        .into_iter()
        .map(|(name, shown)| {
            let label = named(&name).to_string();
            boxed(labelled(
                &label,
                Switch::new(format!("settings.quick.control.{name}"), shown)
                    .on_toggle(move |on| tell(&[if on { "show" } else { "hide" }, &name]))
                    .build(),
            ))
        })
        .collect();

    Column::new(vec![
        boxed(setting(
            "Layout",
            Column::new(vec![
                boxed(layout),
                boxed(note(
                    "Customize opens Quick Settings to move and resize its controls.",
                )),
            ])
            .gap(sys::space::gap::s()),
        )),
        boxed(setting(
            "Controls",
            Column::new(switches).gap(sys::space::gap::s()),
        )),
    ])
    .gap(sys::space::gap::l())
}
