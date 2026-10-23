//! The context menu's tokens from Penpot (`design-tokens/tokens.json`):
//! `sys.*` roles plus its own `menu.*` tokens.

use amane::Color;

use crate::tokens::font::first_installed;
use crate::tokens::{Cache, Reader, TokenError, cached};

/// Colour roles (they follow light/dark mode).
#[derive(Clone, Copy)]
pub struct ThemeTokens {
    pub surface: Color,
    pub border: Color,
    pub fg: Color,
    pub muted: Color,
    pub hover_fill: Color,
    pub hover_destructive_fill: Color,
    pub selected_fill: Color,
    pub on_selected_fg: Color,
    pub accent: Color,
    pub separator: Color,
    pub destructive: Color,
}

pub struct Tokens {
    pub radius: f32,
    pub radius_row: f32,
    pub card_padding: f32,
    pub row_pad_x: f32,
    pub gap: f32,
    pub trailing_gap: f32,
    pub shortcut_gap: f32,
    pub row_height: f32,
    pub icon_slot: f32,
    pub chevron_slot: f32,
    pub hairline: f32,
    pub separator_margin: f32,
    pub min_width: f32,
    pub max_width: f32,
    pub submenu_offset: f32,
    pub font_family: String,
    pub font_regular: u16,
    pub label_size: f32,
    pub shortcut_size: f32,
    pub shortcut_hover_alpha: f32,
    pub advance_ratio: f32,
}

pub fn tokens() -> &'static Tokens {
    static CACHE: Cache<Tokens> = Cache::new();
    cached(&CACHE, "menu", |r| parse(&r.sys(), &r.component("menu")))
}

pub fn theme_tokens() -> &'static ThemeTokens {
    static CACHE: Cache<ThemeTokens> = Cache::new();
    cached(&CACHE, "menu theme", |r| parse_theme(&r.sys()))
}

pub fn parse(sys: &Reader, menu: &Reader) -> Result<Tokens, TokenError> {
    let label = sys.typography("type.label")?;
    let size = label.size.unwrap_or_default();

    Ok(Tokens {
        radius: sys.radius("radius.popover")?,
        radius_row: menu.radius("row.radius")?,
        card_padding: menu.spacing("card.padding")?,
        row_pad_x: sys.spacing("space.inset.l")?,
        gap: sys.spacing("space.gap.m")?,
        trailing_gap: sys.spacing("space.gap.s")?,
        shortcut_gap: sys.spacing("space.gap.xl")?,
        row_height: menu.sizing("row.height")?,
        icon_slot: menu.sizing("icon-slot.width")?,
        chevron_slot: menu.sizing("chevron-slot.width")?,
        hairline: sys.stroke_width("border.hairline")?,
        separator_margin: sys.spacing("space.gap.xs")?,
        min_width: menu.sizing("min-width")?,
        max_width: menu.sizing("max-width")?,
        submenu_offset: menu.spacing("submenu.offset")?,
        font_family: first_installed(&label.families),
        font_regular: label.weight.map_or(400, |(w, _)| w),
        label_size: size,
        shortcut_size: size,
        shortcut_hover_alpha: menu.opacity("shortcut.hover-opacity")?,
        advance_ratio: menu.number("label.advance-ratio")?,
    })
}

pub fn parse_theme(sys: &Reader) -> Result<ThemeTokens, TokenError> {
    let accent = sys.color("color.accent.bg")?;

    Ok(ThemeTokens {
        surface: sys.color("color.surface.popover.bg")?,
        border: sys.color("color.surface.popover.stroke")?,
        fg: sys.color("color.surface.popover.fg")?,
        muted: sys.color("color.surface.popover.fg-secondary")?,
        hover_fill: sys.color("color.state.hover")?,
        hover_destructive_fill: sys.color("color.status.critical.hover")?,
        selected_fill: accent,
        on_selected_fg: sys.color("color.accent.fg")?,
        accent,
        separator: sys.color("color.separator")?,
        destructive: sys.color("color.status.critical.text")?,
    })
}
