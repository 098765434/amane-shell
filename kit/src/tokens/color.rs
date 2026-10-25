//! CSS colours as Penpot's tinycolor reads them: hex (CSS channel order),
//! `rgb()`/`rgba()`, `hsl()`/`hsla()` and named colours. `rgba(<colour>, α)`
//! replaces a colour's alpha, which is how translucent roles alias the
//! palette.

use amane::Color;

/// sRGB with every channel in 0..=1. Kept in floats until the reader hands
/// an `amane::Color` out, so a value is rounded to 8 bits exactly once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    pub const WHITE: Rgba = Rgba::opaque(1.0, 1.0, 1.0);
    pub const BLACK: Rgba = Rgba::opaque(0.0, 0.0, 0.0);

    pub const fn opaque(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// An opaque colour from 8-bit channels.
    pub fn from_rgb8([r, g, b]: [u8; 3]) -> Self {
        let unit = |c: u8| f64::from(c) / 255.0;
        Self::opaque(unit(r), unit(g), unit(b))
    }

    pub fn to_color(self) -> Color {
        let byte = |c: f64| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color::rgba(byte(self.r), byte(self.g), byte(self.b), byte(self.a))
    }

    /// `#rrggbb`, or `#rrggbbaa` when not opaque.
    pub fn hex(self) -> String {
        let c = self.to_color();
        let rgb = format!("#{:02x}{:02x}{:02x}", c.red(), c.green(), c.blue());
        if c.alpha() == 255 {
            rgb
        } else {
            format!("{rgb}{:02x}", c.alpha())
        }
    }

    /// `self` painted over an opaque `backdrop`.
    pub fn over(self, backdrop: Rgba) -> Rgba {
        let mix = |top: f64, bottom: f64| top * self.a + bottom * (1.0 - self.a);
        Rgba::opaque(
            mix(self.r, backdrop.r),
            mix(self.g, backdrop.g),
            mix(self.b, backdrop.b),
        )
    }

    /// WCAG 2 relative luminance (alpha ignored).
    pub fn luminance(self) -> f64 {
        0.2126 * to_linear(self.r) + 0.7152 * to_linear(self.g) + 0.0722 * to_linear(self.b)
    }

    /// WCAG 2 contrast ratio between two opaque colours.
    pub fn contrast(self, other: Rgba) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// CIE L* (0 black … 100 white).
    pub fn lightness(self) -> f64 {
        let y = self.luminance();
        if y <= 216.0 / 24389.0 {
            y * 24389.0 / 27.0
        } else {
            116.0 * y.cbrt() - 16.0
        }
    }
}

pub fn parse(src: &str) -> Result<Rgba, String> {
    let s = src.trim();
    let lower = s.to_ascii_lowercase();

    if let Some(hex) = lower.strip_prefix('#') {
        return parse_hex(hex).ok_or_else(|| format!("bad hex colour `{s}`"));
    }

    if let Some((name, args)) = function(&lower) {
        return match name {
            "rgb" | "rgba" => parse_rgb(args),
            "hsl" | "hsla" => parse_hsl(args),
            _ => Err(format!("unknown colour function `{name}()`")),
        }
        .map_err(|e| format!("{e} in `{s}`"));
    }

    if lower == "transparent" {
        return Ok(Rgba {
            a: 0.0,
            ..Rgba::BLACK
        });
    }

    named(&lower)
        .map(|rgb| {
            let ch = |shift: u32| ((rgb >> shift) & 0xff) as f64 / 255.0;
            Rgba::opaque(ch(16), ch(8), ch(0))
        })
        .ok_or_else(|| format!("`{s}` is not a colour"))
}

fn function(s: &str) -> Option<(&str, &str)> {
    let open = s.find('(')?;
    let inner = s.strip_suffix(')')?;
    Some((s[..open].trim(), &inner[open + 1..]))
}

fn parse_hex(hex: &str) -> Option<Rgba> {
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let digit = |i: usize| {
        u8::from_str_radix(&hex[i..=i], 16)
            .ok()
            .map(|d| d as f64 * 17.0 / 255.0)
    };
    let pair = |i: usize| {
        u8::from_str_radix(&hex[i..i + 2], 16)
            .ok()
            .map(|d| d as f64 / 255.0)
    };

    match hex.len() {
        3 => Some(Rgba::opaque(digit(0)?, digit(1)?, digit(2)?)),
        4 => Some(Rgba {
            a: digit(3)?,
            ..Rgba::opaque(digit(0)?, digit(1)?, digit(2)?)
        }),
        6 => Some(Rgba::opaque(pair(0)?, pair(2)?, pair(4)?)),
        8 => Some(Rgba {
            a: pair(6)?,
            ..Rgba::opaque(pair(0)?, pair(2)?, pair(4)?)
        }),
        _ => None,
    }
}

/// Splits `a, b, c[, d]` or `a b c [/ d]` into parts.
fn args(s: &str) -> Vec<String> {
    let (channels, alpha) = match s.split_once('/') {
        Some((channels, alpha)) => (channels, Some(alpha.trim())),
        None => (s, None),
    };

    let mut parts: Vec<String> = if channels.contains(',') {
        channels.split(',').map(|p| p.trim().to_string()).collect()
    } else {
        channels.split_whitespace().map(String::from).collect()
    };
    parts.extend(alpha.map(String::from));
    parts
}

fn channel(part: &str, scale: f64) -> Result<f64, String> {
    let (num, div) = match part.strip_suffix('%') {
        Some(pct) => (pct, 100.0),
        None => (part, scale),
    };
    num.trim()
        .parse::<f64>()
        .map(|n| (n / div).clamp(0.0, 1.0))
        .map_err(|_| format!("bad channel `{part}`"))
}

fn parse_rgb(inner: &str) -> Result<Rgba, String> {
    // rgba(<colour>, α): a colour (often a substituted reference) with a new alpha
    if let Some((first, alpha)) = inner.rsplit_once(',')
        && let Ok(base) = parse(first)
    {
        return Ok(Rgba {
            a: channel(alpha, 1.0)?,
            ..base
        });
    }

    match args(inner).as_slice() {
        [r, g, b] => Ok(Rgba::opaque(
            channel(r, 255.0)?,
            channel(g, 255.0)?,
            channel(b, 255.0)?,
        )),
        [r, g, b, a] => Ok(Rgba {
            a: channel(a, 1.0)?,
            ..Rgba::opaque(channel(r, 255.0)?, channel(g, 255.0)?, channel(b, 255.0)?)
        }),
        _ => Err(String::from("rgb() takes 3 or 4 values")),
    }
}

fn parse_hsl(inner: &str) -> Result<Rgba, String> {
    let parts = args(inner);
    let (h, s, l, a) = match parts.as_slice() {
        [h, s, l] => (h, s, l, None),
        [h, s, l, a] => (h, s, l, Some(a)),
        _ => return Err(String::from("hsl() takes 3 or 4 values")),
    };

    let h = h
        .trim_end_matches("deg")
        .parse::<f64>()
        .map_err(|_| format!("bad hue `{h}`"))?
        .rem_euclid(360.0);
    let s = channel(s, 100.0)?;
    let l = channel(l, 100.0)?;
    let a = a.map(|a| channel(a, 1.0)).transpose()?.unwrap_or(1.0);

    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    Ok(Rgba {
        a,
        ..Rgba::opaque(r + m, g + m, b + m)
    })
}

fn named(name: &str) -> Option<u32> {
    NAMED
        .binary_search_by(|(n, _)| n.cmp(&name))
        .ok()
        .map(|i| NAMED[i].1)
}

/// CSS Color Module Level 4 named colours, sorted for binary search.
#[rustfmt::skip]
const NAMED: &[(&str, u32)] = &[
    ("aliceblue", 0xf0f8ff), ("antiquewhite", 0xfaebd7), ("aqua", 0x00ffff),
    ("aquamarine", 0x7fffd4), ("azure", 0xf0ffff), ("beige", 0xf5f5dc), ("bisque", 0xffe4c4),
    ("black", 0x000000), ("blanchedalmond", 0xffebcd), ("blue", 0x0000ff),
    ("blueviolet", 0x8a2be2), ("brown", 0xa52a2a), ("burlywood", 0xdeb887),
    ("cadetblue", 0x5f9ea0), ("chartreuse", 0x7fff00), ("chocolate", 0xd2691e),
    ("coral", 0xff7f50), ("cornflowerblue", 0x6495ed), ("cornsilk", 0xfff8dc),
    ("crimson", 0xdc143c), ("cyan", 0x00ffff), ("darkblue", 0x00008b), ("darkcyan", 0x008b8b),
    ("darkgoldenrod", 0xb8860b), ("darkgray", 0xa9a9a9), ("darkgreen", 0x006400),
    ("darkgrey", 0xa9a9a9), ("darkkhaki", 0xbdb76b), ("darkmagenta", 0x8b008b),
    ("darkolivegreen", 0x556b2f), ("darkorange", 0xff8c00), ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000), ("darksalmon", 0xe9967a), ("darkseagreen", 0x8fbc8f),
    ("darkslateblue", 0x483d8b), ("darkslategray", 0x2f4f4f), ("darkslategrey", 0x2f4f4f),
    ("darkturquoise", 0x00ced1), ("darkviolet", 0x9400d3), ("deeppink", 0xff1493),
    ("deepskyblue", 0x00bfff), ("dimgray", 0x696969), ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff), ("firebrick", 0xb22222), ("floralwhite", 0xfffaf0),
    ("forestgreen", 0x228b22), ("fuchsia", 0xff00ff), ("gainsboro", 0xdcdcdc),
    ("ghostwhite", 0xf8f8ff), ("gold", 0xffd700), ("goldenrod", 0xdaa520), ("gray", 0x808080),
    ("green", 0x008000), ("greenyellow", 0xadff2f), ("grey", 0x808080), ("honeydew", 0xf0fff0),
    ("hotpink", 0xff69b4), ("indianred", 0xcd5c5c), ("indigo", 0x4b0082), ("ivory", 0xfffff0),
    ("khaki", 0xf0e68c), ("lavender", 0xe6e6fa), ("lavenderblush", 0xfff0f5),
    ("lawngreen", 0x7cfc00), ("lemonchiffon", 0xfffacd), ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080), ("lightcyan", 0xe0ffff), ("lightgoldenrodyellow", 0xfafad2),
    ("lightgray", 0xd3d3d3), ("lightgreen", 0x90ee90), ("lightgrey", 0xd3d3d3),
    ("lightpink", 0xffb6c1), ("lightsalmon", 0xffa07a), ("lightseagreen", 0x20b2aa),
    ("lightskyblue", 0x87cefa), ("lightslategray", 0x778899), ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de), ("lightyellow", 0xffffe0), ("lime", 0x00ff00),
    ("limegreen", 0x32cd32), ("linen", 0xfaf0e6), ("magenta", 0xff00ff), ("maroon", 0x800000),
    ("mediumaquamarine", 0x66cdaa), ("mediumblue", 0x0000cd), ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db), ("mediumseagreen", 0x3cb371), ("mediumslateblue", 0x7b68ee),
    ("mediumspringgreen", 0x00fa9a), ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585), ("midnightblue", 0x191970), ("mintcream", 0xf5fffa),
    ("mistyrose", 0xffe4e1), ("moccasin", 0xffe4b5), ("navajowhite", 0xffdead),
    ("navy", 0x000080), ("oldlace", 0xfdf5e6), ("olive", 0x808000), ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500), ("orangered", 0xff4500), ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa), ("palegreen", 0x98fb98), ("paleturquoise", 0xafeeee),
    ("palevioletred", 0xdb7093), ("papayawhip", 0xffefd5), ("peachpuff", 0xffdab9),
    ("peru", 0xcd853f), ("pink", 0xffc0cb), ("plum", 0xdda0dd), ("powderblue", 0xb0e0e6),
    ("purple", 0x800080), ("rebeccapurple", 0x663399), ("red", 0xff0000),
    ("rosybrown", 0xbc8f8f), ("royalblue", 0x4169e1), ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072), ("sandybrown", 0xf4a460), ("seagreen", 0x2e8b57),
    ("seashell", 0xfff5ee), ("sienna", 0xa0522d), ("silver", 0xc0c0c0), ("skyblue", 0x87ceeb),
    ("slateblue", 0x6a5acd), ("slategray", 0x708090), ("slategrey", 0x708090),
    ("snow", 0xfffafa), ("springgreen", 0x00ff7f), ("steelblue", 0x4682b4), ("tan", 0xd2b48c),
    ("teal", 0x008080), ("thistle", 0xd8bfd8), ("tomato", 0xff6347), ("turquoise", 0x40e0d0),
    ("violet", 0xee82ee), ("wheat", 0xf5deb3), ("white", 0xffffff), ("whitesmoke", 0xf5f5f5),
    ("yellow", 0xffff00), ("yellowgreen", 0x9acd32),
];

/// An sRGB channel (0..=1) in linear light.
pub fn to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The largest difference between two colours in any 8-bit channel.
#[cfg(test)]
pub fn byte_distance(a: Color, b: Color) -> u8 {
    [
        a.red().abs_diff(b.red()),
        a.green().abs_diff(b.green()),
        a.blue().abs_diff(b.blue()),
        a.alpha().abs_diff(b.alpha()),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(src: &str) -> String {
        parse(src).unwrap_or_else(|e| panic!("{src}: {e}")).hex()
    }

    #[test]
    fn forms() {
        assert_eq!(hex("#0A84FF"), "#0a84ff");
        assert_eq!(hex("#fff"), "#ffffff");
        assert_eq!(hex("#0008"), "#00000088");
        assert_eq!(hex("#00000080"), "#00000080");
        assert_eq!(hex("rgb(10, 132, 255)"), "#0a84ff");
        assert_eq!(hex("rgba(0, 0, 0, 0.5)"), "#00000080");
        assert_eq!(hex("rgb(0 0 0 / 50%)"), "#00000080");
        assert_eq!(hex("hsl(0, 100%, 50%)"), "#ff0000");
        assert_eq!(hex("hsla(120, 100%, 25%, 1)"), "#008000");
        assert_eq!(hex("rebeccapurple"), "#663399");
        assert_eq!(hex("transparent"), "#00000000");
    }

    #[test]
    fn rgba_over_a_colour() {
        assert_eq!(hex("rgba(#ffffff, 0.08)"), "#ffffff14");
        assert_eq!(hex("rgba(red, 0.5)"), "#ff000080");
    }

    #[test]
    fn rejects() {
        for bad in ["#12", "#ggg", "rgb(1, 2)", "blurple", "", "cmyk(0,0,0,0)"] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn named_table_is_sorted() {
        assert!(NAMED.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn lightness() {
        assert_eq!(parse("#1d1d1f").unwrap().lightness().round(), 11.0);
        assert_eq!(parse("#f5f5f7").unwrap().lightness().round(), 97.0);
        assert_eq!(parse("#ff453a").unwrap().lightness().round(), 58.0);
    }
}
