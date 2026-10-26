//! The Shader Lab's effects and saved looks, for the shell.
//!
//! The lab (`apps/shader-lab`) saves each look as
//! `apps/shader-lab/configs/<name>.json`. The build embeds every config and
//! every effect folder, so one line puts a saved look on a rectangle:
//!
//! ```rust,ignore
//! use amane_kit::lab;
//!
//! let card = Rectangle::new().width(320.0).height(200.0).radius(24.0);
//!
//! match lab::look("dock-glass", 320.0, 200.0, 24.0) {
//!     Some(look) => boxed(look.apply(card)),
//!     None => boxed(card),
//! }
//! ```

pub mod config;
pub mod manifest;
pub mod shadows;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use amane::{Color, Effect, Rectangle, Shadow, Stack, Widget};

use crate::color_picker::to_color;
use crate::layout::boxed;
use config::{Config, ShadowSpec, slug};
use manifest::{Manifest, normal, read_manifest};

mod files {
    include!(concat!(env!("OUT_DIR"), "/lab_files.rs"));
}

/// Every embedded file, by its path under `apps/shader-lab`, like
/// `effects/chrome/chrome.wgsl` or `configs/dock-glass.json`.
pub fn files() -> &'static [(&'static str, &'static str)] {
    files::FILES
}

pub fn file(name: &str) -> Option<&'static str> {
    files::FILES
        .iter()
        .find(|(path, _)| *path == name)
        .map(|(_, text)| *text)
}

/// The saved configs' names, as the lab lists them.
pub fn saved() -> Vec<String> {
    files::FILES
        .iter()
        .filter_map(|(path, _)| path.strip_prefix("configs/")?.strip_suffix(".json"))
        .map(str::to_owned)
        .collect()
}

/// A saved config by its name or file name.
pub fn config(name: &str) -> Option<Config> {
    let text = file(&format!("configs/{}.json", slug(name)))?;

    serde_json::from_str(text)
        .map_err(|error| eprintln!("amane-kit: lab config {name}: {error}"))
        .ok()
}

/// An embedded effect's manifest, with what it `extends` filled in.
pub fn manifest(effect: &str) -> Option<Manifest> {
    let read = |path: &Path| -> Result<String, String> {
        let name = normal(path).to_string_lossy().replace('\\', "/");

        file(&name)
            .map(str::to_owned)
            .ok_or_else(|| format!("{name} isn't embedded"))
    };

    let value = read_manifest(Path::new("effects").join(effect).as_path(), &read, 0)
        .map_err(|error| eprintln!("amane-kit: lab effect {effect}: {error}"))
        .ok()?;

    let manifest: Manifest = serde_json::from_value(value)
        .map_err(|error| eprintln!("amane-kit: lab effect {effect}: {error}"))
        .ok()?;

    manifest
        .check()
        .map_err(|error| eprintln!("amane-kit: lab effect {effect}: {error}"))
        .ok()?;

    Some(manifest)
}

/// A saved look, built for a `width` by `height` rectangle with corners `radius`.
pub struct Look {
    effect: Option<Effect>,
    shader: Option<(PathBuf, Vec<[f32; 4]>)>,
    surface: Option<Surface>,
    shadows: Vec<ShadowSpec>,
    width: f32,
    height: f32,
    radius: f32,
}

/// A translucent surface like the shell's panels, for looks made with one.
struct Surface {
    tint: Color,
    glow: Color,
    glow_blur: f32,
    frost: f32,
}

/// The look saved as `name`, for a `width` by `height` rectangle with
/// corners `radius`; none if it isn't saved or its effect won't load.
pub fn look(name: &str, width: f32, height: f32, radius: f32) -> Option<Look> {
    let config = config(name)?;
    let manifest = manifest(&config.effect)?;
    let dir = written()?.join("effects").join(&config.effect);

    let mut values = config.rows(&manifest);

    let surface = manifest.surface.as_ref().map(|spec| {
        manifest.put_surface(&mut values, spec, width, height, radius);

        let number = |key: &str| {
            let param = manifest.params.iter().find(|param| param.key == key);

            param.map_or(0.0, |param| config.value(param))
        };

        let color = |key: &str| {
            let param = manifest.colors.iter().find(|param| param.key == key);

            to_color(param.map_or([0.0; 4], |param| config.color(param)))
        };

        Surface {
            tint: color(&spec.tint),
            glow: color(&spec.glow),
            glow_blur: number(&spec.glow_blur),
            frost: number(&spec.frost),
        }
    });

    let shader = manifest
        .shader
        .as_ref()
        .map(|shader| (dir.join(shader), values.clone()));

    Some(Look {
        effect: manifest.effect(&dir, values),
        shader,
        surface,
        shadows: config.shadows,
        width,
        height,
        radius,
    })
}

impl Look {
    /// `rectangle` with the look, its shadows layered around it.
    pub fn apply(self, rectangle: Rectangle) -> Stack {
        let behind = shadows::behind(&self.shadows, self.width, self.height, self.radius);
        let in_front = shadows::in_front(&self.shadows, self.width, self.height, self.radius);

        let mut layers: Vec<Box<dyn Widget>> = Vec::new();

        layers.extend(behind.map(boxed));
        layers.push(boxed(self.style(rectangle)));
        layers.extend(in_front.map(boxed));

        Stack::new(layers)
    }

    /// `rectangle` with the look but no shadows.
    pub fn style(self, mut rectangle: Rectangle) -> Rectangle {
        if let Some(surface) = self.surface {
            rectangle = rectangle
                .fill(surface.tint)
                .blur(surface.frost)
                .shadow(Shadow::inner(surface.glow).blur(surface.glow_blur));
        }

        if let Some((shader, values)) = self.shader {
            rectangle = rectangle.shader(shader).shader_values(values);
        }

        if let Some(effect) = self.effect {
            rectangle = rectangle.effect(effect);
        }

        rectangle
    }
}

/// The embedded effects written out once, as amane loads shaders from
/// files: `$XDG_RUNTIME_DIR/amane-shell/lab`; none if it can't be written.
fn written() -> Option<&'static Path> {
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();

    ROOT.get_or_init(|| {
        let root = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("amane-shell")
            .join("lab");

        write_effects(&root)
            .map_err(|error| eprintln!("amane-kit: {}: {error}", root.display()))
            .ok()?;

        Some(root)
    })
    .as_deref()
}

/// Writes every embedded effect folder under `root/effects`.
pub fn write_effects(root: &Path) -> std::io::Result<()> {
    for (name, text) in files::FILES {
        if !name.starts_with("effects/") {
            continue;
        }

        let path = root.join(name);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&path, text)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_folder_is_embedded_and_sound() {
        let effects: Vec<&str> = files()
            .iter()
            .filter_map(|(path, _)| path.strip_prefix("effects/")?.strip_suffix("/effect.json"))
            .collect();

        assert!(effects.contains(&"chrome"), "{effects:?}");
        assert!(effects.contains(&"liquid-glass"), "{effects:?}");

        for effect in effects {
            let manifest = manifest(effect).unwrap_or_else(|| panic!("{effect} won't load"));

            for pass in &manifest.passes {
                let path = normal(&Path::new("effects").join(effect).join(&pass.shader));
                let name = path.to_string_lossy().replace('\\', "/");

                assert!(file(&name).is_some(), "{effect}: {name} isn't embedded");
            }

            for preset in &manifest.presets {
                for key in preset.values.keys() {
                    assert!(
                        manifest.params.iter().any(|param| &param.key == key),
                        "{effect}: {} sets {key}, which isn't a param",
                        preset.name
                    );
                }
            }
        }
    }

    #[test]
    fn the_glass_shader_is_the_kit_s_own() {
        assert_eq!(
            file("effects/glass/glass.wgsl"),
            Some(include_str!("../glass.wgsl"))
        );
    }

    #[test]
    fn a_variant_takes_what_it_leaves_out_from_what_it_extends() {
        let manifest = manifest("chrome-wallpaper").unwrap();

        assert_eq!(manifest.name, "Reflective chrome (wallpaper)");
        assert_eq!(manifest.params[0].key, "reflect");
        assert_eq!(manifest.params.last().unwrap().key, "evolution");
        assert!(manifest.gradient.is_some());
    }

    #[test]
    fn a_config_packs_its_own_gradient_and_values() {
        let manifest = manifest("chrome").unwrap();

        let mut config = Config::new("chrome");
        config.values.insert("heightIntensity".into(), 150.0);
        config.gradient.stops = Some(vec![[0.0, 1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0, 1.0]]);

        let rows = config.rows(&manifest);

        // depth, then the two stops and their count
        assert_eq!(rows[11][1], 150.0);
        assert_eq!(rows[1], [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(rows[2], [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(rows[11][0], 2.0);
    }
}
