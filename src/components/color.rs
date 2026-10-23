//! Colour arithmetic the components share; amane's `Color` only holds the
//! channels.

use amane::Color;

/// `top` painted over `bottom` (source-over), so a translucent state layer
/// can be one fill instead of another rectangle.
pub(crate) fn over(top: Color, bottom: Color) -> Color {
    let top_a = f32::from(top.alpha()) / 255.0;
    let bottom_a = f32::from(bottom.alpha()) / 255.0 * (1.0 - top_a);
    let alpha = top_a + bottom_a;
    if alpha <= 0.0 {
        return Color::TRANSPARENT;
    }

    let channel =
        |t: u8, b: u8| ((f32::from(t) * top_a + f32::from(b) * bottom_a) / alpha).round() as u8;
    Color::rgba(
        channel(top.red(), bottom.red()),
        channel(top.green(), bottom.green()),
        channel(top.blue(), bottom.blue()),
        (alpha * 255.0).round() as u8,
    )
}

/// `color` at `amount` of its own opacity.
pub(crate) fn faded(color: Color, amount: f32) -> Color {
    let alpha = (f32::from(color.alpha()) * amount.clamp(0.0, 1.0)).round() as u8;
    Color::rgba(color.red(), color.green(), color.blue(), alpha)
}

/// `color` at opacity `alpha`, whatever its own.
pub(crate) fn with_alpha(color: Color, alpha: f32) -> Color {
    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::rgba(color.red(), color.green(), color.blue(), alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn over_an_opaque_fill_stays_opaque() {
        let mixed = over(Color::rgba(255, 255, 255, 51), Color::rgb(0, 0, 0));
        assert_eq!(mixed, Color::rgba(51, 51, 51, 255));
    }

    #[test]
    fn over_transparent_is_the_top_layer() {
        let top = Color::rgba(255, 255, 255, 26);
        assert_eq!(over(top, Color::TRANSPARENT), top);
    }

    #[test]
    fn translucent_layers_add_up() {
        // 20% over 20% covers 36%
        let mixed = over(Color::rgba(0, 0, 0, 51), Color::rgba(0, 0, 0, 51));
        assert_eq!(mixed.alpha(), 92);
    }

    #[test]
    fn faded_scales_and_with_alpha_replaces() {
        let half = Color::rgba(10, 20, 30, 128);
        assert_eq!(faded(half, 0.5).alpha(), 64);
        assert_eq!(with_alpha(half, 1.0).alpha(), 255);
    }
}
