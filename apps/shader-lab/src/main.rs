//! Shader Lab: tries amane shader effects on a shape, text or an icon, with
//! a control for every parameter the effect's manifest lists. Shaders and
//! manifests reload when saved; run it with `--effects <dir>` to edit a
//! source tree, as in `--effects apps/shader-lab/effects`. `--effect
//! <folder>`, `--subject <n>` and `--backdrop <n>` set what it starts on.

mod effects;
mod panel;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use amane::{
    Center, Color, Fill, Image, Rectangle, Row, Service, Size, Text, Weight, Window, request_frame,
};
use amane_kit::app::{app, app_window, standard_shortcut};
use amane_kit::layout::boxed;
use amane_kit::tokens::sys;

use effects::{ColorParam, Effects, Manifest, Param};

const SUBJECTS: [&str; 7] = [
    "Rounded rectangle",
    "Pill",
    "Circle",
    "Text",
    "Image",
    "Fill preview",
    "Photo",
];

/// The subjects that show an image file: Image, fitted in, and Photo, covering.
pub const IMAGE: usize = 4;
pub const FILL: usize = 5;
pub const PHOTO: usize = 6;

const BACKDROPS: [&str; 4] = ["Dark", "Light", "Grey", "Wallpaper"];

/// What the lab shows and every parameter's value, kept while it runs.
pub struct Lab {
    pub effect: usize,
    pub subject: usize,
    pub backdrop: usize,
    pub size: f32,
    pub text: String,
    pub image: String,
    pub photo: String,
    /// By effect name, then parameter key; a parameter left out is at its default.
    pub values: HashMap<String, HashMap<String, f32>>,
    /// By effect name, then colour key; a colour left out is at its default.
    pub colors: HashMap<String, HashMap<String, [f32; 4]>>,
    /// The gradient preset chosen for each effect.
    pub presets: HashMap<String, usize>,
    pub animate: bool,
    pub started: Instant,
    /// How far the controls are scrolled.
    pub scroll: f32,
    /// Presses on the subject: where, from 0 to 1 across it, and when.
    pub presses: Vec<(f32, f32, Instant)>,
    /// A press is held, so dragging on doesn't press again.
    pub pressing: bool,
}

impl Service for Lab {
    fn new() -> Self {
        let flag = |name: &str| {
            let mut args = std::env::args().skip(1);

            while let Some(arg) = args.next() {
                if arg == name {
                    return args.next();
                }
            }

            None
        };

        // `--effect <folder>` picks an effect by its folder's name
        let effects = Effects::read();

        let effect = flag("--effect")
            .and_then(|folder| {
                effects
                    .list
                    .iter()
                    .position(|entry| entry.dir.file_name().is_some_and(|name| *name == *folder))
            })
            .unwrap_or(0);

        // what the effect suggests, unless the flags say otherwise
        let suggest = effects
            .list
            .get(effect)
            .and_then(|entry| entry.manifest.as_ref().ok())
            .map(|manifest| manifest.suggest.clone())
            .unwrap_or_default();

        let subject = flag("--subject")
            .and_then(|value| value.parse().ok())
            .or(suggest.subject)
            .unwrap_or(0);

        let backdrop = flag("--backdrop")
            .and_then(|value| value.parse().ok())
            .or(suggest.backdrop)
            .unwrap_or(0);

        Self {
            effect,
            subject,
            backdrop,
            size: 120.0,
            text: "chrome".into(),
            image: default_image(),
            photo: home("Pictures/overview.jpg"),
            values: HashMap::new(),
            colors: HashMap::new(),
            presets: HashMap::new(),
            animate: false,
            started: Instant::now(),
            scroll: 0.0,
            presses: Vec::new(),
            pressing: false,
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

        self.values
            .get(&manifest.name)
            .and_then(|values| values.get(&param.key))
            .copied()
            .unwrap_or(param.default)
    }

    pub fn set(&mut self, effect: &str, key: &str, value: f32) {
        self.values
            .entry(effect.to_owned())
            .or_default()
            .insert(key.to_owned(), value);
    }

    pub fn preset(&self, manifest: &Manifest) -> usize {
        self.presets.get(&manifest.name).copied().unwrap_or(0)
    }

    pub fn color(&self, manifest: &Manifest, param: &ColorParam) -> [f32; 4] {
        self.colors
            .get(&manifest.name)
            .and_then(|colors| colors.get(&param.key))
            .copied()
            .or_else(|| effects::parse_hex(&param.default))
            .unwrap_or([0.0, 0.0, 0.0, 1.0])
    }

    pub fn set_color(&mut self, effect: &str, key: &str, color: [f32; 4]) {
        self.colors
            .entry(effect.to_owned())
            .or_default()
            .insert(key.to_owned(), color);
    }

    /// The effect's `values` as set now, all but the presses.
    pub fn rows(&self, manifest: &Manifest) -> Vec<[f32; 4]> {
        let presets = manifest.gradient.as_ref().map(|gradient| &gradient.presets);
        let preset = presets.and_then(|presets| presets.get(self.preset(manifest)));

        let mut rows = manifest.pack(|param| self.value(manifest, param), preset);

        manifest.put_colors(&mut rows, |param| self.color(manifest, param));

        rows
    }

    /// Picks an effect, and the subject and backdrop it suggests.
    pub fn pick(&mut self, index: usize, manifest: Option<&Manifest>) {
        self.effect = index;

        if let Some(manifest) = manifest {
            if let Some(subject) = manifest.suggest.subject {
                self.subject = subject;
            }

            if let Some(backdrop) = manifest.suggest.backdrop {
                self.backdrop = backdrop;
            }
        }
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

    let entry = effects.list.get(lab.effect);

    let effect = entry.and_then(|entry| {
        let manifest = entry.manifest.as_ref().ok()?;

        let mut values = lab.rows(manifest);

        if let Some(clicks) = &manifest.clicks {
            let presses: Vec<[f32; 3]> = lab
                .presses
                .iter()
                .map(|(x, y, at)| [*x, *y, at.elapsed().as_secs_f32()])
                .filter(|press| press[2] < clicks.seconds)
                .collect();

            // the ripples' ages move on while any is still showing
            if !presses.is_empty() {
                request_frame();
            }

            manifest.put_clicks(&mut values, &presses);
        }

        Some(manifest.effect(&entry.dir, values))
    });

    let takes_clicks = entry
        .and_then(|entry| entry.manifest.as_ref().ok())
        .is_some_and(|manifest| manifest.clicks.is_some());

    // the sweep moves every frame while it's on
    if lab.animate {
        request_frame();
    }

    let mut subject = subject(&lab);

    if let Some(effect) = effect {
        subject = subject.effect(effect);
    }

    if takes_clicks {
        let (width, height) = subject_size(&lab);

        subject = subject
            .on_drag(move |point| {
                let mut lab = Lab::write();

                if lab.pressing {
                    return;
                }

                lab.pressing = true;

                // only the ones still showing are kept
                lab.presses
                    .retain(|(_, _, at)| at.elapsed().as_secs_f32() < 10.0);

                let at = (point.x / width, point.y / height, Instant::now());
                lab.presses.push(at);
            })
            .on_release(|_| Lab::write().pressing = false);
    }

    /*
     * the clip holds the backdrop's fill as well as the subject: a clip
     * holding an effect draws on its own canvas, and an effect reading the
     * backdrop sees only what that canvas holds
     */
    let preview = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .clip()
        .child(
            Rectangle::new()
                .width(Size::Parent)
                .height(Size::Parent)
                .fill(backdrop(lab.backdrop))
                .align_child(Center, Center)
                .child(subject),
        );

    let content = Row::new(vec![boxed(preview), boxed(panel::panel(&effects, &lab))])
        .gap(sys::space::gap::l())
        .width(Size::Parent)
        .height(Size::Parent);

    app_window(content)
        .title("Shader Lab")
        .size(1100.0, 760.0)
        .min_size(800.0, 600.0)
        .on_shortcut(standard_shortcut)
}

/// How big the subject is, which a press's place is measured against.
fn subject_size(lab: &Lab) -> (f32, f32) {
    let size = lab.size;

    match lab.subject {
        0 => (size * 1.6, size),
        1 => (size * 2.2, size * 0.7),
        2 => (size, size),
        3 => (size * 4.0, size * 1.3),
        IMAGE => (size * 1.6, size * 1.6),
        PHOTO => (size * 2.4, size * 1.5),

        // the window less its padding, the gap and the controls
        _ => {
            let (width, height) = amane::window_size();
            let inset = sys::space::inset::l() * 2.0;

            (
                (width - inset - sys::space::gap::l() - panel::WIDTH).max(1.0),
                (height - inset).max(1.0),
            )
        }
    }
}

/// The shape the effect is tried on. Only its alpha matters to the chrome,
/// so it's drawn in plain white.
fn subject(lab: &Lab) -> Rectangle {
    let size = lab.size;
    let (width, height) = subject_size(lab);

    let sized = Rectangle::new().width(width).height(height);

    match lab.subject {
        0 => sized.radius(size * 0.22).fill(Color::WHITE),
        1 => sized.radius(height / 2.0).fill(Color::WHITE),
        2 => sized.radius(size / 2.0).fill(Color::WHITE),

        3 => sized.align_child(Center, Center).child(
            Text::new(lab.text.clone())
                .size(size)
                .weight(Weight::Bold)
                .color(Color::WHITE),
        ),

        IMAGE => sized.fill(Image::contain(lab.image.clone())),
        PHOTO => sized.fill(Image::cover(lab.photo.clone())),

        // the whole preview, for effects that are backgrounds
        _ => Rectangle::new()
            .width(Size::Parent)
            .height(Size::Parent)
            .fill(Color::WHITE),
    }
}

fn backdrop(index: usize) -> Fill {
    match index {
        0 => Fill::from("#1e1e2e"),
        1 => Fill::from("#eff1f5"),
        2 => Fill::from("#7f7f86"),
        _ => {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();

            Fill::from(Image::cover(home.join("Pictures/overview.jpg")))
        }
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
