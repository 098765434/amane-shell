//! The clock's digits, each one pen stroke in a 60 × 100 box, as SVG path
//! data: parsed once, then cloned and placed for every frame.

use amane::Path;

/// How wide and tall a digit's box is, with room for a thick line's ends.
pub const WIDTH: f32 = 64.0;
pub const HEIGHT: f32 = 104.0;

/// Where the 60 × 100 drawing sits in its box.
const INSET: f32 = 2.0;

const DATA: [&str; 10] = [
    "M12 34 A18 18 0 0 1 48 34 L48 66 A18 18 0 0 1 12 66 Z",
    "M18 24 L34 12 L34 88",
    "M13 30 A17 17 0 0 1 47 30 C47 46 36 56 13 88 L48 88",
    "M14 24 C18 10 46 10 46 30 C46 42 38 48 28 48 C40 48 48 56 48 68 C48 92 16 94 12 78",
    "M40 88 L40 12 L12 62 L50 62",
    "M46 12 L17 12 L14 46 C20 40 26 38 31 38 C42 38 48 48 48 62 C48 82 36 88 28 88 C20 88 14 84 12 78",
    "M42 14 C30 26 14 44 14 64 A17 17 0 1 0 48 64 A17 17 0 1 0 14 64",
    "M12 12 L48 12 L24 88",
    "M30 48 C18 48 14 40 14 30 A16 18 0 0 1 46 30 C46 40 42 48 30 48 C16 48 12 58 12 68 A18 20 0 0 0 48 68 C48 58 44 48 30 48 Z",
    "M18 86 C30 74 46 56 46 36 A17 17 0 1 0 12 36 A17 17 0 1 0 46 36",
];

thread_local! {
    static PARSED: Vec<Path> = DATA
        .iter()
        .map(|data| Path::svg(data).expect("a digit's path parses"))
        .collect();
}

/// `digit`'s stroke with its box's top-left at (`x`, `y`), `scale` times
/// its size; unstyled.
pub fn digit(digit: u8, x: f32, y: f32, scale: f32) -> Path {
    PARSED
        .with(|parsed| parsed[usize::from(digit % 10)].clone())
        .place(x + INSET * scale, y + INSET * scale, scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_digit_parses() {
        for value in 0..10 {
            let _ = digit(value, 0.0, 0.0, 1.0);
        }
    }
}
