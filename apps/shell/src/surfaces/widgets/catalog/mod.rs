//! Every widget's face, and the pieces they're drawn with: a header (the
//! kind's glyph and name), a large figure, a ring, a sparkline and a bar.
//! Each face is laid out by hand for its size, every piece placed at a
//! point in the card's content box: a widget is a few readings in fixed
//! places.

mod battery;
mod clock;
mod digits;
mod system;
pub mod units;

use amane::{Arc, Canvas, Cap, Color, Path, Rectangle, Shape, Stack, Start, Text, Widget};

use super::kind::{WidgetKey, WidgetKind, WidgetSize};
use super::store::Settings;
use crate::tokens::{Styled, TextStyle, sys, widget};
use emaki_kit::layout::{boxed, frame, icon, text_height, text_width};
use emaki_kit::material::{self, FillLevel, Level};

pub use battery::devices;
pub use clock::settle as settle_clock;

/// What a face is drawn in: the content box (the card less `widget.card.pad`) and
/// the widget's settings.
pub struct Face<'a> {
    pub width: f32,
    pub height: f32,
    pub size: WidgetSize,
    pub settings: &'a Settings,
}

/// `key`'s face at `size`, `width` by `height` (the whole card).
pub fn build(
    key: WidgetKey,
    size: WidgetSize,
    width: f32,
    height: f32,
    settings: &Settings,
) -> Box<dyn Widget> {
    // the room inside a card's edge
    let pad = widget::card::pad();
    let face = Face {
        width: width - 2.0 * pad,
        height: height - 2.0 * pad,
        size,
        settings,
    };
    let content = match key.kind {
        WidgetKind::Battery => battery::face(&face),
        WidgetKind::Cpu => system::cpu(&face),
        WidgetKind::Memory => system::memory(&face),
        WidgetKind::Temperature => system::temperature(&face),
        WidgetKind::Network => system::network(&face),
        WidgetKind::Disk => system::disk(&face),
        WidgetKind::System => system::overview(&face),
        WidgetKind::Clock => clock::face(key.id, &face),
    };
    boxed(
        frame(width, height).align_child(Start, Start).child(
            frame(face.width, face.height)
                .translate(pad, pad)
                .child(Stack::new(content)),
        ),
    )
}

// ───── the pieces ────────────────────────────────────────────────────────────

/// `widget` `width` by `height` with its top-left at (`x`, `y`) in the
/// content box.
pub fn place(
    widget: impl Widget + 'static,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Box<dyn Widget> {
    boxed(
        frame(width.max(0.0), height.max(0.0))
            .align_child(Start, Start)
            .translate(x, y)
            .child(widget),
    )
}

/// `text` on one line `width` wide at (`x`, `y`), elided; or, when
/// `right`, ending at `x + width` (measured, as an elided text fills its
/// box and can't be aligned in it).
pub fn line(
    text: impl Into<String>,
    style: TextStyle,
    color: Color,
    (x, y): (f32, f32),
    width: f32,
    right: bool,
) -> Box<dyn Widget> {
    let text = text.into();
    let height = text_height(style);
    let (x, width) = if right {
        let measured = text_width(&text, style).min(width) + 1.0;
        (x + width - measured, measured)
    } else {
        (x, width)
    };
    boxed(
        frame(width.max(0.0), height)
            .align_child(Start, Start)
            .translate(x, y)
            .child(Text::new(text).style(style).color(color).elide()),
    )
}

/// The kind's glyph and name across the top, in the secondary label colour.
pub fn header(kind: WidgetKind, title: &str, width: f32) -> Vec<Box<dyn Widget>> {
    let style = sys::r#type::caption();
    let side = text_height(style);
    let color = material::label(Level::Secondary);
    vec![
        place(icon(kind.glyph(), side, color), 0.0, 0.0, side, side),
        line(
            title,
            style,
            color,
            (side + sys::space::gap::xs(), 0.0),
            width - side - sys::space::gap::xs(),
            false,
        ),
    ]
}

/// A card with nothing to show: its header, and `text` in the middle.
pub fn empty(face: &Face, kind: WidgetKind, title: &str, text: &str) -> Vec<Box<dyn Widget>> {
    let mut out = header(kind, title, face.width);
    let style = label_style();
    let height = text_height(style);
    let width = (text_width(text, style) + 1.0).min(face.width);
    out.push(line(
        text,
        style,
        secondary(),
        (
            (face.width - width) / 2.0,
            (face.height + header_height() - height) / 2.0,
        ),
        width,
        false,
    ));
    out
}

/// How tall [`header`] is.
pub fn header_height() -> f32 {
    text_height(sys::r#type::caption())
}

/// The large reading's style; `large` for a card with the room.
pub fn figure_style(large: bool) -> TextStyle {
    TextStyle {
        size: if large {
            widget::r#type::figure_large()
        } else {
            widget::r#type::figure()
        },
        weight: 600,
        ..sys::r#type::numeric()
    }
}

pub fn label_style() -> TextStyle {
    sys::r#type::label()
}

pub fn caption_style() -> TextStyle {
    sys::r#type::caption()
}

pub fn primary() -> Color {
    material::label(Level::Primary)
}

pub fn secondary() -> Color {
    material::label(Level::Secondary)
}

/// The accent, or the critical colour when `warn`.
pub fn accent(warn: bool) -> Color {
    if warn {
        sys::color::status::critical::bg()
    } else {
        sys::color::accent::bg()
    }
}

/// The track a ring or bar fills along.
fn track() -> Color {
    material::fill_role(FillLevel::Secondary)
}

/// A ring `diameter` across, `thickness` thick, filled `fraction` of the
/// way round from the top in `color`.
pub fn ring(fraction: f32, diameter: f32, thickness: f32, color: Color) -> Canvas {
    let mut shapes: Vec<Box<dyn Shape>> = vec![Box::new(Arc::new().stroke(thickness, track()))];
    let sweep = 360.0 * fraction.clamp(0.0, 1.0);
    if sweep > 0.0 {
        shapes.push(Box::new(
            Arc::new()
                .sweep(sweep.max(1.0))
                .stroke(thickness, color)
                .cap(Cap::Round),
        ));
    }
    Canvas::new()
        .width(diameter)
        .height(diameter)
        .shapes(shapes)
}

/// A ring with `glyph` in the middle.
pub fn ring_with(fraction: f32, diameter: f32, thickness: f32, color: Color, glyph: char) -> Stack {
    let side = diameter * 0.42;
    Stack::new(vec![
        boxed(ring(fraction, diameter, thickness, color)),
        place(
            icon(glyph, side, primary()),
            (diameter - side) / 2.0,
            (diameter - side) / 2.0,
            side,
            side,
        ),
    ])
}

/// `values` (oldest first) as a line `width` by `height`, `max` at the
/// top, with a faint fill under it; the newest at the right edge. Fewer
/// readings than there's room for start further right.
pub fn sparkline(values: &[f32], max: f32, width: f32, height: f32, color: Color) -> Canvas {
    sparkline_between(values, (0.0, max), (width, height), color, true)
}

/// `values` as a line `size` big, `range.0` at the bottom and `range.1`
/// at the top, with a faint fill under it when `filled` (a second series
/// over a first draws as a line alone, so the two don't muddy each other).
pub fn sparkline_between(
    values: &[f32],
    (min, max): (f32, f32),
    (width, height): (f32, f32),
    color: Color,
    filled: bool,
) -> Canvas {
    let canvas = Canvas::new().width(width.max(1.0)).height(height.max(1.0));
    if values.len() < 2 || max <= min {
        return canvas.shapes(Vec::new());
    }
    let step = width / (SPARK_POINTS - 1) as f32;
    let start = width - step * (values.len() - 1) as f32;
    let point = |i: usize, v: f32| {
        let y = height - ((v - min) / (max - min)).clamp(0.0, 1.0) * (height - 1.0) - 0.5;
        (start + step * i as f32, y)
    };
    let mut stroke = Path::new();
    let mut fill = Path::new().move_to(start, height);
    for (i, v) in values.iter().enumerate() {
        let (x, y) = point(i, *v);
        stroke = if i == 0 {
            stroke.move_to(x, y)
        } else {
            stroke.line_to(x, y)
        };
        fill = fill.line_to(x, y);
    }
    let fill = fill.line_to(width, height).close();
    let faint = Color::rgba(color.red(), color.green(), color.blue(), 56);
    let mut shapes: Vec<Box<dyn Shape>> = Vec::new();
    if filled {
        shapes.push(Box::new(fill.fill(faint)));
    }
    shapes.push(Box::new(stroke.stroke(1.5, color).cap(Cap::Round)));
    canvas.shapes(shapes)
}

/// How many readings a sparkline spans across its width.
pub const SPARK_POINTS: usize = 60;

/// A bar `width` by `height`, filled `fraction` of the way in `color`.
pub fn bar(fraction: f32, width: f32, height: f32, color: Color) -> Stack {
    let filled = (width * fraction.clamp(0.0, 1.0)).max(if fraction > 0.0 { height } else { 0.0 });
    Stack::new(vec![
        boxed(
            Rectangle::new()
                .width(width)
                .height(height)
                .radius(height / 2.0)
                .fill(track()),
        ),
        boxed(
            Rectangle::new()
                .width(filled.min(width))
                .height(height)
                .radius(height / 2.0)
                .fill(color),
        ),
    ])
}

/// "42%".
pub fn percent(fraction: f32) -> String {
    format!("{}%", (fraction.clamp(0.0, 1.0) * 100.0).round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentages_round() {
        assert_eq!(percent(0.424), "42%");
        assert_eq!(percent(0.995), "100%");
        assert_eq!(percent(-1.0), "0%");
    }
}
