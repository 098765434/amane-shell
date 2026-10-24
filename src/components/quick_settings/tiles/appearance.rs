//! The screen — GNOME's brightness slider (only with a backlight), Night
//! Light (with its page: how warm) and Dark Style.

use amane::{Brightness, Service};

use super::{GridCell, on_off};
use crate::components::icons::{ICON_BRIGHTNESS, ICON_DARK_MODE, ICON_NIGHT_LIGHT};
use crate::components::layout::{boxed, caption};
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::grid::Cell;
use crate::components::quick_settings::tile::{QuickToggle, Span};
use crate::components::quick_settings::{Page, open};
use crate::components::slider::Slider;
use crate::services::dark_style;
use crate::services::night_light::{self, NightLight};
use crate::tokens::sys;

pub(crate) fn brightness_cell() -> Option<GridCell> {
    let brightness = Brightness::read();
    if !brightness.present() {
        return None;
    }
    let percent = brightness.percent();
    drop(brightness);

    Some(super::sound::row("amane.qs.brightness", false, |width| {
        Slider::new(percent, width)
            .large(ICON_BRIGHTNESS)
            .on_value_change(Brightness::set)
    }))
}

pub(crate) fn night_light_cell() -> Option<GridCell> {
    let light = NightLight::read();
    if !light.present() {
        return None;
    }
    let on = light.on();
    let subtitle = if on {
        format!("{} K", light.temperature())
    } else {
        String::from("Off")
    };
    drop(light);

    let tile = QuickToggle::new("amane.qs.night-light", ICON_NIGHT_LIGHT, "Night Light")
        .subtitle(subtitle)
        .checked(on)
        .on_toggle(NightLight::set)
        .on_open(|| open(Page::NightLight))
        .build();
    Some(Cell::Half(boxed(tile)))
}

pub(crate) fn night_light_page() -> DetailPage {
    let light = NightLight::read();
    let on = light.on();
    let kelvin = light.temperature();
    drop(light);

    let slider = Slider::new(night_light::slider_value(kelvin), Span::Full.width())
        .large(ICON_NIGHT_LIGHT)
        .dimmed(!on)
        .on_value_change(|value| NightLight::set_temperature(night_light::slider_kelvin(value)))
        .build();

    let height = sys::size::row::height() + sys::space::gap::l() + sys::size::control::height();
    let body = vec![
        boxed(caption(format!(
            "Warmer to the right · {kelvin} K{}",
            if on { "" } else { " when on" }
        ))),
        boxed(slider),
    ];

    DetailPage::new("amane.qs.night-light.page", ICON_NIGHT_LIGHT, "Night Light")
        .subtitle(on_off(on))
        .switch(on, NightLight::set)
        .body(body, height)
}

pub(crate) fn dark_style_cell() -> Option<GridCell> {
    if !dark_style::present() {
        return None;
    }
    let dark = dark_style::dark();

    let tile = QuickToggle::new("amane.qs.dark-style", ICON_DARK_MODE, "Dark Style")
        .subtitle(on_off(dark))
        .checked(dark)
        .on_toggle(dark_style::set)
        .build();
    Some(Cell::Half(boxed(tile)))
}
