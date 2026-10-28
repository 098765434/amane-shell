//! A change tried before it's kept: niri makes it at once, a dialog asks
//! whether to keep it, and nobody answering in [`SECONDS`] (a screen left
//! black or unreadable) puts the old setting back. Only a kept change is
//! written to `displays.kdl`.

use std::thread;

use amane_kit::app::Dialog;

use super::config::OutputConfig;
use super::niri::{self, Change, Output};
use super::{Screens, save};

/// How long the dialog waits before going back.
pub const SECONDS: u32 = 15;

/// Tries `change` on `output`, `undo` putting it back; keeping it saves
/// `keep` made to its block, `what` naming it in the commit.
pub fn try_change(
    output: &Output,
    change: Change,
    undo: Change,
    what: String,
    keep: impl Fn(&mut OutputConfig) + Clone + Send + 'static,
) {
    let name = output.name.clone();
    let revert = {
        let name = name.clone();
        move || {
            let (name, undo) = (name.clone(), undo.clone());
            thread::spawn(move || {
                if let Err(error) = niri::apply(&name, &undo) {
                    Screens::fail(format!("Couldn't go back: {error}"));
                }
                Screens::refresh();
            });
        }
    };
    let kept = {
        let output = output.clone();
        move || save(&output, what.clone(), keep.clone())
    };

    Dialog::new("settings.displays.keep", "Keep these display settings?")
        .message("If the screen looks wrong, wait and it goes back by itself.")
        .button("Revert", revert.clone())
        .main_button("Keep Changes", kept)
        .countdown(SECONDS, revert)
        .open("main");

    thread::spawn(move || {
        if let Err(error) = niri::apply(&name, &change) {
            Screens::fail(error);
        }
        Screens::refresh();
    });
}
