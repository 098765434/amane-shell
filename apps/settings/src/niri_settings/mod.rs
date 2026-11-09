//! niri settings kept in `settings.kdl` ([`kdl`]), for the pages that show
//! them: Windows, Input's gestures and shortcuts, Appearance's pointer and
//! Desktop's screenshots.
//!
//! The service reads `config.kdl` and the file, and follows niri's config
//! loads, so a change from anywhere shows. A change is written at once (a
//! slider's when it rests) and committed; when niri's load just after one
//! fails, the file is put back as it was and the page says so. A focus ring
//! set to follow the accent is written again when the accent changes.

pub mod kdl;

use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use amane::{Config, Service};
use emaki_kit::app::Dialog;
use emaki_kit::buttons::PushButton;
use emaki_kit::color::Accent;
use emaki_kit::form::{FormRow, Group};
use emaki_kit::icons::ICON_INFO;

use crate::{group, switch_row};
pub use kdl::Key;
use kdl::{Setup, Sparse};

/// How long a slider rests before its change is written, so a drag is
/// one write and one commit.
pub const SETTLE: Duration = Duration::from_millis(400);

/// How soon after a write of ours a failed load counts as ours.
const OURS_FOR: Duration = Duration::from_secs(5);

/// The marker on a focus ring's colour that follows the accent.
pub const FOLLOWS_ACCENT: &str = "// follows the accent";

/// A focus ring's colour when no accent is chosen: niri's own.
const RING_DEFAULT: &str = "#7fc8ff";

pub struct NiriSettings {
    setup: Setup,
    error: Option<String>,
    /// The file before our last write, and when: put back if niri refuses it.
    before: Option<(Instant, Option<String>)>,
    looked: bool,
}

static CHANGES: AtomicU64 = AtomicU64::new(0);

impl Service for NiriSettings {
    // read from listen(), so opening a page never waits on the disk
    fn new() -> Self {
        Self {
            setup: Setup::Missing,
            error: None,
            before: None,
            looked: false,
        }
    }

    fn listen() {
        refresh();
        follow_accent();
        // the accent can change on Appearance, or in the system
        Config::<emaki_kit::settings::Appearance>::subscribe(|_| follow_accent()).forever();
        amane::Appearance::subscribe(|_| follow_accent()).forever();
        amane::niri::follow_config_loads(|failed| {
            let ours = NiriSettings::write().before.take();
            let ours = ours.filter(|(at, _)| at.elapsed() < OURS_FOR);
            if failed {
                if let Some((_, before)) = ours {
                    let undone = before.and_then(|text| kdl::restore(&text).err());
                    NiriSettings::write().error = Some(match undone {
                        None => String::from(
                            "niri didn't take that change, so it was put back as it was.",
                        ),
                        Some(why) => format!(
                            "niri didn't take that change, and it couldn't be put back: {why}"
                        ),
                    });
                }
                return;
            }
            refresh();
        });
    }
}

/// Reads both files again (off the UI thread).
fn refresh() {
    let setup = kdl::setup();
    let mut now = NiriSettings::write();
    let changed = !now.looked || now.setup != setup;
    now.setup = setup;
    now.looked = true;
    if !changed {
        now.quiet();
    }
}

/// The settings as a page shows them.
#[derive(Clone)]
pub struct Shown {
    ours: Sparse,
    theirs: Sparse,
    /// Whether a change can be written: the file is included.
    pub editable: bool,
    pub looked: bool,
}

/// What's in force now, for a page to draw.
pub fn shown() -> Shown {
    let now = NiriSettings::read();
    let (ours, theirs, editable) = match &now.setup {
        Setup::Ready { ours, theirs } => (ours.clone(), theirs.clone(), true),
        Setup::NotIncluded { theirs } => (Sparse::default(), theirs.clone(), false),
        Setup::Missing => (Sparse::default(), Sparse::default(), false),
    };
    Shown {
        ours,
        theirs,
        editable,
        looked: now.looked,
    }
}

impl Shown {
    /// What `key` is in force as: ours, `config.kdl`'s, or niri's default.
    pub fn get(&self, key: Key) -> Option<String> {
        kdl::effective(&self.ours, &self.theirs, key)
    }

    /// Whether a flag or a section's state is on.
    pub fn on(&self, key: Key) -> bool {
        self.get(key).is_some_and(|value| kdl::truthy(&value))
    }

    /// A number setting, else `or`.
    pub fn number<T: std::str::FromStr>(&self, key: Key, or: T) -> T {
        self.get(key)
            .and_then(|value| value.parse().ok())
            .unwrap_or(or)
    }

    /// A quoted setting's text, else `or`.
    pub fn word(&self, key: Key, or: &str) -> String {
        self.get(key)
            .and_then(|value| crate::niri_files::unquote(&value))
            .unwrap_or_else(|| or.to_string())
    }

    /// Whether any of `keys` is set in `settings.kdl`.
    pub fn any_ours(&self, keys: &[Key]) -> bool {
        keys.iter().any(|key| self.ours.line(*key).is_some())
    }

    /// Whether the focus ring's colour follows the accent.
    pub fn follows_accent(&self) -> bool {
        self.ours
            .line(("layout/focus-ring", "active-color"))
            .is_some_and(|line| line.contains(FOLLOWS_ACCENT))
    }
}

/// Makes `edit` at once on the page, and writes it after `wait`, unless
/// another change comes first.
pub fn change(what: &'static str, wait: Duration, edit: impl FnOnce(&mut Sparse)) {
    {
        let mut now = NiriSettings::write();
        let Setup::Ready { ours, .. } = &mut now.setup else {
            return;
        };
        edit(ours);
        now.error = None;
    }
    let number = CHANGES.fetch_add(1, Ordering::SeqCst) + 1;
    thread::spawn(move || {
        if !wait.is_zero() {
            thread::sleep(wait);
        }
        if CHANGES.load(Ordering::SeqCst) != number {
            return;
        }
        let Setup::Ready { ours, .. } = NiriSettings::read().setup.clone() else {
            return;
        };
        let before = kdl::current_text();
        NiriSettings::write().before = Some((Instant::now(), before));
        if let Err(error) = kdl::save(&ours, what) {
            let mut now = NiriSettings::write();
            now.error = Some(error);
            now.before = None;
        }
    });
}

/// Sets `key` to `value` and writes it at once.
pub fn set(what: &'static str, key: Key, value: impl Into<String>) {
    let value = value.into();
    change(what, Duration::ZERO, move |ours| ours.set(key, &value));
}

/// Sets `key` to `value`, written once a slider rests.
pub fn set_settled(what: &'static str, key: Key, value: impl Into<String>) {
    let value = value.into();
    change(what, SETTLE, move |ours| ours.set(key, &value));
}

/// Leaves `keys` to `config.kdl` again.
pub fn reset(what: &'static str, keys: &'static [Key]) {
    change(what, Duration::ZERO, |ours| {
        for key in keys {
            ours.remove(*key);
        }
    });
}

/// A switch for a section's state (`on`/`off`) or a flag (`true`/`false`).
pub fn switch(shown: &Shown, id: &str, title: &str, what: &'static str, key: Key) -> FormRow {
    let state = key.1 == "state";
    switch_row(id, title, shown.on(key), move |on| {
        let value = match (state, on) {
            (true, true) => "on",
            (true, false) => "off",
            (false, true) => "true",
            (false, false) => "false",
        };
        set(what, key, value);
    })
    .disabled(!shown.editable)
}

/// A switch for something worked out from the settings, off unless they
/// can be changed.
pub fn switch_with(
    shown: &Shown,
    id: &str,
    title: &str,
    on: bool,
    toggle: impl Fn(bool) + 'static,
) -> FormRow {
    switch_row(id, title, on, toggle).disabled(!shown.editable)
}

/// Makes the focus ring follow the accent, or take config.kdl's colour again.
pub fn set_follows_accent(follow: bool) {
    let hex = accent_hex();
    change("focus ring color", Duration::ZERO, move |ours| {
        let key = ("layout/focus-ring", "active-color");
        if follow {
            ours.set_line(key, format!("active-color \"{hex}\" {FOLLOWS_ACCENT}"));
        } else {
            // config.kdl's own colour again
            ours.remove(key);
        }
    });
}

fn accent_hex() -> String {
    emaki_kit::theme::accent()
        .map(|color| color.hex())
        .unwrap_or_else(|| String::from(RING_DEFAULT))
}

/// Writes the accent into a focus ring that follows it, when it changed.
fn follow_accent() {
    let shown = shown();
    if !shown.editable || !shown.follows_accent() {
        return;
    }
    let hex = accent_hex();
    if shown.word(("layout/focus-ring", "active-color"), "") == hex {
        return;
    }
    set_follows_accent(true);
}

/// Above a page using these settings: what's in the way of changing
/// them, or what went wrong; none when all's well.
pub fn notices(id: &str) -> Vec<Group> {
    let now = NiriSettings::read();
    let mut out = Vec::new();
    match &now.setup {
        Setup::Ready { .. } => {}
        Setup::Missing => out.push(
            group().row(
                FormRow::new(format!("{id}.missing"), "Changes here can't be kept")
                    .icon(emaki_kit::icons::ICON_WARNING, Accent::Orange)
                    .subtitle("niri's config.kdl wasn't found"),
            ),
        ),
        Setup::NotIncluded { .. } => out.push(
            group()
                .row(
                    FormRow::new(format!("{id}.set-up"), "Shown, not changed here")
                        .icon(ICON_INFO, Accent::Gray)
                        .subtitle("niri's config.kdl doesn't include Emaki Settings' file yet.")
                        .trailing(
                            PushButton::new(format!("{id}.set-up.button"), "Set Up…")
                                .on_press(confirm_set_up)
                                .build(),
                        ),
                )
                .footer(
                    "Set up once to change niri's settings here; until then the page shows \
                     what config.kdl sets.",
                ),
        ),
    }
    if let Some(error) = &now.error {
        out.push(group().row(crate::dismissible_problem_row(
            &format!("{id}.error"),
            "Not changed",
            error,
            || NiriSettings::write().error = None,
        )));
    }
    out
}

fn confirm_set_up() {
    Dialog::new("settings.niri.set-up", "Let Settings change niri?")
        .message(format!(
            "An include of {} goes at the end of {}, as a commit of its own. The settings \
             changed here are written there, and win over config.kdl's own; Reset hands \
             one back.",
            kdl::settings_file().display(),
            crate::niri_files::niri_config().display()
        ))
        .button("Cancel", || {})
        .main_button("Set Up", || {
            thread::spawn(|| {
                if let Err(error) = kdl::set_up() {
                    NiriSettings::write().error = Some(error);
                }
                refresh();
            });
        })
        .open("main");
}
