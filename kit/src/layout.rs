//! Layout primitives every component builds with: small free functions that
//! have no look of their own beyond the `sys.*` tokens they're handed.
//! Anything with a look, states or a builder is a component instead (see
//! [`list`](super::list), [`card`](super::card) and the rest).

use std::cell::Cell;

use amane::{
    Align, Center, Color, Padding, Parent, Point, Rectangle, Row, Size, SpaceBetween, Text, Widget,
};

use crate::material::{self, Level};
use crate::tokens::{Styled, TextStyle, sys};

pub fn boxed(widget: impl Widget + 'static) -> Box<dyn Widget> {
    Box::new(widget)
}

/// `x` either side, `y` above and below.
pub fn pad(x: f32, y: f32) -> Padding {
    Padding {
        top: y,
        right: x,
        bottom: y,
        left: x,
    }
}

/// Nothing at all, for a slot that shows nothing this frame.
pub fn nothing() -> Box<dyn Widget> {
    boxed(Row::new(Vec::new()))
}

/// A transparent rectangle filling its parent: an empty surface's child, or
/// a frame to align something in.
pub fn blank() -> Rectangle {
    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
}

/// A transparent `side`-square slot that centers its child.
pub fn slot(side: f32) -> Rectangle {
    Rectangle::new()
        .width(side)
        .height(side)
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
}

/// A Material-Symbols glyph as a `Text` widget (centered on its ink).
///
/// Every glyph in the shell goes through here, so they all share one font
/// instance (weight, grade, optical size); keep picking glyphs from the same
/// drawing family so their visual weight matches too.
pub fn icon(glyph: char, size: f32, color: Color) -> Text {
    Text::new(glyph.to_string())
        .font(sys::r#type::icon())
        .size(size)
        .color(color)
        .tight()
}

/// A secondary caption: a section title, or a note like "Searching…".
pub fn caption(text: impl Into<String>) -> Text {
    Text::new(text.into())
        .style(sys::r#type::caption())
        .color(material::label(Level::Secondary))
}

/// `leading` at the start, `trailing` (`gap.s` apart) at the end, across the
/// parent's width.
pub fn split_row(leading: impl Widget + 'static, trailing: Vec<Box<dyn Widget>>) -> Row {
    Row::new(vec![
        boxed(leading),
        boxed(Row::new(trailing).gap(sys::space::gap::s()).align(Center)),
    ])
    .width(Parent)
    .justify(SpaceBetween)
    .align(Center)
}

/// One line of text `width` wide, elided and clipped to it.
pub fn clipped_label(
    text: impl Into<String>,
    style: TextStyle,
    color: Color,
    width: f32,
    align: impl Into<Align>,
) -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .align_child(align, Center)
        .clip()
        .child(Text::new(text.into()).style(style).color(color).elide())
}

thread_local! {
    // Last pointer position over a tracked surface (surface-local). A
    // right-click handler only gets a Button, so it reads this to anchor a
    // menu. There's one pointer, so the surface it was last over is the one
    // being clicked.
    static CURSOR: Cell<(i32, i32)> = const { Cell::new((0, 0)) };
}

/// Records the pointer's position over `rect` for [`cursor`].
pub fn track_cursor(rect: Rectangle) -> Rectangle {
    rect.on_move(|p: Point| CURSOR.with(|c| c.set((p.x as i32, p.y as i32))))
}

/// Where the pointer last was over a tracked surface, for anchoring a menu.
pub fn cursor() -> (i32, i32) {
    CURSOR.with(Cell::get)
}

/// How wide `text` is in `style`, measured.
pub fn text_width(text: &str, style: TextStyle) -> f32 {
    fixed_width(&Text::new(text).style(style))
}

/// `text` broken onto at most `max_lines` lines no wider than `width`, at
/// the spaces, the last line elided to what's left over. amane's `Text`
/// can't centre each line of wrapped text, so a caption centred under a
/// tile is drawn a line at a time from this.
pub fn wrapped_lines(text: &str, style: TextStyle, width: f32, max_lines: usize) -> Vec<String> {
    let fits = |line: &str| text_width(line, style) <= width;
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let joined = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if line.is_empty() || fits(&joined) {
            line = joined;
        } else {
            lines.push(std::mem::replace(&mut line, word.to_string()));
            if lines.len() == max_lines {
                break;
            }
        }
    }
    if lines.len() < max_lines {
        if !line.is_empty() {
            lines.push(line);
        }
    } else if let Some(last) = lines.last_mut() {
        // the rest didn't fit: the last line says so
        let mut elided = format!("{last}…");
        while !fits(&elided) && elided.chars().count() > 1 {
            elided.pop();
            elided.pop();
            elided.push('…');
        }
        *last = elided;
    }
    lines
}

/// How tall one line of text in `style` is, measured.
pub fn text_height(style: TextStyle) -> f32 {
    match Text::new("").style(style).height() {
        Size::Fixed(height) => height,
        Size::Parent => 0.0,
    }
}

/// A widget's fixed width; 0 for one that takes its parent's.
pub fn fixed_width(widget: &impl Widget) -> f32 {
    match widget.width() {
        Size::Fixed(width) => width,
        Size::Parent => 0.0,
    }
}

/// The room inside a surface `outer` wide, less its `inset.l` either side.
pub fn inner_width(outer: f32) -> f32 {
    outer - sys::space::inset::l() * 2.0
}
