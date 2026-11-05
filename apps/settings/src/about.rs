//! What's running, and where its settings are kept.

use amane::{Column, Config};
use amane_kit::form::FormRow;
use amane_kit::settings::{Appearance, Desktop, Dock, Launcher, StatusBar};

use crate::{group, groups};

pub fn page() -> Column {
    let files = [
        ("Appearance", Config::<Appearance>::path()),
        ("Desktop", Config::<Desktop>::path()),
        ("Status Bar", Config::<StatusBar>::path()),
        ("Dock", Config::<Dock>::path()),
        ("Spotlight", Config::<Launcher>::path()),
    ]
    .map(|(name, path)| {
        FormRow::new(format!("settings.about.file.{name}"), name)
            .subtitle(path.display().to_string())
    });

    groups(vec![
        group().row(
            FormRow::new("settings.about.version", "Shell Settings")
                .value(format!("version {}", env!("CARGO_PKG_VERSION"))),
        ),
        group().header("Settings files").rows(files).footer(
            "AMANE_THEME, AMANE_DENSITY and AMANE_TEXT_SCALE, when set, win over these settings.",
        ),
    ])
}
