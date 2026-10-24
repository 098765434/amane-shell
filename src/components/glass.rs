//! Glass — the edge light every translucent surface wears: the bar's panels,
//! the notification center, the toasts and the context menus.
//!
//! Over niri's frost and the surface's own tint, a shader (`glass.wgsl`)
//! draws a thin rim lit from the top-left, a fainter catch on the far edge
//! and a soft bloom inside the lit edge, and an inner shadow adds an even
//! glow. The drop shadow under a surface is niri's
//! ([`layer_rules`](crate::tokens::layer_rules)): each surface is only as big
//! as its card, so a shadow drawn here would be cut off.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use amane::{Color, Rectangle, Shadow};

use crate::theme::{self, Mode};

// TODO(token): these become `sys.color.glass.*` and `sys.effect.glass.*`
// tokens once Penpot has them (docs/penpot-todo.md; manual §1.1: no style
// literals in component code).
/// The rim's thickness.
const RIM_WIDTH: f32 = 1.5;
/// How far in from the lit edge the bloom reaches.
const BLOOM_WIDTH: f32 = 14.0;
/// The even glow's blur.
const GLOW_BLUR: f32 = 5.0;

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

// barely there: the edge should be felt more than seen
const LIGHT: Light = Light {
    rim: Color::rgba(255, 255, 255, 90),
    far: 0.3,
    bloom: 0.08,
    glow: Color::rgba(255, 255, 255, 50),
};

const DARK: Light = Light {
    rim: Color::rgba(255, 255, 255, 22),
    far: 0.1,
    bloom: 0.01,
    glow: Color::rgba(255, 255, 255, 5),
};

const SOURCE: &str = include_str!("glass.wgsl");

/// `surface`, its corners `radius`, in glass. Replaces any shadow it has.
pub fn apply(surface: Rectangle, radius: f32) -> Rectangle {
    let light = match theme::mode() {
        Mode::Light => &LIGHT,
        Mode::Dark => &DARK,
    };

    let surface = surface.shadow(Shadow::inner(light.glow).blur(GLOW_BLUR));

    let Some(shader) = shader() else {
        return surface;
    };

    let channel = |value: u8| f32::from(value) / 255.0;
    let rim = light.rim;

    surface.shader(shader).shader_values(vec![
        [radius, RIM_WIDTH, channel(rim.alpha()), light.bloom],
        [BLOOM_WIDTH, light.far, 0.0, 0.0],
        [
            channel(rim.red()),
            channel(rim.green()),
            channel(rim.blue()),
            0.0,
        ],
    ])
}

/// The shader, written once into the runtime dir: amane loads shaders from
/// a file, and the shell ships as one binary. `None` if it can't be
/// written; the surfaces then keep their glow and go without the rim.
fn shader() -> Option<&'static Path> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

    PATH.get_or_init(|| {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("amane-shell");
        let path = dir.join("glass.wgsl");

        match std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, SOURCE)) {
            Ok(()) => Some(path),
            Err(e) => {
                eprintln!("amane-shell: {}: {e}", path.display());
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
