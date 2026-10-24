//! Detail page — what a tile opens: GNOME's submenu, drawn as a macOS
//! Control Center page that slides over the grid.
//!
//! A back chevron, the setting's glyph and name (and what it's doing), an
//! optional switch for it, then its body — networks, devices, choices —
//! and an optional link to the app that has the rest of its settings.
//! Bodies say how tall they are, so the surface can spring to fit the page.

use std::rc::Rc;

use amane::{Center, Color, Column, Parent, Rectangle, Size, Start, Widget};

use super::back;
use crate::components::app_bar::TopAppBar;
use crate::components::buttons::Switch;
use crate::components::icons::ICON_CHEVRON_RIGHT;
use crate::components::layout::{boxed, icon};
use crate::components::list::ListItem;
use crate::tokens::sys;

/// The header's switch: whether it's on, and what flipping it asks for.
type HeaderSwitch = (bool, Rc<dyn Fn(bool)>);

pub struct DetailPage {
    id: &'static str,
    glyph: char,
    title: String,
    subtitle: Option<String>,
    switch: Option<HeaderSwitch>,
    body: Vec<Box<dyn Widget>>,
    body_height: f32,
    footer: Option<(String, Rc<dyn Fn()>)>,
}

impl DetailPage {
    /// `id` prefixes its controls' ids, so it must be unique.
    pub fn new(id: &'static str, glyph: char, title: impl Into<String>) -> Self {
        Self {
            id,
            glyph,
            title: title.into(),
            subtitle: None,
            switch: None,
            body: Vec::new(),
            body_height: 0.0,
            footer: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// A switch at the header's end for the setting itself.
    pub fn switch(mut self, on: bool, toggle: impl Fn(bool) + 'static) -> Self {
        self.switch = Some((on, Rc::new(toggle)));
        self
    }

    /// What the page shows, `height` tall (it's clipped to that).
    pub fn body(mut self, body: Vec<Box<dyn Widget>>, height: f32) -> Self {
        self.body = body;
        self.body_height = height;
        self
    }

    /// A last row that opens the app with the rest of the settings.
    pub fn footer(mut self, label: impl Into<String>, open: impl Fn() + 'static) -> Self {
        self.footer = Some((label.into(), Rc::new(open)));
        self
    }

    /// How tall the page is, inside the card's inset.
    pub fn height(&self) -> f32 {
        let gap = sys::space::gap::l();
        let footer = if self.footer.is_some() {
            gap + sys::size::row::height()
        } else {
            0.0
        };
        sys::size::row::height() + gap + self.body_height + footer
    }

    pub fn build(self) -> Column {
        let height = self.height();

        let mut bar = TopAppBar::new(self.title)
            .back(format!("{}.back", self.id), back)
            .navigation_icon(self.glyph);
        if let Some(subtitle) = self.subtitle {
            bar = bar.subtitle(subtitle);
        }
        if let Some((on, toggle)) = self.switch {
            bar = bar.actions(vec![boxed(
                Switch::new(format!("{}.switch", self.id), on)
                    .on_toggle(move |on| toggle(on))
                    .build(),
            )]);
        }
        let header = Rectangle::new()
            .width(Parent)
            .height(Size::Fixed(sys::size::row::height()))
            .fill(Color::TRANSPARENT)
            .align_child(Start, Center)
            .child(bar.build());

        let body = Rectangle::new()
            .width(Parent)
            .height(Size::Fixed(self.body_height))
            .fill(Color::TRANSPARENT)
            .clip()
            .child(
                Column::new(self.body)
                    .gap(sys::space::gap::l())
                    .width(Parent),
            );

        let mut rows = vec![boxed(header), boxed(body)];
        if let Some((label, open)) = self.footer {
            rows.push(boxed(
                ListItem::new(format!("{}.footer", self.id), label)
                    .trailing(icon(
                        ICON_CHEVRON_RIGHT,
                        sys::size::icon::m(),
                        sys::color::content::secondary(),
                    ))
                    .on_press(move || open())
                    .build(),
            ));
        }

        Column::new(rows)
            .gap(sys::space::gap::l())
            .width(Parent)
            .height(Size::Fixed(height))
    }
}
