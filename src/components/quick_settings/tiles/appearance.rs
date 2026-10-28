//! The screen — Control Center's Display card (GNOME's brightness slider,
//! only with a backlight), and the Night Light (with its page: how warm)
//! and Dark Style round buttons.

use amane::{Brightness, Service, Widget};

use super::super::grid::{Metrics, Span};
use super::super::slider_card::{self, SliderRow};
use super::super::{Page, open};
use super::on_off;
use crate::components::icons::{
    ICON_BRIGHTNESS_HIGH, ICON_BRIGHTNESS_LOW, ICON_DARK_MODE, ICON_NIGHT_LIGHT,
};
use crate::components::layout::{boxed, caption};
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::tile::QuickToggle;
use crate::components::slider::Slider;
use crate::services::dark_style;
use crate::services::night_light::{self, NightLight, Schedule};

pub(crate) fn brightness_cell(span: Span, inert: bool) -> Option<Box<dyn Widget>> {
    let brightness = Brightness::read();
    if !brightness.present() {
        return None;
    }
    let percent = brightness.percent();
    drop(brightness);

    Some(
        slider_card::card(
            SliderRow::new(
                "Display",
                ICON_BRIGHTNESS_LOW,
                ICON_BRIGHTNESS_HIGH,
                move |width| {
                    Slider::new(percent, width)
                        .inert(inert)
                        .on_value_change(Brightness::set)
                },
            ),
            span,
        )
        .into(),
    )
}

pub(crate) fn night_light_toggle() -> Option<QuickToggle> {
    let light = NightLight::read();
    if !light.present() {
        return None;
    }
    let on = light.on();
    let subtitle = match (on, light.schedule()) {
        (false, _) => String::from("Off"),
        (true, Schedule::Manual) => format!("{} K", light.temperature()),
        (true, _) => String::from("Scheduled"),
    };
    drop(light);

    Some(
        QuickToggle::new("amane.qs.night-light", ICON_NIGHT_LIGHT, "Night Light")
            .subtitle(subtitle)
            .checked(on)
            .on_toggle(NightLight::set)
            .on_open(|| open(Page::NightLight)),
    )
}

pub(crate) fn night_light_page() -> DetailPage {
    let light = NightLight::read();
    let on = light.on();
    let kelvin = light.temperature();
    drop(light);

    DetailPage::new("amane.qs.night-light.page", "Night Light")
        .subtitle(if on {
            format!("{kelvin} K")
        } else {
            String::from("Off")
        })
        .switch(on, NightLight::set)
        .body(move |_| {
            let slider = Slider::new(
                night_light::slider_value(kelvin),
                Metrics::tokens().width(Span::FULL),
            )
            .large(ICON_NIGHT_LIGHT)
            .dimmed(!on)
            .on_value_change(|value| NightLight::set_temperature(night_light::slider_kelvin(value)))
            .build();
            vec![
                boxed(caption(format!(
                    "Warmer to the right · {kelvin} K{}",
                    if on { "" } else { " when on" }
                ))),
                boxed(slider),
            ]
        })
}

pub(crate) fn dark_style_toggle() -> Option<QuickToggle> {
    if !dark_style::present() {
        return None;
    }
    let dark = dark_style::dark();

    Some(
        QuickToggle::new("amane.qs.dark-style", ICON_DARK_MODE, "Dark Style")
            .subtitle(on_off(dark))
            .checked(dark)
            .on_toggle(dark_style::set),
    )
}
