//! Displays: each screen niri drives, with what it can do found out from
//! the screen itself (its EDID and the kernel's connector), and its
//! resolution, refresh rate, variable refresh, scale, rotation and place.
//!
//! niri makes a change at once over IPC ([`amane::niri`]) and keeps it only until
//! its config is read again, so a lasting change is written to
//! `displays.kdl` ([`kdl`]), which `config.kdl` includes. A change that
//! could leave a screen unreadable (resolution, refresh rate, scale,
//! rotation, turning one off) is tried first: it is written only when
//! kept, and undone when the countdown runs out ([`trial`]).
//!
//! The screens are read every two seconds while the page shows, which is
//! how a monitor plugged in turns up: niri announces no such change.

mod caps;
mod ddc;
mod drm;
mod edid;
mod kdl;
mod night;
mod outputs;
mod sections;
mod trial;

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use amane::{Column, Service};
use amane_kit::buttons::PushButton;
use amane_kit::form::{FormRow, Group};
use amane_kit::layout::boxed;
use amane_kit::tokens::sys;

use crate::{group, groups};
use caps::Caps;
use kdl::{Displays as Saved, OutputConfig, Setup};
use outputs::{Output, OutputExt};

// whether the page shows, so the poll can rest while it doesn't
static SHOWN: AtomicBool = AtomicBool::new(false);

/// Says whether the page is the one showing; the window calls it with
/// every page it draws.
pub fn showing(shown: bool) {
    let was = SHOWN.swap(shown, Ordering::Relaxed);
    if shown && !was {
        thread::spawn(Screens::refresh);
    }
}

/// How long an error stays on the page.
const ERROR_FOR: Duration = Duration::from_secs(10);

/// The screens as niri has them, what each can do, and the saved file.
pub struct Screens {
    /// Built-in first, then by connector; none while niri isn't answering.
    outputs: Option<Vec<Output>>,
    caps: Vec<(String, Caps)>,
    setup: Setup,
    /// The connector the page shows.
    selected: Option<String>,
    /// What last went wrong and when: shown for [`ERROR_FOR`], or until
    /// dismissed.
    error: Option<(String, std::time::Instant)>,
    /// niri has been asked once; until then the page says it's reading.
    looked: bool,
}

impl Service for Screens {
    fn new() -> Self {
        let screens = Self {
            outputs: None,
            caps: Vec::new(),
            setup: Setup::Missing,
            selected: None,
            error: None,
            looked: false,
        };
        // the first look reads every screen's EDID: off the UI thread
        thread::spawn(Screens::refresh);
        screens
    }

    fn interval() -> Duration {
        Duration::from_secs(2)
    }

    fn update(&mut self) -> bool {
        let expired = self
            .error
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() >= ERROR_FOR);
        if expired {
            self.error = None;
        }
        if !SHOWN.load(Ordering::Relaxed) {
            return expired;
        }
        let fresh = look(&self.caps);
        self.take(fresh) || expired
    }
}

/// What a look finds, read with nothing locked.
struct Look {
    outputs: Option<Vec<Output>>,
    caps: Vec<(String, Caps)>,
    setup: Setup,
}

fn look(known: &[(String, Caps)]) -> Look {
    let outputs = outputs::all().map(|outputs| {
        let mut outputs: Vec<Output> = outputs.into_values().collect();
        outputs.sort_by_key(|output| (!output.built_in(), output.name.clone()));
        outputs
    });
    let caps = outputs
        .iter()
        .flatten()
        .map(|output| {
            let cached = known.iter().find(|(name, _)| *name == output.name);
            let caps = Caps::read(&output.name, cached.map(|(_, caps)| caps));
            (output.name.clone(), caps)
        })
        .collect();
    Look {
        outputs,
        caps,
        setup: kdl::setup(),
    }
}

impl Screens {
    /// Takes in a look; true when anything shown changed.
    fn take(&mut self, fresh: Look) -> bool {
        let changed = !self.looked
            || fresh.outputs != self.outputs
            || fresh.caps != self.caps
            || fresh.setup != self.setup;
        self.looked = true;
        self.outputs = fresh.outputs;
        self.caps = fresh.caps;
        self.setup = fresh.setup;
        let gone = self.selected.as_ref().is_some_and(|name| {
            !self
                .outputs
                .iter()
                .flatten()
                .any(|output| output.name == *name)
        });
        if gone || self.selected.is_none() {
            self.selected = self
                .outputs
                .iter()
                .flatten()
                .next()
                .map(|output| output.name.clone());
        }
        changed || gone
    }

    /// Looks again now, after a change of our own (off the UI thread).
    pub fn refresh() {
        let known = Self::read().caps.clone();
        let fresh = look(&known);
        let mut screens = Self::write();
        if !screens.take(fresh) {
            screens.quiet();
        }
    }

    pub fn fail(error: String) {
        Self::write().error = Some((error, std::time::Instant::now()));
    }

    /// Puts the last error away.
    pub fn dismiss() {
        Self::write().error = None;
    }

    fn selected_output(&self) -> Option<&Output> {
        let name = self.selected.as_ref()?;
        self.outputs
            .iter()
            .flatten()
            .find(|output| output.name == *name)
    }

    fn caps_of(&self, name: &str) -> Caps {
        self.caps
            .iter()
            .find(|(connector, _)| connector == name)
            .map(|(_, caps)| caps.clone())
            .unwrap_or_default()
    }

    /// The saved file, when the page may change it.
    fn saved(&self) -> Option<&Saved> {
        match &self.setup {
            Setup::Ready(saved) => Some(saved),
            _ => None,
        }
    }
}

/// The name a screen's block goes by: a laptop's panel by its connector,
/// a monitor by make, model and serial so it keeps its settings on any
/// port, or by connector when it gives no serial.
pub fn block_name(output: &Output) -> String {
    if output.built_in() || output.serial.is_none() {
        output.name.clone()
    } else {
        output.identity()
    }
}

/// The saved block for `output`, by either name it could have.
fn saved_block<'a>(saved: &'a Saved, output: &Output) -> Option<&'a OutputConfig> {
    saved
        .output(&block_name(output))
        .or_else(|| saved.output(&output.name))
        .or_else(|| saved.output(&output.identity()))
}

/// `output` as it runs now, as a block to save: what's already saved for
/// it, with what niri shows on top.
fn current_block(saved: &Saved, output: &Output) -> OutputConfig {
    let mut block = saved_block(saved, output).cloned().unwrap_or_default();
    block.name = block_name(output);
    if let Some(mode) = output.mode().copied() {
        block.mode = Some((mode.width, mode.height, mode.hz()));
        block.custom = output.is_custom_mode;
    }
    if let Some(logical) = output.logical {
        block.scale = Some(logical.scale);
        block.transform = logical.transform;
    }
    block.off = !output.on();
    if output.hdr_supported {
        block.hdr = output.hdr_enabled;
        if output.hdr_enabled {
            block.sdr_brightness = output.sdr_brightness.map(f64::round);
        }
    }
    block
}

/// Saves `edit` made to `output`'s block, off the UI thread.
pub fn save(output: &Output, what: String, edit: impl FnOnce(&mut OutputConfig) + Send + 'static) {
    let output = output.clone();
    thread::spawn(move || {
        let Some(mut saved) = Screens::read().saved().cloned() else {
            return;
        };
        let mut block = current_block(&saved, &output);
        edit(&mut block);
        // one block per screen, under the name it goes by now
        saved.outputs.retain(|other| {
            other.name != block.name && other.name != output.name && other.name != output.identity()
        });
        saved.outputs.push(block);
        if let Err(error) = kdl::save(&saved, &what) {
            Screens::fail(error);
        }
        Screens::refresh();
    });
}

/// Saves a change to the file as a whole: the apps that turn on-demand
/// variable refresh on.
pub fn save_all(what: String, edit: impl FnOnce(&mut Saved) + Send + 'static) {
    thread::spawn(move || {
        let Some(mut saved) = Screens::read().saved().cloned() else {
            return;
        };
        edit(&mut saved);
        if let Err(error) = kdl::save(&saved, &what) {
            Screens::fail(error);
        }
        Screens::refresh();
    });
}

/// `group` built onto `rows`, if there is one with anything in it.
fn add(rows: &mut Vec<Box<dyn amane::Widget>>, group: Option<Group>) {
    if let Some(group) = group.filter(Group::has_rows) {
        rows.push(boxed(group.build()));
    }
}

pub fn page() -> Column {
    let screens = Screens::read();
    if !screens.looked {
        drop(screens);
        return groups(vec![group().row(FormRow::new(
            "settings.displays.reading",
            "Reading the displays…",
        ))]);
    }
    let Some(outputs) = screens.outputs.clone() else {
        drop(screens);
        return groups(vec![
            group().row(
                FormRow::new("settings.displays.silent", "The compositor isn't answering")
                    .icon(
                        amane_kit::icons::ICON_WARNING,
                        amane_kit::color::Accent::Orange,
                    )
                    .subtitle("The displays can't be read until it does")
                    .trailing(
                        PushButton::new("settings.displays.retry", "Try Again")
                            .on_press(|| {
                                thread::spawn(Screens::refresh);
                            })
                            .build(),
                    ),
            ),
        ]);
    };
    let Some(output) = screens.selected_output().cloned() else {
        drop(screens);
        return groups(vec![group().row(FormRow::new(
            "settings.displays.none",
            "No displays are connected",
        ))]);
    };
    let caps = screens.caps_of(&output.name);
    let setup = screens.setup.clone();
    let error = screens.error.as_ref().map(|(error, _)| error.clone());
    drop(screens);

    let saved = match &setup {
        Setup::Ready(saved) => Some(saved.clone()),
        _ => None,
    };
    let ctx = sections::Ctx {
        output,
        outputs,
        caps,
        saved,
    };

    let mut rows: Vec<Box<dyn amane::Widget>> = Vec::new();
    add(&mut rows, sections::setup_banner(&setup));
    if let Some(error) = error {
        rows.push(boxed(sections::error_line(error)));
    }
    add(&mut rows, sections::arrangement(&ctx));
    add(&mut rows, Some(sections::header(&ctx)));

    // the picture: how bright, how big, how fast, which way up
    let picture = sections::brightness(&ctx, group())
        .row(sections::resolution(&ctx))
        .rows(sections::refresh_rate(&ctx))
        .row(sections::scale(&ctx))
        .row(sections::rotation(&ctx));
    add(&mut rows, Some(picture));
    add(&mut rows, sections::multiple(&ctx));
    add(&mut rows, Some(sections::variable_refresh(&ctx)));
    add(&mut rows, Some(sections::hdr(&ctx)));
    add(&mut rows, Some(sections::advanced_link()));
    // what holds for every display
    add(&mut rows, sections::all_displays(&ctx));
    add(&mut rows, Some(sections::night_light()));
    add(&mut rows, sections::oled_care(&ctx));

    Column::new(rows).gap(sys::space::gap::l())
}

/// The page's sub-pages: Advanced, for the display picked.
pub fn sub(name: &str) -> Option<(String, Box<dyn amane::Widget>)> {
    if name != "advanced" {
        return None;
    }
    let screens = Screens::read();
    let outputs = screens.outputs.clone()?;
    let output = screens.selected_output().cloned()?;
    let caps = screens.caps_of(&output.name);
    let saved = match &screens.setup {
        Setup::Ready(saved) => Some(saved.clone()),
        _ => None,
    };
    let error = screens.error.as_ref().map(|(error, _)| error.clone());
    drop(screens);
    let ctx = sections::Ctx {
        output,
        outputs,
        caps,
        saved,
    };
    let mut rows: Vec<Box<dyn amane::Widget>> = Vec::new();
    if let Some(error) = error {
        rows.push(boxed(sections::error_line(error)));
    }
    add(&mut rows, Some(sections::advanced(&ctx)));
    Some((
        format!("{} — Advanced", ctx.name()),
        boxed(Column::new(rows).gap(sys::space::gap::l())),
    ))
}
