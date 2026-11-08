//! Pinned apps: an app kept in the dock on the workspace it was pinned on,
//! there to open whether or not it's running (on one workspace, not
//! every one).
//!
//! A pin knows its workspace by name when the workspace has one, which
//! niri keeps whatever its place, else by its place on the monitor (niri
//! renumbers unnamed workspaces as empty ones go). Kept in the state file
//! `dock-pins.json`; the older `dock-pins`, a pin a line
//! (`app_id<TAB>output<TAB>name:<name>` or `…<TAB>index:<n>`), is read when
//! that isn't there.

use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use amane::serde::{Deserialize, Serialize};
use amane::{Windows, Workspaces};

use crate::services::state::{self, State};
use emaki_kit::app_icon;

/// The file from before the state files, a pin a line.
const OLD_FILE: &str = "dock-pins";

/// How long after a pin opens its app a window of that app that turns up
/// on another workspace is still taken to be the one it opened.
const PLACING: Duration = Duration::from_secs(10);

/// Which workspace a pin is on.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Place {
    Named(String),
    /// Its place on the monitor, from 1.
    Index(u32),
}

/// One pinned app.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(crate = "amane::serde")]
pub struct Pin {
    pub app_id: String,
    /// The monitor it was pinned on, for a pin by index.
    pub output: String,
    pub place: Place,
}

impl Pin {
    /// The pin for `app_id` on the workspace named `name` (if it has one)
    /// or at `index` on `output`.
    pub fn new(app_id: &str, output: &str, name: Option<&str>, index: u32) -> Self {
        Self {
            app_id: app_id.to_string(),
            output: output.to_string(),
            place: match name {
                Some(name) => Place::Named(name.to_string()),
                None => Place::Index(index),
            },
        }
    }
}

/// Every pin, as kept.
#[derive(Default, Serialize, Deserialize)]
#[serde(crate = "amane::serde", transparent)]
struct Pins(Vec<Pin>);

impl State for Pins {
    const FILE: &'static str = "dock-pins.json";
    const VERSION: u32 = 1;

    fn legacy() -> Option<Self> {
        state::legacy_text(OLD_FILE).map(|text| Self(parse(&text)))
    }
}

thread_local! {
    static PINS: RefCell<Vec<Pin>> =
        RefCell::new(state::load::<Pins>().unwrap_or_default().0);

    // apps a pin opened on a workspace, and until when a window of theirs
    // turning up elsewhere is moved there
    static PLACING_ON: RefCell<Vec<(String, i64, Instant)>> = const { RefCell::new(Vec::new()) };
}

/// Every pin, in the order they were pinned.
pub fn all() -> Vec<Pin> {
    PINS.with_borrow(Clone::clone)
}

/// Pins `pin`, unless it's there already.
pub fn add(pin: Pin) {
    change(|pins| {
        if pins.contains(&pin) {
            return false;
        }
        pins.push(pin);
        true
    });
}

/// Takes `pin` out of the dock.
pub fn remove(pin: &Pin) {
    change(|pins| {
        let before = pins.len();
        pins.retain(|other| other != pin);
        pins.len() != before
    });
}

fn change(edit: impl FnOnce(&mut Vec<Pin>) -> bool) {
    let changed = PINS.with_borrow_mut(edit);
    if changed {
        PINS.with_borrow(|pins| state::save(&Pins(pins.clone())));
        super::Docks::changed();
    }
}

/// Opens `app_id` from its desktop entry on `workspace`, which comes into
/// view; false when it has no entry to open it by.
pub fn open(app_id: &str, workspace: i64) -> bool {
    open_with(app_id, workspace, &[])
}

/// Opens `files` with `app_id` on `workspace`, as [`open`] does.
pub fn open_with(app_id: &str, workspace: i64, files: &[PathBuf]) -> bool {
    let Some(app) = app_icon::entry_for(app_id) else {
        return false;
    };
    // all go through amane's one worker thread, in order: niri opens the
    // window on the workspace that has just been focused
    Workspaces::focus(workspace);
    for command in app.commands_with(files) {
        Windows::spawn(vec![String::from("sh"), String::from("-c"), command]);
    }
    // a slow app may map after the pointer has gone to another workspace
    placing(app_id, workspace);
    true
}

/// Remembers that `app_id` was just opened for workspace `workspace`.
fn placing(app_id: &str, workspace: i64) {
    PLACING_ON.with_borrow_mut(|placing| {
        placing.retain(|(app, _, _)| app != app_id);
        placing.push((app_id.to_string(), workspace, Instant::now() + PLACING));
    });
}

/// Where a new window of `app_id` belongs, if a pin opened it a moment
/// ago for another workspace; each opening places one window.
pub fn placed(app_id: &str) -> Option<i64> {
    PLACING_ON.with_borrow_mut(|placing| {
        let now = Instant::now();
        placing.retain(|(_, _, until)| *until > now);
        let at = placing.iter().position(|(app, _, _)| app == app_id)?;
        Some(placing.remove(at).1)
    })
}

fn parse(text: &str) -> Vec<Pin> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let app_id = fields.next()?.trim();
            let output = fields.next()?.trim();
            let place = fields.next()?.trim();
            let place = if let Some(name) = place.strip_prefix("name:") {
                Place::Named(name.to_string())
            } else {
                Place::Index(place.strip_prefix("index:")?.parse().ok()?)
            };
            (!app_id.is_empty()).then(|| Pin {
                app_id: app_id.to_string(),
                output: output.to_string(),
                place,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_file_a_pin_a_line_is_read() {
        let pins = vec![
            Pin::new("firefox", "eDP-1", None, 2),
            Pin::new("foot", "HDMI-A-1", Some("code"), 1),
        ];
        let text = "firefox\teDP-1\tindex:2\nfoot\tHDMI-A-1\tname:code\n";
        assert_eq!(parse(text), pins);
    }

    #[test]
    fn pins_are_kept_as_json() {
        let pins = Pins(vec![
            Pin::new("firefox", "eDP-1", None, 2),
            Pin::new("foot", "HDMI-A-1", Some("code"), 1),
        ]);
        let text = serde_json::to_string(&pins).unwrap();
        assert_eq!(
            text,
            r#"[{"app_id":"firefox","output":"eDP-1","place":{"index":2}},{"app_id":"foot","output":"HDMI-A-1","place":{"named":"code"}}]"#
        );
        let back: Pins = serde_json::from_str(&text).unwrap();
        assert_eq!(back.0, pins.0);
    }

    #[test]
    fn a_named_workspace_is_kept_by_name() {
        assert_eq!(
            Pin::new("foot", "eDP-1", Some("web"), 3).place,
            Place::Named(String::from("web"))
        );
    }

    #[test]
    fn lines_it_cant_read_are_skipped() {
        let pins = parse(
            "firefox\teDP-1\tindex:x\n\nfoot\teDP-1\n\tDP-1\tindex:1\nfoot\teDP-1\tindex:1\n",
        );
        assert_eq!(pins, [Pin::new("foot", "eDP-1", None, 1)]);
    }

    #[test]
    fn a_live_old_file_migrates() {
        let old = include_str!("../../../tests/fixtures/state/dock-pins");
        crate::services::state::tests::in_folder("dock-pins", &[(OLD_FILE, old)], |_| {
            let pins = state::load::<Pins>().expect("the old file").0;
            assert_eq!(pins, [Pin::new("code", "eDP-1", None, 2)]);
        });
    }
}
