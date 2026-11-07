//! A message's bubble, drawn after Messages' (`message_bubble.svg`): a
//! pill with a curled tail at its bottom corner, sent ones in the accent
//! as a gradient on the right, received ones grey on the left, the two
//! mirrored. Only the last bubble of a run has the tail.
//!
//! The outline is one vector path made for each bubble's size, the SVG's
//! tail joined into the pill's bottom corner, so it stays sharp at any
//! scale; amane draws it, and its gradient, on the quad pipeline.

use std::cell::RefCell;
use std::collections::HashMap;

use amane::{Canvas, Color, Gradient, Path, Rectangle, Shape, Stack, Text};
use amane_kit::color::{over, with_alpha};
use amane_kit::layout::{boxed, pad, text_height, text_width};
use amane_kit::style;
use amane_kit::tokens::{Styled, TextStyle, sys};

/// The text size the SVG was drawn for: its geometry is in these units.
const SVG_TEXT: f32 = 17.0;

/// A bubble's text size.
const TEXT_SIZE: f32 = 15.0;

/// The SVG's measures, in its units: the padding across, a one-line
/// bubble's height and the corner radius, and how far the tail hangs below.
const PAD_X: f32 = 13.0;
const LINE_HEIGHT: f32 = 40.0;
const RADIUS: f32 = 20.0;
const TAIL_BELOW: f32 = 7.0;

/// A bubble is never narrower than this, so its tail has room.
const MIN_WIDTH: f32 = 48.0;

/// How much of the way to white a sent bubble's gradient starts (#0088FF
/// to #4EBAFF in the SVG).
const LIGHTER: f32 = 0.32;

/// Where the pill's bottom-right corner meets the tail, from that corner of
/// the pill, in the SVG's units: on the corner's circle by design.
const JOIN: (f32, f32) = (-7.03, -4.78);

/// The tail's edge from where it leaves the pill's corner (`JOIN`) round
/// its tip and back to the bottom edge, as cubic curves (two handles and an
/// end), from the pill's bottom-right corner in the SVG's units: the SVG's
/// own tail path run backwards.
const TAIL: [[(f32, f32); 3]; 9] = [
    [(-7.26, -4.59), (-7.49, -4.40), (-7.73, -4.21)],
    [(-7.87, -4.10), (-7.99, -4.00), (-8.08, -3.92)],
    [(-9.12, -3.00), (-10.44, -1.62), (-10.44, 0.24)],
    [(-10.44, 1.75), (-10.29, 2.28), (-9.81, 3.29)],
    [(-9.59, 3.74), (-9.03, 4.57), (-8.54, 5.21)],
    [(-8.05, 5.85), (-8.21, 7.04), (-9.47, 6.94)],
    [(-9.89, 6.91), (-11.57, 6.17), (-13.53, 5.09)],
    [(-15.80, 3.84), (-18.54, 1.74), (-19.25, 1.24)],
    [(-20.58, 0.31), (-21.23, 0.00), (-22.22, 0.00)],
];

/// Which side of the conversation a bubble is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// This side's, on the right with the tail on its right.
    Sent,
    /// The other side's, on the left, mirrored.
    Received,
}

/// How a bubble is painted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    Sent,
    Received,
    /// Sent from here and not confirmed yet: faded.
    Pending,
    Failed,
}

impl Look {
    fn side(self) -> Side {
        match self {
            Self::Received => Side::Received,
            _ => Side::Sent,
        }
    }
}

/// One SVG unit at the bubble's text size.
fn unit() -> f32 {
    TEXT_SIZE / SVG_TEXT
}

fn text_style() -> TextStyle {
    TextStyle {
        size: TEXT_SIZE,
        ..sys::r#type::label()
    }
}

/// How big a bubble for `text` is at most `widest` across: its width and
/// the pill's height (the tail hangs below that).
pub fn size(text: &str, widest: f32) -> (f32, f32) {
    let s = unit();
    let pad_x = PAD_X * s;
    let width = (text_width(text, text_style()) + 2.0 * pad_x + 1.0)
        .clamp(MIN_WIDTH * s, widest.max(MIN_WIDTH * s));
    let lines = Text::new(text.to_string())
        .style(text_style())
        .wrap()
        .height_in(width - 2.0 * pad_x);
    let pad_y = (LINE_HEIGHT * s - text_height(text_style())) / 2.0;
    (width, (lines + 2.0 * pad_y).max(LINE_HEIGHT * s))
}

thread_local! {
    // each message's bubble size at a width: measuring text is the costly part of a redraw
    static SIZES: RefCell<HashMap<(i64, u32), (f32, f32)>> = RefCell::new(HashMap::new());
}

/// [`size`], kept for the message `key` at `widest`: a message's text
/// never changes, so it's measured once for each width the window has.
pub fn cached_size(key: i64, text: &str, widest: f32) -> (f32, f32) {
    let at = (key, widest.to_bits());
    if let Some(size) = SIZES.with_borrow(|sizes| sizes.get(&at).copied()) {
        return size;
    }
    let measured = size(text, widest);
    SIZES.with_borrow_mut(|sizes| {
        // a window dragged through many widths leaves old ones behind
        if sizes.len() > 4096 {
            sizes.clear();
        }
        sizes.insert(at, measured);
    });
    measured
}

/// One step of an outline: as `Path` takes them.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Move(f32, f32),
    Line(f32, f32),
    /// Centre, radius, start and sweep in degrees (0 straight up, clockwise).
    Arc(f32, f32, f32, f32, f32),
    /// Two handles and the end.
    Cubic((f32, f32), (f32, f32), (f32, f32)),
}

/// The bubble's outline, `width` by `height` (the pill), with the tail
/// hanging below it when `tail`; mirrored for a received one.
fn steps(width: f32, height: f32, side: Side, tail: bool) -> Vec<Step> {
    let s = unit();
    let r = (RADIUS * s).min(height / 2.0).min(width / 2.0);
    let (w, h) = (width, height);
    // received bubbles are sent ones seen in a mirror
    let x = |x: f32| match side {
        Side::Sent => x,
        Side::Received => w - x,
    };
    let at = |(dx, dy): (f32, f32)| (x(w + dx * s), h + dy * s);
    let arc = |cx: f32, cy: f32, start: f32, sweep: f32| match side {
        Side::Sent => Step::Arc(cx, cy, r, start, sweep),
        Side::Received => Step::Arc(w - cx, cy, r, 360.0 - start, -sweep),
    };

    let mut steps = vec![
        Step::Move(x(r), 0.0),
        Step::Line(x(w - r), 0.0),
        arc(w - r, r, 0.0, 90.0),
        Step::Line(x(w), h - r),
    ];

    if tail {
        // round the corner as far as the tail, along its curl and back to the bottom edge
        let join = ((JOIN.0 * s) + r).atan2(-((JOIN.1 * s) + r)).to_degrees();
        steps.push(arc(w - r, h - r, 90.0, join - 90.0));
        for [first, second, end] in TAIL {
            steps.push(Step::Cubic(at(first), at(second), at(end)));
        }
    } else {
        steps.push(arc(w - r, h - r, 90.0, 90.0));
    }

    steps.extend([
        Step::Line(x(r), h),
        arc(r, h - r, 180.0, 90.0),
        Step::Line(x(0.0), r),
        arc(r, r, 270.0, 90.0),
    ]);
    steps
}

pub fn outline(width: f32, height: f32, side: Side, tail: bool) -> Path {
    steps(width, height, side, tail)
        .into_iter()
        .fold(Path::new(), |path, step| match step {
            Step::Move(x, y) => path.move_to(x, y),
            Step::Line(x, y) => path.line_to(x, y),
            Step::Arc(cx, cy, r, start, sweep) => path.arc(cx, cy, r, start, sweep),
            Step::Cubic(a, b, (x, y)) => path.cubic_to(a.0, a.1, b.0, b.1, x, y),
        })
        .close()
}

/// `color` taken `amount` of the way to white.
fn lighter(color: Color, amount: f32) -> Color {
    over(with_alpha(Color::WHITE, amount), color)
}

/// A bubble holding `text` (the message `key`'s), at most `widest`
/// across, with its tail when it ends a run: a `width` × `height + tail`
/// widget.
pub fn bubble(key: i64, text: &str, look: Look, tail: bool, widest: f32) -> Rectangle {
    let s = unit();
    let (width, height) = cached_size(key, text, widest);
    let total = height + if tail { TAIL_BELOW * s } else { 0.0 };
    let side = look.side();

    let path = outline(width, height, side, tail);
    let accent = sys::color::accent::bg();
    let shape: Box<dyn Shape> = match look {
        Look::Sent => Box::new(path.gradient(Gradient::linear(
            180.0,
            [(0.0, lighter(accent, LIGHTER)), (1.0, accent)],
        ))),
        Look::Pending => Box::new(path.gradient(Gradient::linear(
            180.0,
            [
                (0.0, with_alpha(lighter(accent, LIGHTER), 0.55)),
                (1.0, with_alpha(accent, 0.55)),
            ],
        ))),
        Look::Received => Box::new(path.fill(style::bubble_received())),
        Look::Failed => Box::new(path.fill(sys::color::status::critical::bg())),
    };

    let ink = match look {
        Look::Received => style::label_primary(),
        _ => sys::color::accent::fg(),
    };
    let pad_y = (LINE_HEIGHT * s - text_height(text_style())) / 2.0;
    let words = Rectangle::new()
        .width(width)
        .height(height)
        .padding(pad(PAD_X * s, pad_y))
        .child(
            Text::new(text.to_string())
                .style(text_style())
                .color(ink)
                .wrap(),
        );

    Rectangle::new()
        .width(width)
        .height(total)
        .child(Stack::new(vec![
            boxed(Canvas::new().width(width).height(total).shapes(vec![shape])),
            boxed(words),
        ]))
}

/// How big a few emoji sent alone are drawn, without a bubble.
pub const EMOJI_SIZE: f32 = 40.0;

/// A message of a few emoji alone: large, with no bubble behind them.
pub fn emoji(text: &str) -> Rectangle {
    let style = TextStyle {
        size: EMOJI_SIZE,
        ..sys::r#type::label()
    };
    let width = text_width(text.trim(), style) + 4.0;
    let height = text_height(style) + 4.0;
    Rectangle::new()
        .width(width)
        .height(height)
        .child(Text::new(text.trim().to_string()).style(style))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where an outline reaches: its points, and each arc's ends.
    fn bounds(steps: &[Step]) -> (f32, f32, f32, f32) {
        let mut points = Vec::new();
        for step in steps {
            match *step {
                Step::Move(x, y) | Step::Line(x, y) => points.push((x, y)),
                Step::Cubic(_, _, end) => points.push(end),
                Step::Arc(cx, cy, r, start, sweep) => {
                    for angle in [start, start + sweep] {
                        let (sin, cos) = angle.to_radians().sin_cos();
                        points.push((cx + r * sin, cy - r * cos));
                    }
                }
            }
        }
        points.iter().fold(
            (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
            |(l, t, r, b), &(x, y)| (l.min(x), t.min(y), r.max(x), b.max(y)),
        )
    }

    #[test]
    fn the_tail_hangs_below_the_pill() {
        let s = unit();
        let (left, top, right, bottom) = bounds(&steps(120.0, 40.0 * s, Side::Sent, true));
        assert!(left.abs() < 0.01 && top.abs() < 0.01, "{left} {top}");
        assert!((right - 120.0).abs() < 0.01, "{right}");
        assert!((bottom - (40.0 + 6.94) * s).abs() < 0.05, "bottom {bottom}");

        let (.., bottom) = bounds(&steps(120.0, 40.0 * s, Side::Sent, false));
        assert!((bottom - 40.0 * s).abs() < 0.01);
    }

    #[test]
    fn a_received_bubble_is_a_sent_one_mirrored() {
        let sent = bounds(&steps(120.0, 60.0, Side::Sent, true));
        let received = bounds(&steps(120.0, 60.0, Side::Received, true));
        assert!((sent.0 - (120.0 - received.2)).abs() < 0.01);
        assert!((sent.2 - (120.0 - received.0)).abs() < 0.01);
        assert!((sent.3 - received.3).abs() < 0.01);

        // the tail's tip is the sent one's, mirrored
        let tip = |steps: &[Step]| {
            steps
                .iter()
                .filter_map(|step| match step {
                    Step::Cubic(_, _, end) => Some(*end),
                    _ => None,
                })
                .fold(
                    (0.0, f32::MIN),
                    |low: (f32, f32), p| if p.1 > low.1 { p } else { low },
                )
        };
        let (sent_tip, received_tip) = (
            tip(&steps(120.0, 60.0, Side::Sent, true)),
            tip(&steps(120.0, 60.0, Side::Received, true)),
        );
        assert!((sent_tip.0 - (120.0 - received_tip.0)).abs() < 0.01);
        assert!(
            (sent_tip.0 - 110.0).abs() < 3.0,
            "the tail is at the right: {sent_tip:?}"
        );
    }

    #[test]
    fn the_tail_leaves_the_corner_on_its_circle() {
        // the join is on the 20-unit corner circle, so the outline doesn't step there
        let (jx, jy) = JOIN;
        let distance = (jx + RADIUS).hypot(jy + RADIUS);
        assert!((distance - RADIUS).abs() < 0.05, "{distance}");

        // and the corner's arc ends right on it
        let s = unit();
        let corner = steps(120.0, 40.0 * s, Side::Sent, true)
            .into_iter()
            .filter_map(|step| match step {
                Step::Arc(cx, cy, r, start, sweep) if start == 90.0 => {
                    let (sin, cos) = (start + sweep).to_radians().sin_cos();
                    Some((cx + r * sin, cy - r * cos))
                }
                _ => None,
            })
            .next()
            .unwrap();
        assert!((corner.0 - (120.0 + jx * s)).abs() < 0.05, "{corner:?}");
        assert!((corner.1 - (40.0 + jy) * s).abs() < 0.05, "{corner:?}");
    }

    #[test]
    fn a_bubble_grows_with_its_text_and_stops_at_the_widest() {
        let (short_w, short_h) = size("ok", 400.0);
        assert_eq!(cached_size(1, "ok", 400.0), (short_w, short_h));
        // the same message at the same width isn't measured again
        assert_eq!(cached_size(1, "changed", 400.0), (short_w, short_h));
        let (long_w, long_h) = size(&"word ".repeat(80), 400.0);
        assert!((short_h - LINE_HEIGHT * unit()).abs() < 0.01);
        assert!(short_w >= MIN_WIDTH * unit());
        assert!(long_w <= 400.0);
        assert!(long_h > short_h * 2.0);
    }
}
