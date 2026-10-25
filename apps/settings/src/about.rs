//! What's running, and where its settings are kept.

use amane::{Column, Config, Text};
use amane_kit::layout::boxed;
use amane_kit::settings::Appearance;
use amane_kit::tokens::{Styled, sys};

use crate::setting;

pub fn page() -> Column {
    let line = |text: String| {
        Text::new(text)
            .style(sys::r#type::label())
            .color(sys::color::surface::popover::fg())
    };

    Column::new(vec![
        boxed(setting(
            "Shell Settings",
            line(format!("version {}", env!("CARGO_PKG_VERSION"))),
        )),
        boxed(setting(
            "Settings file",
            line(Config::<Appearance>::path().display().to_string()),
        )),
        boxed(setting(
            "Overrides",
            line(String::from(
                "AMANE_THEME, AMANE_DENSITY and AMANE_TEXT_SCALE, when set, win over these settings",
            )),
        )),
    ])
    .gap(sys::space::gap::l())
}
