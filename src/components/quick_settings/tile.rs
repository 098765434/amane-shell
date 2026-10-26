//! Quick toggle — a Quick Settings control: a setting's icon, name and
//! state, in one of macOS Control Center's shapes ([`Style`]), each a
//! frosted glass module of its own on the clear card ([`module`]).
//!
//! GNOME's QuickToggle (and QuickMenuToggle, whose arrow opens a menu) in
//! the Control Center look: the glyph on a round disc that turns accent when
//! the setting is on, springing like the switch, with the title and a
//! one-line subtitle beside it. A [`Style::Card`] is a glass capsule of its
//! own, two to four cells wide; a [`Style::Round`] is a glass circle one
//! cell big that is the disc itself, the glyph alone in it, named by a
//! tooltip.
//!
//! As on macOS, the disc toggles and the rest of the tile opens the
//! setting's page (GNOME's arrow); a tile with no page toggles from
//! anywhere. A right-click opens the page too. Like the other controls it
//! shows a click at once, and goes back if the service doesn't follow
//! within the pending timeout. In edit mode a control is inert: it draws,
//! but takes nothing.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use amane::{
    Animation, Blend, Center, Color, Column, Radius, Rectangle, Row, Shadow, Size, Start, Text,
    Widget, request_frame,
};

use super::QS_NS;
use super::grid::{Metrics, Span};
use crate::components::buttons::{
    Actions, Pending, hovered, interactive, press_scale, state_fill, with_existing, with_state,
};
use crate::components::icons::{Glyph, glyph};
use crate::components::layout::{boxed, icon, pad, text_height, text_width};
use crate::components::material::{self, FillLevel, Level, Material};
use crate::components::motion;
use crate::components::tooltip;
use crate::tokens::{Styled, TextStyle, quick, sys};

/// Which of Control Center's shapes a toggle takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// A glass capsule of its own, two to four cells wide.
    Card,
    /// A glass circle a cell big that is the disc, named by its tooltip.
    Round,
}

/// The soft shadow every module casts (`quick.shadow`): vello draws it
/// analytically in the same pass as the module, so it costs little.
fn shadow() -> Option<Shadow> {
    quick::shadow().first().map(|layer| {
        Shadow::drop(layer.color.to_color())
            .blur(layer.blur)
            .offset(layer.x, layer.y)
    })
}

/// A frosted module, in the Control material: its tint and a soft shadow, `width` by
/// `height` with `radius` corners, holding `content`. The compositor
/// frosts behind it where the surface says so (see `glass_region`); where
/// it draws the glass material, the tint and lit edge are its too, and
/// the module is only the shadow and the content. Otherwise
/// the grid lights every module's glass edge in one pass over them all
/// (`glass::rims`): a pass per module (`glass::apply`) cost more than a
/// frame at a dozen modules (measured: +13 ms a frame on an integrated
/// GPU), so only a page's sheet, one piece, wears `glass::apply` itself.
pub(super) fn module(
    width: f32,
    height: f32,
    radius: f32,
    content: impl Widget + 'static,
) -> Rectangle {
    let module = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .radius(radius)
        .fill(fill(Material::Control, Material::Control.tint()))
        .clip()
        .child(content);
    // a plain module sits in its sheet, casting nothing on it
    match shadow() {
        Some(shadow) if !PLAIN.get() => module.shadow(shadow),
        _ => module,
    }
}

/// A whole sheet of `material`, tinted `tint` (the material's own, or on
/// its way there from a module's), with the surfaces' full glass edge.
pub(super) fn sheet(
    width: f32,
    height: f32,
    radius: f32,
    material: Material,
    tint: Color,
    content: impl Widget + 'static,
) -> Rectangle {
    let surface = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .radius(radius)
        .fill(fill(material, tint))
        .clip()
        .child(content);
    let surface = match shadow() {
        Some(shadow) => surface.shadow(shadow),
        None => surface,
    };
    material.lit(surface, radius)
}

thread_local! {
    // whether the modules being built are plain: tinted, never frosted
    static PLAIN: Cell<bool> = const { Cell::new(false) };
}

/// Builds `f` with every module in it plain: the panels' tint laid on by
/// the module itself, whether or not the compositor would frost it, for a
/// control shown somewhere it isn't glass (the Customize window's catalog,
/// where the surface asks for no frost behind it).
pub(super) fn plain<R>(f: impl FnOnce() -> R) -> R {
    let previous = PLAIN.replace(true);
    let out = f();
    PLAIN.set(previous);
    out
}

/// What `material` tinted `tint` is filled with: the tint, unless the
/// compositor lays it on with the frost. A plain module on a sheet is a
/// translucent fill on it instead, as Control Center's catalog shows its
/// controls: the sheet's colour comes through, not a milky tint over it.
fn fill(material: Material, tint: Color) -> Color {
    if PLAIN.get() && material != Material::Prominent {
        material::fill_role(FillLevel::Secondary)
    } else if PLAIN.get() || !material.compositor_draws() {
        tint
    } else {
        Color::TRANSPARENT
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
    glyph: Glyph,
    title: String,
    subtitle: Option<String>,
    checked: bool,
    disabled: bool,
    inert: bool,
    // the disc in the critical colour: a battery running low
    alert: bool,
    style: Style,
    span: Span,
    toggle: Option<Rc<dyn Fn(bool)>>,
    press: Option<Rc<dyn Fn()>>,
    open: Option<Rc<dyn Fn()>>,
}

impl QuickToggle {
    pub fn new(id: impl Into<String>, glyph: impl Into<Glyph>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            glyph: glyph.into(),
            title: title.into(),
            subtitle: None,
            checked: false,
            disabled: false,
            inert: false,
            alert: false,
            style: Style::Card,
            span: Span::M,
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

    /// The disc in the critical colour, for a state that wants attention.
    pub fn alert(mut self, alert: bool) -> Self {
        self.alert = alert;
        self
    }

    /// Drawn but taking nothing: no hover, press or tooltip (edit mode).
    pub fn inert(mut self, inert: bool) -> Self {
        self.inert = inert;
        self
    }

    /// Its shape; a card by default.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// How many cells a card takes; two across by default.
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

    /// How tall a card or a row is: one cell.
    pub fn height() -> f32 {
        Metrics::tokens().cell
    }

    /// How tall (and wide) a round one is: one cell.
    pub fn round_size() -> f32 {
        Metrics::tokens().cell
    }

    /// Makes `base` take the pointer, unless the tile is inert.
    fn act(&self, base: Rectangle, id: String, actions: Actions) -> Rectangle {
        if self.inert {
            return base;
        }
        interactive(base, id, self.disabled, actions)
    }

    /// The same for a part of a disabled tile: the tile is dimmed as a
    /// whole, so the part takes nothing but isn't dimmed again.
    fn act_part(&self, base: Rectangle, id: String, actions: Actions) -> Rectangle {
        if self.inert || self.disabled {
            return base;
        }
        interactive(base, id, false, actions)
    }

    /// Names a tile whose lines are too long for it, while hovered.
    fn offer_full_text(&self, label_w: f32) {
        if self.disabled || self.inert || !hovered(&self.id) {
            return;
        }
        let long = |text: &str, style| text_width(text, style) > label_w;
        let subtitle = self.subtitle.as_deref().unwrap_or("");
        if long(&self.title, quick::r#type::title()) || long(subtitle, quick::r#type::caption()) {
            let name = if subtitle.is_empty() {
                self.title.clone()
            } else {
                format!("{} · {subtitle}", self.title)
            };
            tooltip::offer(&self.id, QS_NS, name);
        }
    }

    pub fn build(self) -> Rectangle {
        let (shown, progress) = self.animate();

        // a control that is on goes white with its glyph in the accent
        // (`sys.color.state.on`), as Control Center's do
        let (disc_fill, glyph_color) = if self.alert {
            (sys::color::status::critical::bg(), sys::color::accent::fg())
        } else {
            (
                Color::blend(
                    sys::color::state::selected(),
                    sys::color::state::on::bg(),
                    progress,
                ),
                Color::blend(
                    material::label(Level::Primary),
                    sys::color::state::on::fg(),
                    progress,
                ),
            )
        };

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

        if self.style == Style::Round {
            return self.build_round(progress, glyph_color, disc_press);
        }

        let disc_side = quick::tile::disc();
        let height = Self::height();
        // a card is padded as the reference: the disc `pad` in from the
        // capsule's end, the label `gap` after it
        let metrics = Metrics::tokens();
        let width = metrics.width(self.span);
        let inset = quick::tile::pad();
        let gap = quick::tile::gap();
        let quiet = self.disabled || self.inert;

        let disc_id = format!("{}.disc", self.id);
        let disc = Rectangle::new()
            .width(Size::Fixed(disc_side))
            .height(Size::Fixed(disc_side))
            .radius(Radius::Full)
            .fill(state_fill(&disc_id, disc_fill, quiet))
            .scale(press_scale(&disc_id, quick::press::scale()))
            .align_child(Center, Center)
            .child(glyph(self.glyph, quick::tile::glyph(), glyph_color));
        let disc = self.act_part(
            disc,
            disc_id,
            Actions {
                press: disc_press,
                secondary: self.open.clone(),
                ..Actions::default()
            },
        );

        let label_w = (width - inset * 2.0 - disc_side - gap).max(1.0);
        self.offer_full_text(label_w);
        let mut labels = vec![boxed(line(
            self.title.clone(),
            quick::r#type::title(),
            material::label(Level::Primary),
            label_w,
        ))];
        if let Some(subtitle) = &self.subtitle {
            labels.push(boxed(line(
                subtitle.clone(),
                quick::r#type::caption(),
                material::label(Level::Secondary),
                label_w,
            )));
        }

        let content = Row::new(vec![boxed(disc), boxed(Column::new(labels))])
            .gap(gap)
            .align(Center);

        let inner = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height))
            .radius(Radius::Full)
            .fill(state_fill(&self.id, Color::TRANSPARENT, quiet))
            .padding(pad(inset, 0.0))
            .align_child(Start, Center)
            .clip()
            .child(content);

        let tile = module(width, height, height / 2.0, inner)
            .scale(press_scale(&self.id, quick::press::scale()));

        let actions = Actions {
            press: tile_press,
            secondary: self.open.clone(),
            ..Actions::default()
        };
        let id = self.id.clone();
        self.act(tile, id, actions)
    }

    /// A round button: the glass circle is the disc, turning accent when
    /// the setting is on, with the glyph alone in it and its name in a
    /// tooltip.
    fn build_round(
        self,
        progress: f32,
        glyph_color: Color,
        press: Option<Rc<dyn Fn()>>,
    ) -> Rectangle {
        let quiet = self.disabled || self.inert;
        if !quiet && hovered(&self.id) {
            let name = match &self.subtitle {
                Some(subtitle) => format!("{}: {subtitle}", self.title),
                None => self.title.clone(),
            };
            tooltip::offer(&self.id, QS_NS, name);
        }

        let side = Self::round_size();
        let tint = Color::blend(Color::TRANSPARENT, sys::color::state::on::bg(), progress);
        let inner = Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(Radius::Full)
            .fill(state_fill(&self.id, tint, quiet))
            .align_child(Center, Center)
            .child(glyph(self.glyph, quick::button::glyph(), glyph_color));
        let button = module(side, side, side / 2.0, inner)
            .scale(press_scale(&self.id, quick::press::scale()));

        let actions = Actions {
            press,
            secondary: self.open.clone(),
            ..Actions::default()
        };
        let id = self.id.clone();
        self.act(button, id, actions)
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

/// One elided line of text, `width` wide, on the reference's line height
/// (`quick.type.line`; amane's text has no leading of its own).
pub(super) fn line(text: String, style: TextStyle, color: Color, width: f32) -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(quick::r#type::line().max(text_height(style))))
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .clip()
        .child(Text::new(text).style(style).color(color).elide())
}

/// How wide [`capsule_button`] draws `label`.
pub(super) fn capsule_width(label: &str, glyph: bool) -> f32 {
    let glyph_w = if glyph {
        sys::size::icon::s() + sys::space::gap::xs()
    } else {
        0.0
    };
    text_width(label, sys::r#type::label()) + glyph_w + sys::space::inset::l() * 2.0
}

/// A glass capsule button with a label: "Edit Controls", "Add a Control".
pub(super) fn capsule_button(
    id: &'static str,
    glyph: Option<char>,
    label: &str,
    accent: bool,
    press: impl Fn() + 'static,
) -> Rectangle {
    let height = sys::size::control::height();
    let style = sys::r#type::label();
    let width = capsule_width(label, glyph.is_some());
    // the one main action of the moment (Done) is in the accent: glass
    // tinted with it, which the compositor lays on with the frost where it
    // draws the material (`tiles::grid` asks), else a fill of it
    let (fill, fg) = if accent {
        (
            fill(Material::Prominent, Material::Prominent.tint()),
            sys::color::accent::fg(),
        )
    } else {
        (Color::TRANSPARENT, material::label(Level::Primary))
    };

    let mut content: Vec<Box<dyn Widget>> = Vec::new();
    if let Some(glyph) = glyph {
        content.push(boxed(icon(glyph, sys::size::icon::s(), fg)));
    }
    content.push(boxed(Text::new(label).style(style).color(fg)));
    let inner = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .radius(Radius::Full)
        .fill(state_fill(id, fill, false))
        .align_child(Center, Center)
        .child(Row::new(content).gap(sys::space::gap::xs()).align(Center));

    let button =
        module(width, height, height / 2.0, inner).scale(press_scale(id, quick::press::scale()));
    interactive(
        button,
        id.to_string(),
        false,
        Actions {
            press: Some(Rc::new(press)),
            ..Actions::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Density, Mode};
    use crate::tokens::{Key, with_key};

    #[test]
    fn the_cells_of_a_row_and_their_gaps_fill_it() {
        for key in Key::all() {
            with_key(key, || {
                let px = Metrics::tokens();
                assert_eq!(px.width(Span::M) * 2.0 + px.gap, px.width(Span::FULL));
                assert_eq!(px.width(Span::S) * 4.0 + px.gap * 3.0, px.width(Span::FULL));
                // a round button is exactly its disc and the padding
                assert_eq!(px.cell, quick::tile::disc() + quick::tile::pad() * 2.0);
            });
        }
        let key = Key::new(Mode::Dark, Density::Comfortable);
        assert_eq!(with_key(key, || Metrics::tokens().width(Span::M)), 164.0);
    }
}
