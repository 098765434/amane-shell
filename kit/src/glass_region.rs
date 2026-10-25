//! Where the compositor frosts behind a clear surface's glass: one rounded
//! shape per piece of glass, each fading with its piece (niri-blur-shapes,
//! through amane's `BlurShape`). A compositor without shapes gets amane's
//! stand-in instead, a region of strips that steps in at the corners and
//! shows only behind what's at least half visible.
//!
//! The shapes come from [`Material`](crate::material::Material); this is
//! what's under it: the glass level, adaptive glass's readings of the
//! backdrop, and moving shapes with the layer they're under.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use amane::{Backdrop, BlurShape, Color, Service};

use crate::color::{self, with_alpha};
use crate::theme::{self, Glass, Mode};
use crate::tokens::{Key, with_key};

// TODO(token): `sys.glass.material` once Penpot has it (manual §1.1).
/// The compositor's material glass falls back on where its own isn't
/// configured: niri's liquid glass, which every config with materials has.
pub const MATERIAL: &str = "glass";

/// How much clearer than its tint Clear glass is.
const CLEAR: f32 = 0.8;

/// Whether the glass is see-through at all: not where the settings ask for
/// solid glass, nor where the system asks for higher contrast (Apple's
/// Reduce Transparency and Increase Contrast).
pub fn see_through() -> bool {
    theme::glass() != Glass::Solid && !theme::high_contrast()
}

/// `tint` as the glass level has it: clearer for Clear glass, opaque where
/// the glass isn't see-through.
pub(crate) fn level(tint: Color) -> Color {
    if !see_through() {
        return with_alpha(tint, 1.0);
    }
    // glass whose backdrop already reads as its mode needs less of its tint
    let backed = if BACKED.get() { BACKED_TINT } else { 1.0 };
    match theme::glass() {
        Glass::Clear => color::faded(tint, CLEAR * backed),
        Glass::Tinted | Glass::Solid => color::faded(tint, backed),
    }
}

// TODO(token): `sys.glass.adapt.*` once Penpot has them.
/// Over a backdrop brighter than this, small glass turns light.
const TURN_LIGHT: f32 = 0.65;
/// Over a backdrop dimmer than this, small glass turns dark again.
const TURN_DARK: f32 = 0.35;

/// How much of its tint adaptive glass keeps where its backdrop is clearly
/// on its side (brighter than [`TURN_LIGHT`] under light glass, dimmer than
/// [`TURN_DARK`] under dark). The tints are made for the worst backdrop,
/// light glass over a black wallpaper, and niri's `adaptive-tint` thickens
/// them further over a backdrop close in brightness, which adapted glass
/// always has: at the full tint, light glass over a bright sky came out
/// nearly opaque. The backdrop carries the labels' contrast there instead.
const BACKED_TINT: f32 = 0.45;

thread_local! {
    // the mode each piece of adaptive glass took last, by shape id
    static ADAPTED: RefCell<HashMap<u32, Mode>> = RefCell::new(HashMap::new());
    // whether the glass being built has a backdrop clearly on its side
    static BACKED: Cell<bool> = const { Cell::new(false) };
}

/// A shape id for the piece of glass named `name` (unique across the
/// shell's windows): the same every frame, so niri's readings of the
/// backdrop behind it (`amane::Backdrop`) can be found again.
pub fn shape_id(name: &str) -> u32 {
    // FNV-1a
    name.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

/// The mode glass `id` takes, and whether its backdrop is clearly on that
/// mode's side, past the threshold, rather than in the band between.
fn adaptation(id: u32) -> (Mode, bool) {
    let theme = Key::current().mode;
    if !theme::adaptive_glass() || !see_through() {
        return (theme, false);
    }
    let Some(luminance) = Backdrop::read().luminance(id) else {
        return (theme, false);
    };
    let mode = ADAPTED.with_borrow_mut(|adapted| {
        let before = *adapted.get(&id).unwrap_or(&theme);
        let mode = if luminance > TURN_LIGHT {
            Mode::Light
        } else if luminance < TURN_DARK {
            Mode::Dark
        } else {
            before
        };
        adapted.insert(id, mode);
        mode
    });
    let backed = match mode {
        Mode::Light => luminance > TURN_LIGHT,
        Mode::Dark => luminance < TURN_DARK,
    };
    (mode, backed)
}

/// Whether niri's latest reading puts the backdrop behind glass `id`
/// past the turn to light: bright enough that Clear glass dims over it.
pub(crate) fn bright(id: u32) -> bool {
    see_through()
        && Backdrop::read()
            .luminance(id)
            .is_some_and(|luminance| luminance > TURN_LIGHT)
}

/// Builds `f`, a piece of small glass and what's on it, in the mode glass
/// `id` takes ([`adaptation`]): its tint and labels flip together, and the
/// tint thins where the backdrop is clearly on its side ([`BACKED_TINT`]).
pub(crate) fn adapted<T>(id: u32, f: impl FnOnce() -> T) -> T {
    let (mode, backed) = adaptation(id);
    let key = Key {
        mode,
        ..Key::current()
    };
    let before = BACKED.replace(backed);
    let out = with_key(key, f);
    BACKED.set(before);
    out
}

/// `shapes` scaled by `scale` about `origin`, then moved by `offset`: where
/// a layer drawn with that transform puts them.
pub fn transformed(
    shapes: Vec<BlurShape>,
    scale: f32,
    origin: (f32, f32),
    offset: (f32, f32),
) -> Vec<BlurShape> {
    shapes
        .into_iter()
        .map(|shape| shape.scale_about(scale, origin).offset(offset.0, offset.1))
        .collect()
}

/// `shapes` under a layer at `opacity`.
pub fn faded(shapes: Vec<BlurShape>, opacity: f32) -> Vec<BlurShape> {
    shapes
        .into_iter()
        .map(|shape| shape.fade(opacity))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_scaled_about_its_corner_scales_its_frost_too() {
        let one = vec![BlurShape::new(0.0, 0.0, 100.0, 200.0, 20.0)];
        let corner = (100.0, 200.0);
        assert_eq!(transformed(one.clone(), 1.0, corner, (0.0, 0.0)), one);
        let half = &transformed(one, 0.5, corner, (0.0, 0.0))[0];
        assert_eq!(
            (
                half.x(),
                half.y(),
                half.width(),
                half.height(),
                half.radius()
            ),
            (50.0, 100.0, 50.0, 100.0, 10.0)
        );
    }

    #[test]
    fn frost_follows_a_moved_and_scaled_layer() {
        let one = vec![BlurShape::new(10.0, 10.0, 20.0, 20.0, 4.0)];
        let moved = &transformed(one.clone(), 1.0, (0.0, 0.0), (-5.0, 0.0))[0];
        assert_eq!((moved.x(), moved.width()), (5.0, 20.0));
        // halved about (20, 20): the corner at 10 comes to 15
        let half = &transformed(one, 0.5, (20.0, 20.0), (0.0, 0.0))[0];
        assert_eq!(
            (half.x(), half.y(), half.width(), half.height()),
            (15.0, 15.0, 10.0, 10.0)
        );
    }

    #[test]
    fn frost_fades_with_its_layer() {
        let shapes = vec![BlurShape::new(0.0, 0.0, 10.0, 10.0, 0.0).alpha(0.5)];
        assert_eq!(faded(shapes, 0.5)[0].opacity(), 0.25);
    }
}
