//! The taskbar family's tokens from Penpot (`design-tokens/tokens.json`).
//! [`Tokens`] holds the `sys.*` roles the bar, Wi-Fi and power panels share;
//! [`BarTokens`] and [`WifiTokens`] hold each component's own tokens.

use amane::Color;

use crate::tokens::font::first_installed;
use crate::tokens::{Cache, Reader, TokenError, cached};

/// Colour roles (they follow light/dark mode).
#[derive(Clone, Copy)]
pub struct ThemeTokens {
    pub surface: Color,
    pub fg: Color,
    pub fg_strong: Color,
    pub pill: Color,
    pub pill_focused: Color,
    pub accent: Color,
    pub urgent: Color,
}

pub struct Tokens {
    pub height_compact: f32,
    pub radius_pill: f32,
    pub radius_popover: f32,
    pub inset_m: f32,
    pub inset_l: f32,
    pub gap_2xs: f32,
    pub gap_xs: f32,
    pub gap_s: f32,
    pub gap_m: f32,
    pub gap_l: f32,
    pub gap_xl: f32,
    pub icon_size: f32,
    pub icon_small: f32,
    pub panel_w: f32,
    pub panel_h: f32,
    pub list_h: f32,
    pub row_h: f32,
    pub row_trailing_w: f32,
    pub control_h: f32,
    pub blur: f32,
    pub font_family: String,
    pub font_icon: String,
    pub weight_medium: u16,
    pub label_size: f32,
    pub title_size: f32,
    pub clock_size: f32,
    pub caption_size: f32,
    pub theme: ThemeTokens,
}

pub struct BarTokens {
    pub height_overview: f32,
    pub radius: f32,
    pub pad_y: f32,
    pub pill_pad_y: f32,
    pub dot_size: f32,
    pub dot_active: f32,
    pub indicator_w: f32,
    pub window_max: f32,
    pub window_radius: f32,
    pub volume_w: f32,
}

pub struct WifiTokens {
    pub switch_w: f32,
    pub switch_h: f32,
    pub icon_button_w: f32,
    pub button_w: f32,
}

pub fn tokens() -> &'static Tokens {
    static CACHE: Cache<Tokens> = Cache::new();
    cached(&CACHE, "taskbar", |r| parse(&r.sys()))
}

pub fn bar_tokens() -> &'static BarTokens {
    static CACHE: Cache<BarTokens> = Cache::new();
    cached(&CACHE, "bar", |r| parse_bar(&r.component("bar")))
}

pub fn wifi_tokens() -> &'static WifiTokens {
    static CACHE: Cache<WifiTokens> = Cache::new();
    cached(&CACHE, "wifi", |r| parse_wifi(&r.component("wifi")))
}

pub fn parse(sys: &Reader) -> Result<Tokens, TokenError> {
    let label = sys.typography("type.label")?;
    let title = sys.typography("type.title")?;

    Ok(Tokens {
        height_compact: sys.sizing("size.panel.height")?,
        radius_pill: sys.radius("radius.raised")?,
        radius_popover: sys.radius("radius.popover")?,
        inset_m: sys.spacing("space.inset.m")?,
        inset_l: sys.spacing("space.inset.l")?,
        gap_2xs: sys.spacing("space.gap.2xs")?,
        gap_xs: sys.spacing("space.gap.xs")?,
        gap_s: sys.spacing("space.gap.s")?,
        gap_m: sys.spacing("space.gap.m")?,
        gap_l: sys.spacing("space.gap.l")?,
        gap_xl: sys.spacing("space.gap.xl")?,
        icon_size: sys.sizing("size.icon.m")?,
        icon_small: sys.sizing("size.icon.s")?,
        panel_w: sys.sizing("size.popover.width")?,
        panel_h: sys.sizing("size.popover.height")?,
        list_h: sys.sizing("size.list.max-height")?,
        row_h: sys.sizing("size.row.height")?,
        row_trailing_w: sys.sizing("size.row.trailing-width")?,
        control_h: sys.sizing("size.control.height")?,
        blur: sys.number("effect.backdrop-blur")?,
        font_icon: first_installed(&sys.font_family("type.icon")?),
        weight_medium: title.weight.map_or(400, |(w, _)| w),
        label_size: label.size.unwrap_or_default(),
        title_size: title.size.unwrap_or_default(),
        clock_size: sys.typography("type.numeric")?.size.unwrap_or_default(),
        caption_size: sys.typography("type.caption")?.size.unwrap_or_default(),
        font_family: first_installed(&label.families),
        theme: ThemeTokens {
            surface: sys.color("color.surface.panel.bg")?,
            fg: sys.color("color.content.secondary")?,
            fg_strong: sys.color("color.surface.panel.fg")?,
            pill: sys.color("color.surface.raised.bg")?,
            pill_focused: sys.color("color.state.selected")?,
            accent: sys.color("color.accent.bg")?,
            urgent: sys.color("color.status.critical.bg")?,
        },
    })
}

pub fn parse_bar(bar: &Reader) -> Result<BarTokens, TokenError> {
    Ok(BarTokens {
        height_overview: bar.sizing("overview.height")?,
        radius: bar.radius("radius")?,
        pad_y: bar.spacing("panel.padding-y")?,
        pill_pad_y: bar.spacing("pill.padding-y")?,
        dot_size: bar.sizing("workspace-dot.size")?,
        dot_active: bar.sizing("workspace-dot.active-size")?,
        indicator_w: bar.sizing("indicator.width")?,
        window_max: bar.sizing("window-item.max-width")?,
        window_radius: bar.radius("window-item.radius")?,
        volume_w: bar.sizing("volume.width")?,
    })
}

pub fn parse_wifi(wifi: &Reader) -> Result<WifiTokens, TokenError> {
    Ok(WifiTokens {
        switch_w: wifi.sizing("switch.width")?,
        switch_h: wifi.sizing("switch.height")?,
        icon_button_w: wifi.sizing("icon-button.width")?,
        button_w: wifi.sizing("button.width")?,
    })
}
