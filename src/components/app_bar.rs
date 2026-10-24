//! Top app bar — the title row of a panel or sheet.
//!
//! Modelled on Material 3's small top app bar: an optional navigation icon
//! (the panel's glyph), a title with an optional subtitle, and trailing
//! action items. A page drilled into from another (Quick Settings' pages)
//! leads with M3's back button too, a chevron as on macOS. Unlike M3's it's
//! no taller than its content and has no fill of its own: it sits at the
//! top of a [`Card`](super::card::Card), in that card's surface colours.

use std::rc::Rc;

use amane::{Center, Color, Column, Row, Start, Text, Widget};

use crate::components::buttons::IconButton;
use crate::components::card::Surface;
use crate::components::icons::ICON_CHEVRON_LEFT;
use crate::components::layout::{boxed, clipped_label, icon, split_row};
use crate::tokens::{Styled, sys};

pub struct TopAppBar {
    title: String,
    subtitle: Option<String>,
    navigation_icon: Option<char>,
    back: Option<(String, Rc<dyn Fn()>)>,
    surface: Surface,
    color: Option<Color>,
    actions: Vec<Box<dyn Widget>>,
    // the title's and subtitle's width, when they're to be elided
    text_width: Option<f32>,
}

impl TopAppBar {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            navigation_icon: None,
            back: None,
            surface: Surface::Panel,
            color: None,
            actions: Vec::new(),
            text_width: None,
        }
    }

    /// A glyph before the title.
    pub fn navigation_icon(mut self, glyph: char) -> Self {
        self.navigation_icon = Some(glyph);
        self
    }

    /// A back chevron first, which runs `back`. `id` keys its hover, so it
    /// must be unique and stable.
    pub fn back(mut self, id: impl Into<String>, back: impl Fn() + 'static) -> Self {
        self.back = Some((id.into(), Rc::new(back)));
        self
    }

    /// A secondary line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The surface it sits on, whose `fg` and secondary it uses;
    /// `Surface::Panel` by default.
    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    /// The title and icon colour, in place of the surface's `fg` (a status
    /// colour, say).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// The controls at the trailing end, `gap.s` apart.
    pub fn actions(mut self, actions: Vec<Box<dyn Widget>>) -> Self {
        self.actions = actions;
        self
    }

    /// Elides the title and subtitle to `width`, for a long network or
    /// device name.
    pub fn elide_to(mut self, width: f32) -> Self {
        self.text_width = Some(width);
        self
    }

    pub fn build(self) -> Row {
        let color = self.color.unwrap_or(self.surface.fg());

        let line = |text: String, style, color| -> Box<dyn Widget> {
            match self.text_width {
                Some(width) => boxed(clipped_label(text, style, color, width, Start)),
                None => boxed(Text::new(text).style(style).color(color)),
            }
        };
        let title = line(self.title, sys::r#type::title(), color);
        let title: Box<dyn Widget> = match self.subtitle {
            Some(subtitle) => boxed(
                Column::new(vec![
                    title,
                    line(
                        subtitle,
                        sys::r#type::caption(),
                        self.surface.fg_secondary(),
                    ),
                ])
                .gap(sys::space::gap::_2xs()),
            ),
            None => title,
        };

        let mut leading = Vec::new();
        if let Some((id, back)) = self.back {
            leading.push(boxed(
                IconButton::new(id, ICON_CHEVRON_LEFT)
                    .color(color)
                    .on_press(move || back())
                    .build(),
            ));
        }
        if let Some(glyph) = self.navigation_icon {
            leading.push(boxed(icon(glyph, sys::size::icon::m(), color)));
        }
        leading.push(title);

        split_row(
            Row::new(leading).gap(sys::space::gap::s()).align(Center),
            self.actions,
        )
    }
}
