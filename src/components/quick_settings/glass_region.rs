//! Where the compositor frosts behind the card: one rounded shape per
//! module, each fading with its module (niri-blur-shapes, through amane's
//! `BlurShape`). A compositor without shapes gets amane's stand-in instead,
//! a region of strips that steps in at the corners and shows only behind
//! what's at least half visible.

use amane::{BlurShape, Color};

use crate::components::card::Surface;

// TODO(token): `quick.glass.material` once Penpot has it (manual §1.1).
/// The compositor's material the modules ask for: niri's liquid glass where
/// its config names one so, plain frost otherwise.
pub(super) const MATERIAL: &str = "glass";

/// Whether the compositor draws the modules' glass itself (their tint and
/// lit edge, with the frost, on one edge): the modules then leave both out.
pub(super) fn compositor_glass() -> bool {
    amane::material_available(MATERIAL)
}

/// The frost behind a module drawn at (`x`, `y`), `width` by `height`, with
/// `radius` corners, at `opacity`.
pub(super) fn module(
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
        .tint(tint())
}

/// The modules' own tint, for a material that lays it on with the frost.
fn tint() -> Color {
    Surface::Panel.bg()
}

/// `shapes` as they are once the card is drawn at `scale` about its
/// bottom-right corner (`width`, `height` from the surface's origin), as
/// it is while it opens and closes.
pub(super) fn scaled(
    shapes: Vec<BlurShape>,
    scale: f32,
    width: f32,
    height: f32,
) -> Vec<BlurShape> {
    transformed(shapes, scale, (width, height), (0.0, 0.0))
}

/// `shapes` scaled by `scale` about `origin`, then moved by `offset`: where
/// a layer drawn with that transform puts them.
pub(super) fn transformed(
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
pub(super) fn faded(shapes: Vec<BlurShape>, opacity: f32) -> Vec<BlurShape> {
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
        assert_eq!(scaled(one.clone(), 1.0, 100.0, 200.0), one);
        let half = &scaled(one, 0.5, 100.0, 200.0)[0];
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
