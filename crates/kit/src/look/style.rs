//! The colours, sizes and fonts the form controls share: the stepper, the
//! checkbox and radio, the dropdown and its menu, text fields, form rows.
//!
//! The look was ported from a dark-only Quickshell set. Where a token has a
//! kit counterpart with the same dark value it reads that (so the accent,
//! the labels and light mode follow the shell); the rest keep the
//! original's literal values, each marked `TODO(token)`, with a light twin
//! chosen to read the same on a light window.

use amane::Color;

use crate::color::with_alpha;
use crate::material::{self, FillLevel, Level};
use crate::theme::{self, Mode};
use crate::tokens::{TextStyle, sys};

fn dark() -> bool {
    theme::mode() == Mode::Dark
}

/// White in dark mode and black in light, at `alpha`.
fn ink(alpha: f32) -> Color {
    let base = if dark() { Color::WHITE } else { Color::BLACK };
    with_alpha(base, alpha)
}

// ---- colours ----

/// `tint`: the accent (#0A84FF in the original).
pub fn tint() -> Color {
    sys::color::accent::bg()
}

/// `glyphOnTint`: what sits on the accent.
pub fn glyph_on_tint() -> Color {
    sys::color::accent::fg()
}

/// `fieldFill`: white @ 0.08, the kit's secondary fill.
pub fn field_fill() -> Color {
    material::fill_role(FillLevel::Secondary)
}

/// `fieldBorder`: white @ 0.15 (black @ 0.15 in light).
// TODO(token): sys.color.field.border
pub fn field_border() -> Color {
    ink(0.15)
}

/// `controlFill`: white @ 0.12, under push buttons and pull-downs
/// (black @ 0.06 in light, where a darker one reads heavy).
// TODO(token): sys.color.control.fill
pub fn control_fill() -> Color {
    if dark() { ink(0.12) } else { ink(0.06) }
}

/// `menuFill`: an open menu's panel.
// TODO(token): sys.color.surface.menu.bg
pub fn menu_fill() -> Color {
    if dark() {
        Color::rgb(0x2e, 0x2e, 0x2e)
    } else {
        Color::rgb(0xf6, 0xf6, 0xf6)
    }
}

/// `groupFill`: a settings group's card, set into the window rather than
/// raised from it: black @ 0.12 in dark mode, @ 0.02 in light.
// TODO(token): sys.color.surface.group.bg
pub fn group_fill() -> Color {
    on_window(with_alpha(Color::BLACK, if dark() { 0.12 } else { 0.02 }))
}

/// `groupBorder`: the card's 1 px edge, black @ 0.15 in dark mode and
/// @ 0.05 in light.
// TODO(token): sys.color.surface.group.border
pub fn group_border() -> Color {
    on_window(with_alpha(Color::BLACK, if dark() { 0.15 } else { 0.05 }))
}

/// `shade` laid over the window's background, as one opaque colour. A
/// vibrant window (amane's split for niri-vibrancy) sends translucent black
/// to a layer the compositor subtracts from the screen, which takes far
/// more off than a shade over the window would; a card set into the window
/// must look the same with vibrancy on or off, so it's drawn opaque.
fn on_window(shade: Color) -> Color {
    crate::color::over(shade, sys::color::window::bg())
}

/// `rowSeparator`: the hairline between a card's rows, softer than
/// [`divider`]: white @ 0.05 in dark mode, black @ 0.07 in light.
// TODO(token): sys.color.separator.soft
pub fn row_separator() -> Color {
    if dark() { ink(0.05) } else { ink(0.07) }
}

/// A pressable row's hover layer: 5% of the label colour in dark mode, 6%
/// in light, lighter than a button's.
pub fn row_hover() -> Color {
    with_alpha(
        material::label(Level::Primary),
        if dark() { 0.05 } else { 0.06 },
    )
}

/// A pressable row while it's held down.
pub fn row_press() -> Color {
    with_alpha(
        material::label(Level::Primary),
        if dark() { 0.12 } else { 0.14 },
    )
}

/// A received message's bubble, as Messages greys it: #3B3B3D in dark
/// mode, #E9E9EB in light.
// TODO(token): a Messages component token
pub fn bubble_received() -> Color {
    if dark() {
        Color::rgb(0x3b, 0x3b, 0x3d)
    } else {
        Color::rgb(0xe9, 0xe9, 0xeb)
    }
}

/// `divider`: the kit's separator (white @ 0.10).
pub fn divider() -> Color {
    material::separator()
}

/// `labelPrimary`, `labelSecondary`, `labelTertiary`.
pub fn label_primary() -> Color {
    material::label(Level::Primary)
}

/// `labelSecondary`: text that says less, like a subtitle.
pub fn label_secondary() -> Color {
    material::label(Level::Secondary)
}

/// `labelTertiary`: text that says least, like a placeholder.
pub fn label_tertiary() -> Color {
    material::label(Level::Tertiary)
}

/// The hover layer: 8% of whatever colour the content is drawn in.
pub fn hover_of(content: Color) -> Color {
    with_alpha(content, 0.08)
}

/// The shadow under raised parts (slider knob, nav pill, menus).
// TODO(token): sys.color.shadow
pub fn shadow_color() -> Color {
    with_alpha(Color::BLACK, if dark() { 0.7 } else { 0.35 })
}

pub use crate::color::Accent;

// ---- geometry: the original's Figma pixels, times the density ----

/// One Figma pixel at the current density.
pub fn px(v: f32) -> f32 {
    v * sys::density::factor()
}

/// How tall a control is: 24 Figma pixels.
pub fn control_height() -> f32 {
    px(24.0)
}

/// A field's or button's corner radius: 6.
pub fn field_radius() -> f32 {
    px(6.0)
}

/// A settings group's corner radius: 14.
pub fn group_radius() -> f32 {
    px(14.0)
}

/// The least a settings row is tall.
pub fn row_min_height() -> f32 {
    px(42.0)
}

/// The least a settings row with a subtitle is tall.
pub fn row_two_line_height() -> f32 {
    px(50.0)
}

/// From a group's caption down to its card.
pub fn group_header_gap() -> f32 {
    px(7.0)
}

/// A settings row's padding at its sides.
pub fn row_pad_h() -> f32 {
    px(12.0)
}

/// How tall a menu row is.
pub fn menu_item_height() -> f32 {
    px(24.0)
}

// ---- type ----

/// `controlFont`: 13 medium, the kit's title style.
pub fn control_font() -> TextStyle {
    sys::r#type::title()
}

/// `subtitleFont`: 11 medium.
// TODO(token): a sys.type role
pub fn subtitle_font() -> TextStyle {
    TextStyle {
        size: 11.0,
        ..sys::r#type::title()
    }
}

/// `pageTitleFont`: 15 bold, as a header bar's title.
// TODO(token): a sys.type role
pub fn page_title_font() -> TextStyle {
    TextStyle {
        size: 15.0,
        weight: 700,
        ..sys::r#type::title()
    }
}

/// A group's caption: bold, at the body's size.
// TODO(token): a sys.type role, once the sys tier has room under its ceiling
pub fn group_header_font() -> TextStyle {
    TextStyle {
        weight: 700,
        ..sys::r#type::label()
    }
}

/// Between a page's groups: 24.
// TODO(token): sys.space, once the sys tier has room under its ceiling
pub fn section_gap() -> f32 {
    px(24.0)
}
