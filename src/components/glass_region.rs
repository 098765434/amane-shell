//! Where the compositor frosts behind a clear surface's glass: one rounded
//! shape per piece of glass, each fading with its piece (niri-blur-shapes,
//! through amane's `BlurShape`). A compositor without shapes gets amane's
//! stand-in instead, a region of strips that steps in at the corners and
//! shows only behind what's at least half visible.
//!
//! Quick Settings frosts each module this way, and the dock its one card.
//! Where the compositor draws the glass material itself, it lays on the
//! tint and lit edge too, so the glass drawn in the surface is clear
//! ([`tint`]) and goes without the shader's edge ([`lit`]).

use amane::{BlurShape, Color, Rectangle};

use crate::components::card::Surface;
use crate::components::glass;

// TODO(token): `sys.glass.material` once Penpot has it (manual §1.1).
/// The compositor's material the glass asks for: niri's liquid glass where
/// its config names one so, plain frost otherwise.
pub(crate) const MATERIAL: &str = "glass";

/// Whether the compositor draws the glass itself (its tint and lit edge,
/// with the frost, on one edge): the surface then leaves both out.
pub(crate) fn compositor_glass() -> bool {
    amane::material_available(MATERIAL)
}

/// The frost behind a piece of glass drawn at (`x`, `y`), `width` by
/// `height`, with `radius` corners, at `opacity`.
pub(crate) fn module(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    opacity: f32,
) -> BlurShape {
    BlurShape::new(x, y, width, height, radius)
        .alpha(opacity)
        .material(MATERIAL)
        .tint(Surface::Panel.bg())
}

/// What the glass drawn in the surface is tinted: the panels' tint, unless
/// the compositor lays it on with the frost.
pub(crate) fn tint() -> Color {
    if compositor_glass() {
        Color::TRANSPARENT
    } else {
        Surface::Panel.bg()
    }
}

/// `surface`, a piece of glass with `radius` corners, with the lit edge the
/// compositor would otherwise draw.
pub(crate) fn lit(surface: Rectangle, radius: f32) -> Rectangle {
    if compositor_glass() {
        surface
    } else {
        glass::apply(surface, radius)
    }
}

/// `shapes` scaled by `scale` about `origin`, then moved by `offset`: where
/// a layer drawn with that transform puts them.
pub(crate) fn transformed(
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
pub(crate) fn faded(shapes: Vec<BlurShape>, opacity: f32) -> Vec<BlurShape> {
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
