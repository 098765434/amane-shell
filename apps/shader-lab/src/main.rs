//! Shader Lab: tries amane shader effects on a shape, text, a picture or the
//! whole preview, with a control for every parameter the effect's manifest
//! lists, shadows, a colour picker, and looks saved for the shell.
//!
//! Run it with `--effects apps/shader-lab/effects` to edit the source tree:
//! shaders and manifests reload when saved, and looks are saved into
//! `apps/shader-lab/configs`, where `amane_kit::lab::look` finds them.
//! `--effect <folder>`, `--subject <n>`, `--backdrop <n>` and `--config
//! <name>` set what it starts on, `--open <picker>` opens a colour picker;
//! `--configs <dir>` saves looks elsewhere.

mod configs;
mod effects;
mod gradient;
mod history;
mod panel;
mod preview;
mod shadows;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use amane::{Key, Point, Row, Service, Size, Window, modifiers};
use amane_kit::app::{app, app_window, standard_shortcut};
use amane_kit::lab::config::{Backdrop, Config, Kind};
use amane_kit::lab::manifest::{Manifest, Param};
use amane_kit::layout::boxed;
use amane_kit::tokens::sys;

use effects::{Effects, flag};
use history::History;

/// What the lab shows, the look being edited and how to get back to the
/// last ones, kept while it runs.
pub struct Lab {
    pub effect: usize,
    /// The look being edited: everything Save writes.
    pub state: Config,
    /// Each other effect's last look, so switching back finds it as it was.
    stash: HashMap<String, Config>,
    pub history: History,
    /// The saved file the look came from or went to, by its name.
    pub saved_as: Option<String>,

    pub animate: bool,
    pub started: Instant,
    /// How far the controls are scrolled.
    pub scroll: f32,
    /// Presses on the subject: where, from 0 to 1 across it, and when.
    pub presses: Vec<(f32, f32, Instant)>,
    pub drag: Option<Drag>,
    /// Which colour picker is open: `color.<key>`, `shadow.<n>` or `stop`.
    pub picking: Option<String>,
    /// The gradient stop the editor has selected.
    pub stop: usize,
    /// The name being typed for Save as or Rename.
    pub naming: Option<Naming>,
    /// What saving last said.
    pub message: Option<String>,
}

/// A press on the preview: where it began and where the subject was.
pub struct Drag {
    pub start: Point,
    pub from: (f32, f32),
    pub moved: bool,
}

#[derive(Clone, PartialEq)]
pub enum Naming {
    SaveAs,
    Rename(String),
}

impl Service for Lab {
    fn new() -> Self {
        let effects = Effects::read();

        let effect = flag("--effect")
            .and_then(|folder| effects.position(&folder))
            .unwrap_or(0);

        let entry = effects.list.get(effect);
        let folder = entry.map(|entry| entry.folder()).unwrap_or_default();
        let manifest = entry.and_then(|entry| entry.manifest.as_ref().ok());

        let mut state = Config::new(&folder);

        state.subject.image = default_image();
        state.subject.photo = home("Pictures/overview.jpg");

        if let Some(manifest) = manifest {
            suggest(&mut state, manifest);
        }

        if let Some(index) = flag("--subject").and_then(|value| value.parse().ok()) {
            state.subject.kind = Kind::from_index(index);
        }

        if let Some(index) = flag("--backdrop").and_then(|value| value.parse().ok()) {
            state.backdrop = Backdrop::from_index(index);
        }

        // `--config <name>` starts on a saved look
        let mut saved_as = None;

        if let Some(slug) = flag("--config") {
            let path = configs::folder().join(format!("{slug}.json"));

            let config = std::fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|text| {
                    serde_json::from_str::<Config>(&text).map_err(|error| error.to_string())
                });

            match config {
                Ok(config) => {
                    state = config;
                    saved_as = Some(slug);
                }
                Err(error) => eprintln!("amane-shader-lab: {}: {error}", path.display()),
            }
        }

        let effect = effects.position(&state.effect).unwrap_or(effect);

        Self {
            effect,
            state,
            stash: HashMap::new(),
            history: History::default(),
            saved_as,
            animate: false,
            started: Instant::now(),
            scroll: 0.0,
            presses: Vec::new(),
            drag: None,
            // `--open <picker>`, like `color.tint` or `shadow.0`, for screenshots
            picking: flag("--open"),
            stop: 0,
            naming: None,
            message: None,
        }
    }

    // only the controls change it
    fn listen() {}
}

impl Lab {
    /// The parameter's value now: swept while animating, else as set.
    pub fn value(&self, manifest: &Manifest, param: &Param) -> f32 {
        if self.animate
            && let Some(animate) = &manifest.animate
            && animate.key == param.key
        {
            let elapsed = self.started.elapsed().as_secs_f32();
            let through = (elapsed / animate.seconds.max(0.1)).fract();

            return animate.from + through * (animate.to - animate.from);
        }

        self.state.value(param)
    }

    /// The effect's `values`, all but the surface and the presses.
    pub fn rows(&self, manifest: &Manifest) -> Vec<[f32; 4]> {
        self.state
            .rows_with(manifest, |param| self.value(manifest, param))
    }
}

/// Every change to the look goes through here, so it can be undone;
/// changes to one `key` close together are one step.
pub fn change(key: &str, edit: impl FnOnce(&mut Config)) {
    let mut lab = Lab::write();
    let before = lab.state.clone();

    edit(&mut lab.state);

    if lab.state != before {
        lab.history.record(key, &before);
    }
}

/// Switches to another effect, with its last look or a fresh one at the
/// subject and backdrop it suggests.
pub fn pick_effect(index: usize) {
    let effects = Effects::read();

    let Some(entry) = effects.list.get(index) else {
        return;
    };

    let folder = entry.folder();
    let mut lab = Lab::write();

    if folder == lab.state.effect {
        return;
    }

    let before = lab.state.clone();

    let next = match lab.stash.remove(&folder) {
        Some(next) => next,

        None => {
            let mut fresh = Config::new(&folder);

            fresh.subject = before.subject.clone();
            fresh.backdrop = before.backdrop;

            if let Ok(manifest) = &entry.manifest {
                suggest(&mut fresh, manifest);
            }

            fresh
        }
    };

    lab.stash.insert(before.effect.clone(), before.clone());
    lab.history.record("effect", &before);
    lab.state = next;
    lab.effect = index;
    lab.saved_as = None;
    lab.picking = None;
}

/// Puts a saved look on, as one step that can be undone.
pub fn load(slug: &str, config: &Config) {
    let effects = Effects::read();
    let mut lab = Lab::write();

    let before = lab.state.clone();

    lab.history.record("load", &before);
    lab.state = config.clone();
    lab.effect = effects.position(&config.effect).unwrap_or(lab.effect);
    lab.saved_as = Some(slug.to_owned());
    lab.picking = None;
    lab.message = None;
}

pub fn undo() {
    let effects = Effects::read();
    let mut lab = Lab::write();

    let now = lab.state.clone();

    if let Some(before) = lab.history.undo(&now) {
        lab.effect = effects.position(&before.effect).unwrap_or(lab.effect);
        lab.state = before;
    }
}

pub fn redo() {
    let effects = Effects::read();
    let mut lab = Lab::write();

    let now = lab.state.clone();

    if let Some(after) = lab.history.redo(&now) {
        lab.effect = effects.position(&after.effect).unwrap_or(lab.effect);
        lab.state = after;
    }
}

/// Saves over the look's file, or asks for a name if it has none.
pub fn save() {
    let saved_as = Lab::read().saved_as.clone();

    match saved_as {
        Some(_) => panel::update_saved(),
        None => Lab::write().naming = Some(Naming::SaveAs),
    }
}

/// The subject and backdrop an effect asks for.
fn suggest(state: &mut Config, manifest: &Manifest) {
    if let Some(subject) = manifest.suggest.subject {
        state.subject.kind = Kind::from_index(subject);
    }

    if let Some(backdrop) = manifest.suggest.backdrop {
        state.backdrop = Backdrop::from_index(backdrop);
    }
}

fn main() {
    app("amane-shader-lab")
        .normal_window("main", main_window)
        .run();
}

fn main_window() -> Window {
    let effects = Effects::read();
    let lab = Lab::read();

    let content = Row::new(vec![
        boxed(preview::preview(&effects, &lab)),
        boxed(panel::panel(&effects, &lab)),
    ])
    .gap(sys::space::gap::l())
    .width(Size::Parent)
    .height(Size::Parent);

    app_window(content)
        .title("Shader Lab")
        .size(1100.0, 760.0)
        .min_size(800.0, 600.0)
        .on_shortcut(|key| shortcut(key) || standard_shortcut(key))
        .on_key(key)
}

/// Ctrl+Z undoes, Ctrl+Shift+Z or Ctrl+Y redoes, Ctrl+S saves.
fn shortcut(key: Key) -> bool {
    let held = modifiers();

    if !held.ctrl || held.alt || held.logo {
        return false;
    }

    match key {
        Key::Character('z') if !held.shift => undo(),
        Key::Character('z' | 'Z') | Key::Character('y') => redo(),
        Key::Character('s') => save(),
        _ => return false,
    }

    true
}

/// Keys no text field took: Space pauses time, the arrows nudge the
/// subject (Shift for 10 px), Delete removes the selected stop.
fn key(key: Key) {
    let step = if modifiers().shift { 10.0 } else { 1.0 };

    let nudge = |x: f32, y: f32| {
        change("move", |state| {
            state.subject.x += x;
            state.subject.y += y;
        });
    };

    match key {
        Key::Space => preview::toggle_time(),
        Key::Left => nudge(-step, 0.0),
        Key::Right => nudge(step, 0.0),
        Key::Up => nudge(0.0, -step),
        Key::Down => nudge(0.0, step),
        Key::Delete => gradient::remove_selected(),
        _ => {}
    }
}

fn home(path: &str) -> String {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();

    home.join(path).display().to_string()
}

fn default_image() -> String {
    let icon = "/run/current-system/sw/share/icons/hicolor/1024x1024/apps/nix-snowflake-white.png";

    if std::path::Path::new(icon).exists() {
        icon.into()
    } else {
        String::new()
    }
}
