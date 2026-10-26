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

use effects::{Effects, Manifest, Param};

const SUBJECTS: [&str; 5] = ["Rounded rectangle", "Pill", "Circle", "Text", "Image"];

const BACKDROPS: [&str; 4] = ["Dark", "Light", "Grey", "Wallpaper"];

/// What the lab shows and every parameter's value, kept while it runs.
pub struct Lab {
    pub effect: usize,
    pub subject: usize,
    pub backdrop: usize,
    pub size: f32,
    pub text: String,
    pub image: String,
    /// By effect name, then parameter key; a parameter left out is at its default.
    pub values: HashMap<String, HashMap<String, f32>>,
    /// The gradient preset chosen for each effect.
    pub presets: HashMap<String, usize>,
    pub animate: bool,
    pub started: Instant,
    /// How far the controls are scrolled.
    pub scroll: f32,
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

        let index = |name: &str| flag(name).and_then(|value| value.parse().ok()).unwrap_or(0);

        // `--effect <folder>` picks an effect by its folder's name
        let effect = flag("--effect")
            .and_then(|folder| {
                let effects = Effects::read();

                effects
                    .list
                    .iter()
                    .position(|entry| entry.dir.file_name().is_some_and(|name| *name == *folder))
            })
            .unwrap_or(0);

        Self {
            effect,
            subject: index("--subject"),
            backdrop: index("--backdrop"),
            size: 120.0,
            text: "chrome".into(),
            image: default_image(),
            values: HashMap::new(),
            presets: HashMap::new(),
            animate: false,
            started: Instant::now(),
            scroll: 0.0,
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

        let presets = manifest.gradient.as_ref().map(|gradient| &gradient.presets);
        let preset = presets.and_then(|presets| presets.get(lab.preset(manifest)));

        let values = manifest.pack(|param| lab.value(manifest, param), preset);

        Some(manifest.effect(&entry.dir, values))
    });

    // the sweep moves every frame while it's on
    if lab.animate {
        request_frame();
    }

    let mut subject = subject(&lab);

    if let Some(effect) = effect {
        subject = subject.effect(effect);
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

/// The shape the effect is tried on. Only its alpha matters to the chrome,
/// so it's drawn in plain white.
fn subject(lab: &Lab) -> Rectangle {
    let size = lab.size;

    match lab.subject {
        0 => Rectangle::new()
            .width(size * 1.6)
            .height(size)
            .radius(size * 0.22)
            .fill(Color::WHITE),

        1 => Rectangle::new()
            .width(size * 2.2)
            .height(size * 0.7)
            .radius(size * 0.35)
            .fill(Color::WHITE),

        2 => Rectangle::new()
            .width(size)
            .height(size)
            .radius(size / 2.0)
            .fill(Color::WHITE),

        3 => Rectangle::new()
            .width(size * 4.0)
            .height(size * 1.3)
            .align_child(Center, Center)
            .child(
                Text::new(lab.text.clone())
                    .size(size)
                    .weight(Weight::Bold)
                    .color(Color::WHITE),
            ),

        _ => Rectangle::new()
            .width(size * 1.6)
            .height(size * 1.6)
            .fill(Image::contain(lab.image.clone())),
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

fn default_image() -> String {
    let icon = "/run/current-system/sw/share/icons/hicolor/1024x1024/apps/nix-snowflake-white.png";

    if std::path::Path::new(icon).exists() {
        icon.into()
    } else {
        String::new()
    }
}
