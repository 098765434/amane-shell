//! The four accent primitives derived from one seed colour (manual §8.3):
//! the runtime swaps them in for Penpot's values when the system publishes
//! an accent, and everything referencing them follows.

use super::color::{Rgba, to_linear};

/// White text on the seed down to this contrast, dark text below it.
const ON_SEED_MIN_CONTRAST: f64 = 3.0;

/// OKLab lightness bounds for accent-coloured text (libadwaita's rule).
const DIM_MAX_L: f64 = 0.50;
const BRIGHT_MIN_L: f64 = 0.85;

pub const SEED: &str = "ref.color.accent.seed";
pub const ON_SEED: &str = "ref.color.accent.on-seed";
pub const DIM: &str = "ref.color.accent.dim";
pub const BRIGHT: &str = "ref.color.accent.bright";

/// Raw values for the four accent primitives, as hex strings to substitute
/// before resolving.
pub fn derive(seed: Rgba) -> [(&'static str, String); 4] {
    let seed = Rgba { a: 1.0, ..seed };

    let on_seed = if Rgba::WHITE.contrast(seed) >= ON_SEED_MIN_CONTRAST {
        Rgba::WHITE
    } else {
        Rgba {
            a: 0.8,
            ..Rgba::BLACK
        }
    };

    let [l, a, b] = to_oklab(seed);
    let dim = in_gamut([l.min(DIM_MAX_L), a, b]);
    let bright = in_gamut([l.max(BRIGHT_MIN_L), a, b]);

    [
        (SEED, seed.hex()),
        (ON_SEED, on_seed.hex()),
        (DIM, dim.hex()),
        (BRIGHT, bright.hex()),
    ]
}

fn from_linear(c: f64) -> f64 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

pub fn to_oklab(c: Rgba) -> [f64; 3] {
    let (r, g, b) = (to_linear(c.r), to_linear(c.g), to_linear(c.b));

    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();

    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

/// Back to sRGB at the same lightness and hue, reducing chroma until it fits
/// (clipping channels instead would move the lightness off its bound).
fn in_gamut([l, a, b]: [f64; 3]) -> Rgba {
    let fits = |k: f64| {
        let [r, g, b] = linear_from_oklab([l, a * k, b * k]);
        [r, g, b].iter().all(|c| (-1e-6..=1.0 + 1e-6).contains(c))
    };

    if fits(1.0) {
        return from_oklab([l, a, b]);
    }

    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..32 {
        let mid = (lo + hi) / 2.0;
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    from_oklab([l, a * lo, b * lo])
}

fn linear_from_oklab([lightness, a, b]: [f64; 3]) -> [f64; 3] {
    let l = (lightness + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (lightness - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (lightness - 0.0894841775 * a - 1.2914855480 * b).powi(3);

    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
}

/// Back to sRGB, clipping each channel into gamut.
pub fn from_oklab(lab: [f64; 3]) -> Rgba {
    let [r, g, b] = linear_from_oklab(lab);
    Rgba::opaque(from_linear(r), from_linear(g), from_linear(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::color::{self, parse};

    fn derived(seed: &str) -> [String; 4] {
        derive(parse(seed).unwrap()).map(|(_, v)| v)
    }

    fn byte_distance(a: &str, b: &str) -> u8 {
        color::byte_distance(parse(a).unwrap().to_color(), parse(b).unwrap().to_color())
    }

    #[test]
    fn default_seed_reproduces_committed_primitives() {
        let [seed, on_seed, dim, bright] = derived("#0091ff");
        assert_eq!(seed, "#0091ff");
        assert_eq!(on_seed, "#ffffff");
        assert!(byte_distance(&dim, "#0064b3") <= 1, "dim {dim}");
        assert!(byte_distance(&bright, "#a9d2ff") <= 1, "bright {bright}");
    }

    #[test]
    fn on_seed_switches_on_light_accents() {
        assert_eq!(derived("#c88800")[1], "#ffffff", "yellow keeps white");
        assert_eq!(derived("#a6e22e")[1], "#000000cc", "lime gets dark text");
    }

    #[test]
    fn dim_and_bright_bounds() {
        for seed in [
            "#0a84ff", "#c88800", "#a6e22e", "#ff453a", "#1d1d1f", "#f5f5f7",
        ] {
            let [_, _, dim, bright] = derived(seed);
            assert!(
                to_oklab(parse(&dim).unwrap())[0] <= DIM_MAX_L + 0.01,
                "{seed} dim"
            );
            assert!(
                to_oklab(parse(&bright).unwrap())[0] >= BRIGHT_MIN_L - 0.01,
                "{seed} bright"
            );
        }
    }
}
