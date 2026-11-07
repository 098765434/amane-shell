//! SFSymbol: the handful of symbols the macform controls draw, as
//! strokes with round caps and joins (no font has them on Linux).
//!
//! Each symbol is a few straight lines in a `size` square, which amane
//! draws on its quad pipeline (round-joined polylines), or plain
//! rectangles for the round ones.

#![allow(missing_docs)]

use amane::{Canvas, Cap, Center, Color, Join, Path, Radius, Rectangle, Row, Shape, Size, Stack};

use crate::layout::boxed;

/// The symbols the controls use, named as in the bar's symbol set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    ChevronDown,
    ChevronUp,
    ChevronLeft,
    ChevronRight,
    /// `chevron.up.chevron.down`: a pop-up button's mark.
    ChevronUpDown,
    Checkmark,
    Ellipsis,
    InfoCircle,
}

impl Symbol {
    /// The polylines, in 0..1 of the square.
    fn lines(self) -> &'static [&'static [(f32, f32)]] {
        match self {
            Self::ChevronDown => &[&[(0.28, 0.40), (0.50, 0.62), (0.72, 0.40)]],
            Self::ChevronUp => &[&[(0.28, 0.60), (0.50, 0.38), (0.72, 0.60)]],
            Self::ChevronRight => &[&[(0.42, 0.28), (0.64, 0.50), (0.42, 0.72)]],
            Self::ChevronLeft => &[&[(0.58, 0.28), (0.36, 0.50), (0.58, 0.72)]],
            Self::ChevronUpDown => &[
                &[(0.28, 0.42), (0.50, 0.26), (0.72, 0.42)],
                &[(0.28, 0.58), (0.50, 0.74), (0.72, 0.58)],
            ],
            Self::Checkmark => &[&[(0.20, 0.52), (0.42, 0.73), (0.80, 0.28)]],
            Self::Ellipsis | Self::InfoCircle => &[],
        }
    }
}

/// The stroke a symbol `size` across is drawn with.
pub fn stroke_width(size: f32) -> f32 {
    (size * 0.12).max(1.0)
}

/// `symbol` in a `size` square, drawn in `color`.
pub fn symbol(symbol: Symbol, size: f32, color: Color) -> Rectangle {
    let frame = Rectangle::new()
        .width(Size::Fixed(size))
        .height(Size::Fixed(size))
        .align_child(Center, Center);
    let stroke = stroke_width(size);

    match symbol {
        Symbol::Ellipsis => {
            let dot = size * 0.16;
            let gap = size * 0.12;
            let dots = (0..3)
                .map(|_| {
                    boxed(
                        Rectangle::new()
                            .width(Size::Fixed(dot))
                            .height(Size::Fixed(dot))
                            .radius(Radius::Full)
                            .fill(color),
                    )
                })
                .collect();
            frame.child(Row::new(dots).gap(gap).align(Center))
        }
        Symbol::InfoCircle => {
            let ring = size * 0.92;
            let stem_w = stroke * 1.1;
            let dot = Rectangle::new()
                .width(Size::Fixed(stem_w))
                .height(Size::Fixed(stem_w))
                .radius(Radius::Full)
                .fill(color)
                .translate(0.0, size * 0.28 - (size - ring) / 2.0);
            let stem = Rectangle::new()
                .width(Size::Fixed(stem_w))
                .height(Size::Fixed(size * 0.26))
                .radius(Radius::Full)
                .fill(color)
                .translate(0.0, size * 0.44 - (size - ring) / 2.0);
            let column = |part: Rectangle| {
                boxed(
                    Rectangle::new()
                        .width(Size::Fixed(ring))
                        .height(Size::Fixed(ring))
                        .align_child(Center, amane::Start)
                        .child(part),
                )
            };
            frame.child(
                Rectangle::new()
                    .width(Size::Fixed(ring))
                    .height(Size::Fixed(ring))
                    .radius(Radius::Full)
                    .border(stroke, color)
                    .child(Stack::new(vec![column(dot), column(stem)])),
            )
        }
        _ => {
            let shapes = symbol
                .lines()
                .iter()
                .map(|line| {
                    let mut path = Path::new();
                    for (i, (x, y)) in line.iter().enumerate() {
                        path = if i == 0 {
                            path.move_to(x * size, y * size)
                        } else {
                            path.line_to(x * size, y * size)
                        };
                    }
                    Box::new(path.stroke(stroke, color).cap(Cap::Round).join(Join::Round))
                        as Box<dyn Shape>
                })
                .collect();
            frame.child(
                Canvas::new()
                    .width(Size::Fixed(size))
                    .height(Size::Fixed(size))
                    .shapes(shapes),
            )
        }
    }
}
