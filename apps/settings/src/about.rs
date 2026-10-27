//! What's running, and where its settings are kept.

use amane::{Column, Config, Text};
use amane_kit::layout::boxed;
use amane_kit::settings::{Appearance, Desktop, Dock, Launcher, StatusBar};
use amane_kit::tokens::{Styled, sys};

use crate::setting;
use amane_kit::material::{self, Level};

pub fn page() -> Column {
    let line = |text: String| {
        Text::new(text)
            .style(sys::r#type::label())
            .color(material::label(Level::Primary))
    };

    let files = [
        Config::<Appearance>::path(),
        Config::<Desktop>::path(),
        Config::<StatusBar>::path(),
        Config::<Dock>::path(),
        Config::<Launcher>::path(),
    ]
    .map(|path| path.display().to_string())
    .join("\n");

    Column::new(vec![
        boxed(setting(
            "Shell Settings",
            line(format!("version {}", env!("CARGO_PKG_VERSION"))),
        )),
        boxed(setting("Settings files", line(files))),
        boxed(setting(
            "Overrides",
            line(String::from(
                "AMANE_THEME, AMANE_DENSITY and AMANE_TEXT_SCALE, when set, win over these settings",
            )),
        )),
    ])
    .gap(sys::space::gap::l())
}
