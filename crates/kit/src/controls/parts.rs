//! What the form controls share: the checkbox's and the radio button's
//! mark, and a mark with its label.

use amane::{Center, Radius, Rectangle, Row, Size, Text, Widget};

use crate::control_size;
use crate::layout::{boxed, text_width};
use crate::look::hover::crossfade;
use crate::style::{self, px};
use crate::symbol::{Symbol, symbol};
use crate::tokens::Styled;

/// Which mark: the checkbox's square with a tick, or the radio's circle
/// with a dot.
#[derive(Clone, Copy)]
pub(crate) enum Mark {
    Check,
    Dot,
}

/// The mark at the size controls are being built at: the field's fill in a
/// hairline while off, filling with the accent around its tick or dot as it
/// turns on.
pub(crate) fn mark(id: &str, on: bool, kind: Mark) -> Rectangle {
    let size = control_size::current();
    let (side, corner) = size.checkbox();
    let fill = crossfade(
        &format!("{id}.fill"),
        on,
        style::field_fill(),
        style::tint(),
    );
    let base = Rectangle::new()
        .width(Size::Fixed(side))
        .height(Size::Fixed(side))
        .radius(match kind {
            Mark::Check => Radius::from(corner),
            Mark::Dot => Radius::Full,
        })
        .fill(fill)
        .align_child(Center, Center);
    if !on {
        return base.border(1.0, style::field_border());
    }
    match kind {
        Mark::Check => base.child(symbol(
            Symbol::Checkmark,
            side * 11.0 / 16.0,
            style::glyph_on_tint(),
        )),
        Mark::Dot => {
            let dot = size.radio_dot();
            base.child(
                Rectangle::new()
                    .width(Size::Fixed(dot))
                    .height(Size::Fixed(dot))
                    .radius(Radius::Full)
                    .fill(style::glyph_on_tint()),
            )
        }
    }
}

/// A mark and its optional label, 3 px apart, the shared shape of the
/// checkbox and the radio button.
pub(crate) fn labelled(mark: Rectangle, label: Option<String>) -> Rectangle {
    let gap = px(3.0);
    let mut width = control_size::current().checkbox().0;
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(mark)];
    if let Some(label) = label {
        width += gap + text_width(&label, style::control_font()).ceil();
        parts.push(boxed(
            Text::new(label)
                .style(style::control_font())
                .color(style::label_primary()),
        ));
    }
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(style::control_height()))
        .align_child(amane::Start, Center)
        .child(Row::new(parts).gap(gap).align(Center))
}
