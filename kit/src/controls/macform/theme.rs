//! MacTheme: the colours and sizes the macform controls share.
//!
//! The Quickshell original was dark only. Where a token has a kit
//! counterpart with the same dark value it reads that (so the accent, the
//! labels and light mode follow the shell); the rest keep the original's
//! literal values, with a light twin chosen to read the same on a light
//! window.

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

/// `knob`: a switch's or slider's knob, white in both modes.
pub fn knob() -> Color {
    Color::WHITE
}

/// `fieldFill`: white @ 0.08, the kit's secondary fill.
pub fn field_fill() -> Color {
    material::fill_role(FillLevel::Secondary)
}

/// `fieldBorder`: white @ 0.15 (black @ 0.15 in light).
pub fn field_border() -> Color {
    ink(0.15)
}

/// `controlFill`: white @ 0.12, under push buttons and pull-downs
/// (black @ 0.06 in light, where a darker one reads heavy).
pub fn control_fill() -> Color {
    if dark() { ink(0.12) } else { ink(0.06) }
}

/// `menuFill`: an open menu's panel.
pub fn menu_fill() -> Color {
    if dark() {
        Color::rgb(0x2e, 0x2e, 0x2e)
    } else {
        Color::rgb(0xf6, 0xf6, 0xf6)
    }
}

/// `groupFill`: white @ 0.05, a settings group's card (black @ 0.04 in
/// light, where the window is already white).
pub fn group_fill() -> Color {
    if dark() { ink(0.05) } else { ink(0.04) }
}

/// `divider`: the kit's separator (white @ 0.10).
pub fn divider() -> Color {
    material::separator()
}

/// `labelPrimary`, `labelSecondary`, `labelTertiary`.
pub fn label_primary() -> Color {
    material::label(Level::Primary)
}

pub fn label_secondary() -> Color {
    material::label(Level::Secondary)
}

pub fn label_tertiary() -> Color {
    material::label(Level::Tertiary)
}

/// The red edge of a field that failed its check.
pub fn error() -> Color {
    Color::rgb(0xff, 0x69, 0x61)
}

/// The hover layer: 8% of whatever colour the content is drawn in.
pub fn hover_of(content: Color) -> Color {
    with_alpha(content, 0.08)
}

/// The shadow under raised parts (slider knob, nav pill, menus).
pub fn shadow_color() -> Color {
    with_alpha(Color::BLACK, if dark() { 0.7 } else { 0.35 })
}

pub use crate::color::Accent;

// ---- geometry: the original's Figma pixels, times the density ----

/// One Figma pixel at the current density.
pub fn px(v: f32) -> f32 {
    v * sys::density::factor()
}

pub fn control_height() -> f32 {
    px(24.0)
}

pub fn field_radius() -> f32 {
    px(6.0)
}

pub fn group_radius() -> f32 {
    px(12.0)
}

pub fn row_min_height() -> f32 {
    px(42.0)
}

pub fn row_pad_h() -> f32 {
    px(12.0)
}

pub fn menu_item_height() -> f32 {
    px(24.0)
}

// ---- type ----

/// `controlFont`: 13 medium, the kit's title style.
pub fn control_font() -> TextStyle {
    sys::r#type::title()
}

/// `subtitleFont`: 11 medium.
pub fn subtitle_font() -> TextStyle {
    TextStyle {
        size: 11.0,
        ..sys::r#type::title()
    }
}

/// `pageTitleFont`: 15 semibold.
pub fn page_title_font() -> TextStyle {
    TextStyle {
        size: 15.0,
        weight: 600,
        ..sys::r#type::title()
    }
}
