//! Where the compositor frosts behind the card: a region of rectangles, one
//! strip set per module, since niri's blur region is rectangles. A rounded
//! shape is approximated by horizontal strips that step in at the corners,
//! each inset as the curve is at the strip's middle, so the frost strays
//! past the edge by at most half a step and falls short by as much; the
//! blur and the module's shadow soften the steps away.

use amane::InputArea;

/// How many strips approximate a corner: a capsule a cell tall has a
/// 37 px radius, and eight keep the steps under 5 px.
pub(super) const STEPS: usize = 8;

/// How far each corner band reaches into the next, in logical pixels.
const OVERLAP: f32 = 1.0;

/// The strips inside a rounded rectangle at (`x`, `y`), `width` by `height`,
/// with `radius` corners, `steps` strips per corner.
pub(super) fn rounded(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    steps: usize,
) -> Vec<InputArea> {
    let radius = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    let mut strips = Vec::with_capacity(steps * 2 + 1);
    if width <= 0.0 || height <= 0.0 {
        return strips;
    }
    if radius < 1.0 || steps == 0 {
        strips.push(area(x, y, width, height));
        return strips;
    }

    // each corner band spans [top, bottom) from the shape's edge inwards;
    // its inset is the circle's at the band's middle. Bands overlap the
    // next by a pixel: the compositor rounds each rectangle to the screen's
    // pixels on its own, and two that only touch can leave a hairline of
    // sharp desktop between them
    let band = radius / steps as f32;
    for i in 0..steps {
        let outer = band * i as f32;
        let middle = outer + band / 2.0;
        let inset = radius - (radius * radius - (radius - middle).powi(2)).sqrt();
        let top = y + outer;
        strips.push(area(x + inset, top, width - inset * 2.0, band + OVERLAP));
        let bottom_top = y + height - outer - band - OVERLAP;
        strips.push(area(
            x + inset,
            bottom_top,
            width - inset * 2.0,
            band + OVERLAP,
        ));
    }
    strips.push(area(x, y + radius, width, height - radius * 2.0));
    strips
}

/// The frost shows only behind what's at least this visible: niri can't
/// fade its frost, so frost under a control that's faded away reads as a
/// blurred patch with nothing on it.
pub(super) const FROST_FROM: f32 = 0.5;

/// `areas` as they are once the card is drawn at `scale` about its
/// bottom-right corner (`width`, `height` from the surface's origin), as
/// it is while it opens and closes.
pub(super) fn scaled(areas: Vec<InputArea>, scale: f32, width: f32, height: f32) -> Vec<InputArea> {
    transformed(areas, scale, (width, height), (0.0, 0.0))
}

/// `areas` scaled by `scale` about `origin`, then moved by `offset`: where
/// a layer drawn with that transform puts them.
pub(super) fn transformed(
    areas: Vec<InputArea>,
    scale: f32,
    origin: (f32, f32),
    offset: (f32, f32),
) -> Vec<InputArea> {
    if (scale - 1.0).abs() < 0.001 && offset.0.abs() < 0.01 && offset.1.abs() < 0.01 {
        return areas;
    }
    areas
        .into_iter()
        .map(|a| {
            let x = origin.0 + (a.x as f32 - origin.0) * scale + offset.0;
            let y = origin.1 + (a.y as f32 - origin.1) * scale + offset.1;
            area(x, y, a.width as f32 * scale, a.height as f32 * scale)
        })
        .collect()
}

/// Rounded to whole pixels, shrinking rather than growing.
fn area(x: f32, y: f32, width: f32, height: f32) -> InputArea {
    let left = x.ceil();
    let top = y.ceil();
    let right = (x + width).floor();
    let bottom = (y + height).floor();
    InputArea {
        x: left as i32,
        y: top as i32,
        width: (right - left).max(0.0) as i32,
        height: (bottom - top).max(0.0) as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rounded_rectangle_is_strips_inside_it() {
        let strips = rounded(10.0, 20.0, 100.0, 60.0, 16.0, 4);
        assert_eq!(strips.len(), 9);
        for strip in &strips {
            assert!(strip.x >= 10 && strip.x + strip.width <= 110, "{strip:?}");
            assert!(strip.y >= 20 && strip.y + strip.height <= 80, "{strip:?}");
            assert!(strip.width > 0 && strip.height > 0, "{strip:?}");
        }
        // the outermost band is inset less than the radius: the curve at
        // its middle, not its edge, so the corner isn't left bare
        assert!(strips[0].x < 10 + 16);
        assert!(strips[0].x > 10);
        // the outermost band is the most inset, the middle strip not at all
        assert!(strips[0].x > strips[2].x);
        assert_eq!(strips[8].x, 10);
        assert_eq!(strips[8].width, 100);
        // top and bottom mirror each other
        assert_eq!(strips[0].x, strips[1].x);
        assert_eq!(strips[0].y, 20);
        assert_eq!(strips[1].y + strips[1].height, 80);
    }

    #[test]
    fn a_card_scaled_about_its_corner_scales_its_frost_too() {
        let one = vec![InputArea {
            x: 0,
            y: 0,
            width: 100,
            height: 200,
        }];
        assert_eq!(scaled(one.clone(), 1.0, 100.0, 200.0), one);
        let half = scaled(one, 0.5, 100.0, 200.0);
        assert_eq!(
            (half[0].x, half[0].y, half[0].width, half[0].height),
            (50, 100, 50, 100)
        );
    }

    #[test]
    fn frost_follows_a_moved_and_scaled_layer() {
        let one = vec![InputArea {
            x: 10,
            y: 10,
            width: 20,
            height: 20,
        }];
        assert_eq!(transformed(one.clone(), 1.0, (0.0, 0.0), (0.0, 0.0)), one);
        let moved = transformed(one.clone(), 1.0, (0.0, 0.0), (-5.0, 0.0));
        assert_eq!((moved[0].x, moved[0].width), (5, 20));
        // halved about (20, 20): the corner at 10 comes to 15
        let half = transformed(one, 0.5, (20.0, 20.0), (0.0, 0.0));
        assert_eq!(
            (half[0].x, half[0].y, half[0].width, half[0].height),
            (15, 15, 10, 10)
        );
    }

    #[test]
    fn a_square_corner_is_one_rectangle() {
        let strips = rounded(0.0, 0.0, 40.0, 20.0, 0.0, 4);
        assert_eq!(strips.len(), 1);
        assert_eq!(strips[0].width, 40);
        assert_eq!(strips[0].height, 20);
        assert!(rounded(0.0, 0.0, 0.0, 20.0, 4.0, 4).is_empty());
    }

    #[test]
    fn a_capsule_keeps_its_middle_strip() {
        // a radius past half the height is clamped to it
        let strips = rounded(0.0, 0.0, 164.0, 74.0, 999.0, 4);
        assert_eq!(strips.len(), 9);
        let middle = strips[8];
        assert_eq!((middle.y, middle.height), (37, 0));
    }
}
