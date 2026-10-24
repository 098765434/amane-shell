//! Quick toggle — a Quick Settings tile: a setting's icon, name and state.
//!
//! GNOME's QuickToggle (and QuickMenuToggle, whose arrow opens a menu) in
//! the macOS Control Center look: a rounded `surface.raised` tile with the
//! glyph on a round disc, the title and a one-line subtitle beside it. The
//! disc turns accent when the setting is on, springing like the switch.
//!
//! As on macOS, the disc toggles and the rest of the tile opens the
//! setting's page (GNOME's arrow); a tile with no page toggles from
//! anywhere. A right-click opens the page too. Like the other controls it
//! shows a click at once, and goes back if the service doesn't follow
//! within the pending timeout.

use std::rc::Rc;
use std::time::Instant;

use amane::{
    Animation, Blend, Center, Color, Column, Radius, Rectangle, Row, Size, Start, Text,
    request_frame,
};

use crate::components::buttons::{
    Actions, Pending, interactive, state_fill, with_existing, with_state,
};
use crate::components::layout::{boxed, icon, inner_width, pad, text_height};
use crate::components::motion;
use crate::tokens::{Styled, TextStyle, quick, sys};

/// How much of a row a tile takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    Half,
    Full,
}

impl Span {
    /// Its width in a Quick Settings card: half the row less half the gap, or all of it.
    pub fn width(self) -> f32 {
        let row = inner_width(sys::size::popover::width());
        match self {
            Self::Half => (row - sys::space::gap::s()) / 2.0,
            Self::Full => row,
        }
    }
}

/// One tile's motion, kept across rebuilds under its id.
struct Motion {
    // 0.0 off … 1.0 on
    disc: Animation,
    pending: Pending<bool>,
}

/// A tile. `id` keys its motion and hover, so it must be unique and stable.
pub struct QuickToggle {
    id: String,
    glyph: char,
    title: String,
    subtitle: Option<String>,
    checked: bool,
    disabled: bool,
    span: Span,
    toggle: Option<Rc<dyn Fn(bool)>>,
    press: Option<Rc<dyn Fn()>>,
    open: Option<Rc<dyn Fn()>>,
}

impl QuickToggle {
    pub fn new(id: impl Into<String>, glyph: char, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            glyph,
            title: title.into(),
            subtitle: None,
            checked: false,
            disabled: false,
            span: Span::Half,
            toggle: None,
            press: None,
            open: None,
        }
    }

    /// The line under the title: what it's connected to, until when, …
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// On: the disc is accent.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Dimmed and unclickable.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// What a click asks for: the other state.
    pub fn on_toggle(mut self, toggle: impl Fn(bool) + 'static) -> Self {
        self.toggle = Some(Rc::new(toggle));
        self
    }

    /// For a tile that's an action rather than an on/off setting, such as
    /// stopping a hotspot: what a click does.
    pub fn on_press(mut self, press: impl Fn() + 'static) -> Self {
        self.press = Some(Rc::new(press));
        self
    }

    /// Opens the setting's page.
    pub fn on_open(mut self, open: impl Fn() + 'static) -> Self {
        self.open = Some(Rc::new(open));
        self
    }

    pub fn height() -> f32 {
        quick::tile::height()
    }

    pub fn build(self) -> Rectangle {
        let width = self.span.width();
        let height = Self::height();
        let disc_side = quick::tile::disc();
        let gap = sys::space::gap::s();

        let (shown, progress) = self.animate();

        let disc_fill = Color::blend(
            sys::color::state::selected(),
            sys::color::accent::bg(),
            progress,
        );
        let glyph_color = Color::blend(
            sys::color::surface::panel::fg(),
            sys::color::accent::fg(),
            progress,
        );

        // what each part does: the disc switches, the rest opens the page
        let toggle = self.toggle.clone().map(|toggle| {
            let id = self.id.clone();
            let next = !shown;
            Rc::new(move || {
                clicked(&id, next);
                toggle(next);
            }) as Rc<dyn Fn()>
        });
        let act = toggle.or(self.press.clone());
        let disc_press = act.clone().or(self.open.clone());
        let tile_press = self.open.clone().or(act);

        let disc_id = format!("{}.disc", self.id);
        let disc = Rectangle::new()
            .width(Size::Fixed(disc_side))
            .height(Size::Fixed(disc_side))
            .radius(Radius::Full)
            .fill(state_fill(&disc_id, disc_fill, self.disabled))
            .align_child(Center, Center)
            .child(icon(self.glyph, sys::size::icon::m(), glyph_color));
        let disc = interactive(
            disc,
            disc_id,
            self.disabled,
            Actions {
                press: disc_press,
                secondary: self.open.clone(),
                scroll: None,
            },
        );

        let label_w = (width - disc_side - gap * 3.0).max(1.0);
        let mut labels = vec![boxed(line(
            self.title,
            sys::r#type::title(),
            sys::color::surface::panel::fg(),
            label_w,
        ))];
        if let Some(subtitle) = self.subtitle {
            labels.push(boxed(line(
                subtitle,
                sys::r#type::caption(),
                sys::color::content::secondary(),
                label_w,
            )));
        }

        let content = Row::new(vec![
            boxed(disc),
            boxed(Column::new(labels).gap(sys::space::gap::_2xs())),
        ])
        .gap(gap)
        .align(Center);

        let tile = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height))
            .radius(quick::tile::radius())
            .fill(state_fill(
                &self.id,
                sys::color::surface::raised::bg(),
                self.disabled,
            ))
            .padding(pad(gap, 0.0))
            .align_child(Start, Center)
            .clip()
            .child(content);

        interactive(
            tile,
            self.id,
            self.disabled,
            Actions {
                press: tile_press,
                secondary: self.open,
                scroll: None,
            },
        )
    }

    /// Steps the disc towards what should show: the state, or a pending
    /// click laid over it. Gives that state and the disc's colour mix.
    fn animate(&self) -> (bool, f32) {
        let init = || Motion {
            // at rest on the current state, so opening the panel doesn't animate
            disc: motion::spring(if self.checked { 1.0 } else { 0.0 }),
            pending: Pending::NONE,
        };

        with_state(&self.id, init, |motion: &mut Motion| {
            let shown = motion.pending.shown(self.checked, Instant::now());
            motion.disc.to(if shown { 1.0 } else { 0.0 });
            (shown, motion.disc.value().clamp(0.0, 1.0))
        })
    }
}

/// Shows a click before the service answers it.
fn clicked(id: &str, on: bool) {
    with_existing(id, |motion: &mut Motion| {
        motion.pending.set(on, Instant::now())
    });
    request_frame();
}

/// One elided line of text, `width` wide.
pub(super) fn line(text: String, style: TextStyle, color: Color, width: f32) -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(text_height(style)))
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .clip()
        .child(Text::new(text).style(style).color(color).elide())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Density, Mode};
    use crate::tokens::{Key, with_key};

    #[test]
    fn two_half_tiles_and_their_gap_fill_a_row() {
        for key in Key::all() {
            with_key(key, || {
                let row = Span::Half.width() * 2.0 + sys::space::gap::s();
                assert_eq!(row, Span::Full.width());
            });
        }
        let key = Key::new(Mode::Dark, Density::Comfortable);
        assert_eq!(with_key(key, || Span::Half.width()), 147.0);
    }
}
