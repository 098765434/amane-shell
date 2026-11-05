//! Form — settings laid out as macOS's System Settings lays them out:
//! rounded [`Group`]s of [`FormRow`]s, each a coloured leading tile, a
//! title (and a subtitle), and its control at the trailing edge, with
//! hairlines between rows that start after the tile.
//!
//! A group's caption sits above its card and its note below it, wrapped to
//! the group's width. Every row knows its own height (amane's rectangles
//! need one), so a group is exactly as tall as its rows and nobody adds
//! them up by hand.
//!
//! - [`FormRow`]: a title with a trailing control (switch, dropdown,
//!   stepper, button, a value); optionally a block under the title
//!   ([`FormRow::detail`]: radios, fields, tags), or the whole row
//!   pressable with a chevron, to open something ([`FormRow::on_press`]).
//! - [`SliderRow`]: a title and its value over a slider across the row.
//! - [`Group::custom`]: anything else, at a height of its own.
//!
//! Sizes are the macform look's (`macform::theme`): rows 42 tall (52 with a
//! subtitle), 12 px in, cards radius 12 in `groupFill`.

use std::rc::Rc;

use amane::{Center, Color, Column, Padding, Rectangle, Row, Size, Text, Widget};

use crate::color::Accent;
use crate::layout::{boxed, icon, text_height, wrapped_lines};
use crate::macform::symbol::{Symbol, symbol};
use crate::macform::theme::{self, px};
use crate::macform::{MacLeadingIcon, hover_fill, on_press};
use crate::tokens::{Styled, TextStyle, sys};

/// How wide a group is unless told: Shell Settings' page.
pub const DEFAULT_WIDTH: f32 = 504.0;

fn tile_side() -> f32 {
    px(20.0)
}

fn tile_gap() -> f32 {
    px(10.0)
}

fn pad_h() -> f32 {
    theme::row_pad_h()
}

fn title_font() -> TextStyle {
    theme::control_font()
}

fn subtitle_font() -> TextStyle {
    theme::subtitle_font()
}

/// A note's style: a group's footer, a row's subtitle.
fn note_font() -> TextStyle {
    sys::r#type::caption()
}

/// A setting: a title, maybe a tile and a subtitle, and its control.
pub struct FormRow {
    id: String,
    title: String,
    subtitle: Option<String>,
    icon: Option<(char, Accent)>,
    trailing: Vec<Box<dyn Widget>>,
    detail: Option<(Box<dyn Widget>, f32)>,
    on_press: Option<Rc<dyn Fn()>>,
    disabled: bool,
}

impl FormRow {
    /// `id` keys its hover when it's pressable, so it must be unique and
    /// stable.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            trailing: Vec::new(),
            detail: None,
            on_press: None,
            disabled: false,
        }
    }

    /// A second line under the title, in grey, wrapped to the room left.
    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        self.subtitle = (!text.is_empty()).then_some(text);
        self
    }

    /// `glyph` on a tile of `accent` before the title.
    pub fn icon(mut self, glyph: char, accent: Accent) -> Self {
        self.icon = Some((glyph, accent));
        self
    }

    /// A control at the trailing edge; more than one sit side by side.
    pub fn trailing(mut self, widget: impl Widget + 'static) -> Self {
        self.trailing.push(boxed(widget));
        self
    }

    /// A value at the trailing edge, in grey.
    pub fn value(self, text: impl Into<String>) -> Self {
        self.trailing(
            Text::new(text.into())
                .style(title_font())
                .color(theme::label_secondary()),
        )
    }

    /// A block `height` tall under the title, lined up with it: a group of
    /// radios, a field and its button, a row of tags.
    pub fn detail(mut self, widget: impl Widget + 'static, height: f32) -> Self {
        self.detail = Some((boxed(widget), height));
        self
    }

    /// The whole row is a button with a chevron at its trailing edge, to
    /// open what it names.
    pub fn on_press(mut self, press: impl Fn() + 'static) -> Self {
        self.on_press = Some(Rc::new(press));
        self
    }

    /// Dimmed; a pressable row stops taking clicks.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Where the title starts, from the row's leading edge.
    fn text_start(&self) -> f32 {
        pad_h()
            + if self.icon.is_some() {
                tile_side() + tile_gap()
            } else {
                0.0
            }
    }

    fn trailing_width(&self) -> f32 {
        let gap = sys::space::gap::s();
        let controls: f32 = self
            .trailing
            .iter()
            .map(|w| match w.width() {
                Size::Fixed(w) => w,
                Size::Parent => 0.0,
            })
            .sum::<f32>()
            + gap * self.trailing.len().saturating_sub(1) as f32;
        let chevron = if self.on_press.is_some() {
            px(12.0) + gap
        } else {
            0.0
        };
        controls + chevron
    }

    /// The subtitle's lines in a row `width` wide.
    fn subtitle_lines(&self, width: f32) -> Vec<String> {
        let Some(subtitle) = &self.subtitle else {
            return Vec::new();
        };
        let room = width - self.text_start() - pad_h() - self.trailing_width() - px(12.0);
        wrapped_lines(subtitle, subtitle_font(), room.max(80.0), 4)
    }

    /// The top line's height: the title (and subtitle), or the minimum.
    fn head_height(&self, width: f32) -> f32 {
        let lines = self.subtitle_lines(width).len() as f32;
        let text = text_height(title_font()) + lines * text_height(subtitle_font());
        (text + px(16.0)).max(theme::row_min_height())
    }

    /// How tall the row is in a group `width` wide.
    pub fn height(&self, width: f32) -> f32 {
        self.head_height(width) + self.detail.as_ref().map_or(0.0, |(_, h)| h + px(10.0))
    }

    /// Where its divider starts.
    fn divider_start(&self) -> f32 {
        self.text_start()
    }

    pub fn build(self, width: f32) -> Rectangle {
        let height = self.height(width);
        let head = self.head_height(width);
        let lines = self.subtitle_lines(width);
        let start = self.text_start();

        let mut text: Vec<Box<dyn Widget>> = vec![boxed(
            Text::new(self.title.clone())
                .style(title_font())
                .color(theme::label_primary()),
        )];
        for line in lines {
            text.push(boxed(
                Text::new(line)
                    .style(subtitle_font())
                    .color(theme::label_secondary()),
            ));
        }
        let text = Column::new(text);

        let mut leading: Vec<Box<dyn Widget>> = Vec::new();
        if let Some((glyph, accent)) = self.icon {
            leading.push(boxed(MacLeadingIcon::new(glyph).accent(accent).build()));
        }
        leading.push(boxed(text));
        let leading = Row::new(leading).gap(tile_gap()).align(Center);

        let mut trailing = self.trailing;
        if self.on_press.is_some() {
            trailing.push(boxed(symbol(
                Symbol::ChevronRight,
                px(12.0),
                theme::label_tertiary(),
            )));
        }
        let trailing = Row::new(trailing).gap(sys::space::gap::s()).align(Center);

        let top = Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(head))
            .align_child(amane::Start, Center)
            .child(
                Row::new(vec![boxed(leading), boxed(trailing)])
                    .width(Size::Parent)
                    .justify(amane::SpaceBetween)
                    .align(Center),
            );

        let mut parts: Vec<Box<dyn Widget>> = vec![boxed(top)];
        if let Some((detail, detail_h)) = self.detail {
            parts.push(boxed(
                Rectangle::new()
                    .width(Size::Parent)
                    .height(Size::Fixed(detail_h))
                    .padding(Padding {
                        left: start - pad_h(),
                        ..Padding::default()
                    })
                    .align_child(amane::Start, amane::Start)
                    .child(Column::new(vec![detail])),
            ));
        }

        let fill = match &self.on_press {
            Some(_) => hover_fill(
                &self.id,
                Color::TRANSPARENT,
                theme::label_primary(),
                self.disabled,
            ),
            None => Color::TRANSPARENT,
        };

        let base = Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(height))
            .fill(fill)
            .padding(Padding {
                left: pad_h(),
                right: pad_h(),
                top: 0.0,
                bottom: 0.0,
            })
            .child(Column::new(parts).width(Size::Parent));

        match self.on_press {
            Some(press) => {
                crate::macform::interactive(base, self.id, self.disabled, on_press(move || press()))
            }
            None if self.disabled => base.opacity(0.5),
            None => base,
        }
    }
}

/// A title and its value over a slider that spans the row, as macOS sets
/// a level (volume, size).
pub struct SliderRow {
    title: String,
    value: String,
    icon: Option<(char, Accent)>,
    glyph: Option<char>,
    slider: Box<dyn FnOnce(f32) -> Box<dyn Widget>>,
}

impl SliderRow {
    /// `slider` builds the slider for the width it's given.
    pub fn new<W: Widget + 'static>(
        title: impl Into<String>,
        slider: impl FnOnce(f32) -> W + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            value: String::new(),
            icon: None,
            glyph: None,
            slider: Box::new(move |width| boxed(slider(width))),
        }
    }

    /// The level as words ("48 pt", "70%"), at the trailing edge.
    pub fn value(mut self, text: impl Into<String>) -> Self {
        self.value = text.into();
        self
    }

    /// `glyph` on a tile of `accent` before the title.
    pub fn icon(mut self, glyph: char, accent: Accent) -> Self {
        self.icon = Some((glyph, accent));
        self
    }

    /// A bare grey glyph before it, as the original's sound rows had.
    pub fn glyph(mut self, glyph: char) -> Self {
        self.glyph = Some(glyph);
        self
    }

    fn text_start(&self) -> f32 {
        pad_h()
            + if self.icon.is_some() || self.glyph.is_some() {
                tile_side() + tile_gap()
            } else {
                0.0
            }
    }

    pub fn height(&self) -> f32 {
        px(12.0) + text_height(title_font()) + px(6.0) + px(24.0) + px(8.0)
    }

    pub fn build(self, width: f32) -> Rectangle {
        let height = self.height();
        let start = self.text_start();
        let slider_w = (width - start - pad_h()).max(40.0);

        let mut parts: Vec<Box<dyn Widget>> = Vec::new();
        if let Some((glyph, accent)) = self.icon {
            parts.push(boxed(MacLeadingIcon::new(glyph).accent(accent).build()));
        } else if let Some(glyph) = self.glyph {
            parts.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(tile_side()))
                    .height(Size::Fixed(tile_side()))
                    .align_child(Center, Center)
                    .child(icon(glyph, px(18.0), theme::label_secondary())),
            ));
        }

        let head = Row::new(vec![
            boxed(
                Text::new(self.title)
                    .style(title_font())
                    .color(theme::label_primary()),
            ),
            boxed(
                Text::new(self.value)
                    .style(title_font())
                    .color(theme::label_secondary()),
            ),
        ])
        .width(Size::Fixed(slider_w))
        .justify(amane::SpaceBetween)
        .align(Center);

        parts.push(boxed(
            Column::new(vec![boxed(head), (self.slider)(slider_w)]).gap(px(6.0)),
        ));

        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(height))
            .padding(Padding {
                left: pad_h(),
                right: pad_h(),
                top: px(12.0),
                bottom: px(8.0),
            })
            .child(Row::new(parts).gap(tile_gap()))
    }
}

/// One entry of a group.
enum Item {
    Row(FormRow),
    Slider(SliderRow),
    Custom(Box<dyn Widget>, f32),
}

impl Item {
    fn height(&self, width: f32) -> f32 {
        match self {
            Self::Row(row) => row.height(width),
            Self::Slider(row) => row.height(),
            Self::Custom(_, height) => *height,
        }
    }

    fn divider_start(&self) -> f32 {
        match self {
            Self::Row(row) => row.divider_start(),
            Self::Slider(row) => row.text_start(),
            Self::Custom(..) => pad_h(),
        }
    }
}

/// Rows on one rounded card, with an optional caption above it and a note
/// below it.
pub struct Group {
    items: Vec<Item>,
    header: Option<String>,
    footer: Option<String>,
    width: f32,
}

impl Default for Group {
    fn default() -> Self {
        Self::new()
    }
}

impl Group {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            header: None,
            footer: None,
            width: DEFAULT_WIDTH,
        }
    }

    pub fn row(mut self, row: FormRow) -> Self {
        self.items.push(Item::Row(row));
        self
    }

    /// Several rows at once.
    pub fn rows(mut self, rows: impl IntoIterator<Item = FormRow>) -> Self {
        self.items.extend(rows.into_iter().map(Item::Row));
        self
    }

    pub fn slider(mut self, row: SliderRow) -> Self {
        self.items.push(Item::Slider(row));
        self
    }

    /// Anything else, `height` tall, across the card (a list, a picture).
    pub fn custom(mut self, widget: impl Widget + 'static, height: f32) -> Self {
        self.items.push(Item::Custom(boxed(widget), height));
        self
    }

    /// A caption above the card.
    pub fn header(mut self, text: impl Into<String>) -> Self {
        self.header = Some(text.into());
        self
    }

    /// A note below the card, wrapped to its width.
    pub fn footer(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        self.footer = (!text.is_empty()).then_some(text);
        self
    }

    /// As wide as this (504 by default, Shell Settings' page).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The card's height: its rows and the hairlines between them.
    pub fn card_height(&self) -> f32 {
        let rows: f32 = self.items.iter().map(|item| item.height(self.width)).sum();
        rows + self.items.len().saturating_sub(1) as f32
    }

    fn footer_lines(&self) -> Vec<String> {
        self.footer
            .as_ref()
            .map(|text| wrapped_lines(text, note_font(), self.width - pad_h() * 2.0, 6))
            .unwrap_or_default()
    }

    /// The whole height: caption, card and note.
    pub fn height(&self) -> f32 {
        let gap = px(6.0);
        let header = self
            .header
            .as_ref()
            .map_or(0.0, |_| text_height(note_font()) + gap);
        let lines = self.footer_lines().len() as f32;
        let footer = if lines > 0.0 {
            gap + lines * text_height(note_font())
        } else {
            0.0
        };
        header + self.card_height() + footer
    }

    pub fn build(self) -> Column {
        let width = self.width;
        let gap = px(6.0);
        let card_height = self.card_height();
        let footer = self.footer_lines();

        let count = self.items.len();
        let mut rows: Vec<Box<dyn Widget>> = Vec::new();
        for (index, item) in self.items.into_iter().enumerate() {
            let start = item.divider_start();
            rows.push(match item {
                Item::Row(row) => boxed(row.build(width)),
                Item::Slider(row) => boxed(row.build(width)),
                Item::Custom(widget, height) => boxed(
                    Rectangle::new()
                        .width(Size::Parent)
                        .height(Size::Fixed(height))
                        .child(Column::new(vec![widget])),
                ),
            });
            if index + 1 < count {
                // the hairline starts under the title, past the tile
                rows.push(boxed(
                    Rectangle::new()
                        .width(Size::Parent)
                        .height(Size::Fixed(1.0))
                        .padding(Padding {
                            left: start,
                            ..Padding::default()
                        })
                        .child(
                            Rectangle::new()
                                .width(Size::Parent)
                                .height(Size::Fixed(1.0))
                                .fill(theme::divider()),
                        ),
                ));
            }
        }

        let card = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(card_height))
            .radius(theme::group_radius())
            .fill(theme::group_fill())
            .clip()
            .child(Column::new(rows).width(Size::Parent));

        let mut parts: Vec<Box<dyn Widget>> = Vec::new();
        if let Some(header) = self.header {
            parts.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(width))
                    .height(Size::Fixed(text_height(note_font())))
                    .padding(Padding {
                        left: pad_h(),
                        ..Padding::default()
                    })
                    .child(
                        Text::new(header)
                            .style(note_font())
                            .color(theme::label_secondary()),
                    ),
            ));
        }
        parts.push(boxed(card));
        if !footer.is_empty() {
            let footer_height = footer.len() as f32 * text_height(note_font());
            let lines = footer
                .into_iter()
                .map(|line| {
                    boxed(
                        Text::new(line)
                            .style(note_font())
                            .color(theme::label_secondary()),
                    )
                })
                .collect();
            parts.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(width))
                    .height(Size::Fixed(footer_height))
                    .padding(Padding {
                        left: pad_h(),
                        right: pad_h(),
                        ..Padding::default()
                    })
                    .child(Column::new(lines)),
            ));
        }
        Column::new(parts).gap(gap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn switch_row(id: &str) -> FormRow {
        FormRow::new(id, "Setting")
            .icon('\u{e5ca}', Accent::Blue)
            .trailing(
                Rectangle::new()
                    .width(Size::Fixed(36.0))
                    .height(Size::Fixed(16.0)),
            )
    }

    #[test]
    fn a_group_is_as_tall_as_its_rows_and_hairlines() {
        let group = Group::new()
            .row(switch_row("a"))
            .row(switch_row("b").subtitle("A second line"))
            .slider(SliderRow::new("Level", |w| {
                Rectangle::new()
                    .width(Size::Fixed(w))
                    .height(Size::Fixed(24.0))
            }))
            .custom(
                Rectangle::new()
                    .width(Size::Parent)
                    .height(Size::Fixed(30.0)),
                30.0,
            );
        let width = group.width;
        let one = switch_row("a").height(width);
        let two = switch_row("b").subtitle("A second line").height(width);
        assert_eq!(one, theme::row_min_height());
        assert!(two > one);
        let slider = SliderRow::new("Level", |w| {
            Rectangle::new()
                .width(Size::Fixed(w))
                .height(Size::Fixed(24.0))
        })
        .height();
        assert_eq!(group.card_height(), one + two + slider + 30.0 + 3.0);
        assert_eq!(group.height(), group.card_height());
        let _ = group.build();
    }

    #[test]
    fn a_long_footer_wraps_and_adds_its_lines() {
        let words = "word ".repeat(80);
        let group = Group::new()
            .row(switch_row("a"))
            .header("Header")
            .footer(words);
        let lines = group.footer_lines().len();
        assert!(lines > 1, "{lines} lines");
        let line = text_height(note_font());
        let gap = px(6.0);
        let expected = line + gap + group.card_height() + gap + lines as f32 * line;
        assert!((group.height() - expected).abs() < 0.01);
        let _ = group.build();
    }

    #[test]
    fn rows_with_details_and_presses_build() {
        let row = FormRow::new("c", "Open").on_press(|| {}).detail(
            Rectangle::new()
                .width(Size::Parent)
                .height(Size::Fixed(40.0)),
            40.0,
        );
        assert_eq!(
            row.height(DEFAULT_WIDTH),
            theme::row_min_height() + 40.0 + px(10.0)
        );
        let _ = row.disabled(true).build(DEFAULT_WIDTH);
    }
}
