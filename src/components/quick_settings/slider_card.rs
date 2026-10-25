//! Slider card — Control Center's Display and Sound modules: a glass card
//! with a title over the slider, a small glyph before the thin track and a
//! large faint one after it, and a chevron beside a slider that has a page
//! of devices to pick from. Three cells wide it leaves room for a round
//! button beside it; four, it takes the row.

use std::rc::Rc;

use amane::{Button, Center, Color, Column, Rectangle, Row, Size, Stack, Start, Text, Widget};

use super::grid::{Metrics, Span};
use super::tile::module;
use crate::components::color::with_alpha;
use crate::components::layout::{boxed, icon, pad};
use crate::components::material::{self, Level};
use crate::components::slider::Slider;
use crate::tokens::{Styled, quick, sys};

/// One slider in a card: its title, the glyphs either side of the track,
/// the slider made for the width it gets, and what follows it (a chevron
/// to a page). The leading glyph can be a control of its own (mute).
pub(super) struct SliderRow {
    title: String,
    leading: Box<dyn Widget>,
    large: char,
    slider: Box<dyn FnOnce(f32) -> Slider>,
    trailing: Option<Box<dyn Widget>>,
}

impl SliderRow {
    pub(super) fn new(
        title: impl Into<String>,
        small: char,
        large: char,
        slider: impl FnOnce(f32) -> Slider + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            leading: boxed(icon(
                small,
                sys::size::icon::s(),
                material::label(Level::Primary),
            )),
            large,
            slider: Box::new(slider),
            trailing: None,
        }
    }

    /// A control in place of the small glyph before the track (mute).
    pub(super) fn leading(mut self, leading: impl Widget + 'static) -> Self {
        self.leading = boxed(leading);
        self
    }

    /// A control-square widget after the slider.
    pub(super) fn trailing(mut self, trailing: impl Widget + 'static) -> Self {
        self.trailing = Some(boxed(trailing));
        self
    }
}

/// A slider card, with what a right click on it does.
pub(super) struct Card(Rectangle);

impl Card {
    /// A right click anywhere on the card (its page, say).
    pub(super) fn on_secondary(self, open: impl Fn() + 'static) -> Self {
        let open: Rc<dyn Fn()> = Rc::new(open);
        Self(self.0.on_click(move |button| {
            if button == Button::Right {
                open();
            }
        }))
    }
}

impl From<Card> for Box<dyn Widget> {
    fn from(card: Card) -> Self {
        boxed(card.0)
    }
}

/// A card of one `row`, `span` wide and a cell tall.
pub(super) fn card(row: SliderRow, span: Span) -> Card {
    let px = Metrics::tokens();
    let inset = quick::tile::pad();
    let gap = quick::tile::gap();
    let (width, height) = (px.width(span), px.cell);
    let inner = (width - inset * 2.0).max(1.0);
    let control = sys::size::control::height();
    let large_w = sys::size::icon::m();

    let title = Text::new(row.title)
        .style(quick::r#type::title())
        .color(material::label(Level::Primary));
    let mut slider_w = inner - control - gap - large_w - gap;
    if row.trailing.is_some() {
        slider_w -= control + gap;
    }
    let slider = (row.slider)(slider_w.max(1.0))
        .fill(material::label(Level::Primary))
        .thickness(quick::slider::track())
        .build();
    let leading = Rectangle::new()
        .width(Size::Fixed(control))
        .height(Size::Fixed(control))
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .child(Stack::new(vec![row.leading]));
    let large = icon(
        row.large,
        large_w,
        with_alpha(material::label(Level::Primary), 0.5),
    );
    let mut line: Vec<Box<dyn Widget>> = vec![boxed(leading), boxed(slider), boxed(large)];
    line.extend(row.trailing);
    let line = Row::new(line).gap(gap).align(Center);
    let content = Column::new(vec![boxed(title), boxed(line)]).width(Size::Fixed(inner));

    let inner_rect = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .padding(pad(inset, 0.0))
        .align_child(Start, Center)
        .child(content);

    Card(module(width, height, quick::card::radius(), inner_rect))
}
