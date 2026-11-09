//! Glass — the edge light every translucent surface wears: the bar's panels,
//! the notification center, the toasts and the context menus.
//!
//! Over niri's frost and the surface's own tint, a shader (`glass.wgsl`)
//! draws a thin rim lit from the top-left, a fainter catch on the far edge
//! and a soft bloom inside the lit edge, and an inner shadow adds an even
//! glow. The drop shadow under a surface is niri's
//! (the shell's `tokens::layer_rules`): each surface is only as big
//! as its card, so a shadow drawn here would be cut off.
//!
//! A surface holding many glass pieces (Quick Settings' modules) lights
//! them all in one pass ([`rims`]): a shader pass costs the same whatever
//! it covers, and one per module cost more than a frame.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use amane::{Color, Rectangle, Shadow};

use crate::theme::Mode;
use crate::tokens::{Key, sys};

// The rim's thickness, how far in from the lit edge the bloom reaches and
// the even glow's blur are `sys.effect.glass.*`; the rim's and glow's
// colours are `sys.color.glass.*`.
/// How many pieces one [`rims`] pass lights at most: what the shader's
/// values hold (amane gives 64 rows; three carry the light).
pub const RIMS_PER_PASS: usize = 30;

/// The light on one mode's surfaces.
struct Light {
    /// The rim where it faces the light; its alpha is how strong it is.
    rim: Color,
    /// The catch on the far edge, against the rim.
    far: f32,
    /// The bloom's strength, in the rim's colour.
    bloom: f32,
    /// The even glow along the whole inside edge.
    glow: Color,
}

impl Light {
    /// This mode's light: barely there, the edge felt more than seen.
    fn now() -> Self {
        let (far, bloom) = match Key::current().mode {
            Mode::Light => (0.3, 0.08),
            Mode::Dark => (0.1, 0.01),
        };
        Self {
            rim: sys::color::glass::rim(),
            far,
            bloom,
            glow: sys::color::glass::glow(),
        }
    }
}

const SOURCE: &str = include_str!("glass.wgsl");

/// `surface`, its corners `radius`, in glass. Replaces any shadow it has.
pub fn apply(surface: Rectangle, radius: f32) -> Rectangle {
    let light = &Light::now();

    let surface = surface.shadow(Shadow::inner(light.glow).blur(sys::effect::glass::glow_blur()));

    let Some(shader) = shader() else {
        return surface;
    };

    // the whole surface is the one piece; its glow is the inner shadow's
    let mut values = header(light, 1, 0.0);
    values.push([0.0, 0.0, 0.0, 0.0]);
    values.push([radius, 1.0, 0.0, 0.0]);
    surface.shader(shader).shader_values(values)
}

/// One glass piece among many on a surface: where it is on the surface,
/// how round, and how far it has faded in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rim {
    /// Its left edge on the surface, in logical pixels.
    pub x: f32,
    /// Its top edge on the surface, in logical pixels.
    pub y: f32,
    /// Its width in logical pixels.
    pub width: f32,
    /// Its height in logical pixels.
    pub height: f32,
    /// Its corner radius in logical pixels.
    pub radius: f32,
    /// How far it has faded in, 0 to 1.
    pub opacity: f32,
}

/// A clear layer the size of its parent that lights every piece in `rims`
/// at once, to draw over them. More than [`RIMS_PER_PASS`] pieces take a
/// layer each; see [`rim_layers`].
pub fn rims(rims: &[Rim]) -> Rectangle {
    let layer = Rectangle::new().width(amane::Parent).height(amane::Parent);
    let Some(shader) = shader() else {
        return layer;
    };
    let light = &Light::now();
    let rims = &rims[..rims.len().min(RIMS_PER_PASS)];
    // no glow: the shader's stand-in for the panels' inner shadow read as a
    // band inside small modules; the lit rim, far catch and bloom carry it
    let mut values = header(light, rims.len(), 0.0);
    for rim in rims {
        values.push([rim.x, rim.y, rim.width, rim.height]);
        values.push([rim.radius, rim.opacity.clamp(0.0, 1.0), 0.0, 0.0]);
    }
    layer.shader(shader).shader_values(values)
}

/// The layers that light all of `rims`, [`RIMS_PER_PASS`] to a layer.
pub fn rim_layers(rims: &[Rim]) -> Vec<Rectangle> {
    rims.chunks(RIMS_PER_PASS).map(rims_layer).collect()
}

fn rims_layer(chunk: &[Rim]) -> Rectangle {
    rims(chunk)
}

/// The shader's first three rows: how many pieces, and `light`'s strengths.
/// `glow_width` 0 leaves the glow to an inner shadow.
fn header(light: &Light, count: usize, glow_width: f32) -> Vec<[f32; 4]> {
    let channel = |value: u8| f32::from(value) / 255.0;
    let rim = light.rim;
    let glow = if glow_width > 0.0 {
        channel(light.glow.alpha())
    } else {
        0.0
    };
    vec![
        [
            count as f32,
            sys::effect::glass::rim_width(),
            channel(rim.alpha()),
            light.bloom,
        ],
        [
            sys::effect::glass::bloom_width(),
            light.far,
            glow,
            glow_width,
        ],
        [
            channel(rim.red()),
            channel(rim.green()),
            channel(rim.blue()),
            0.0,
        ],
    ]
}

/// The shader, written once into the runtime dir: amane loads shaders from
/// a file, and the shell ships as one binary. `None` if it can't be
/// written; the surfaces then keep their glow and go without the rim.
fn shader() -> Option<&'static Path> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

    PATH.get_or_init(|| {
        let dir = crate::paths::runtime().join(crate::app_ids::SHELL);
        // named for its text, so the shell and Emaki Settings of different builds don't
        // overwrite each other's, and written to a part file first, so neither ever loads
        // half of one
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(SOURCE, &mut hasher);
        let path = dir.join(format!(
            "glass-{:016x}.wgsl",
            std::hash::Hasher::finish(&hasher)
        ));
        let written = crate::fs::write_whole(&path, SOURCE);
        match written {
            Ok(()) => Some(path),
            Err(e) => {
                log::warn!("{}: {e}", path.display());
                None
            }
        }
    })
    .as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;

    // amane redraws a shader every frame if the word appears anywhere in it
    #[test]
    fn the_shader_never_names_the_clock() {
        let is_name_part = |c: char| c.is_alphanumeric() || c == '_';
        assert!(
            !SOURCE
                .split(|c| !is_name_part(c))
                .any(|word| word == "time")
        );
    }
}
