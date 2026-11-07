//! What's running, and where its settings are kept.

use std::path::Path;

use amane::{Center, Color, Column, Config, Rectangle, Row, Size, Text, Widget};
use amane_kit::buttons::PushButton;
use amane_kit::form::FormRow;
use amane_kit::icons::ICON_TUNE;
use amane_kit::layout::{boxed, icon};
use amane_kit::settings::{Appearance, Desktop, Dock, Launcher, StatusBar};
use amane_kit::style;
use amane_kit::tokens::Styled;

use crate::{View, group, groups};

pub fn page() -> Column {
    let count = files().len();
    groups(vec![
        group().custom(hero(), HERO),
        group().row(
            FormRow::new("settings.about.files-link", "Settings files")
                .value(format!("{count} files"))
                .on_press(|| crate::open(View::Sub("files"))),
        ),
    ])
}

/// How tall the app's card is.
const HERO: f32 = 112.0;

/// The app's icon, name and version.
fn hero() -> Rectangle {
    let tile = Rectangle::new()
        .width(Size::Fixed(64.0))
        .height(Size::Fixed(64.0))
        .radius(15.0)
        .fill(Color::from("#8e8e93"))
        .align_child(Center, Center)
        .child(icon(ICON_TUNE, 38.0, Color::WHITE));
    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(HERO))
        .fill(Color::TRANSPARENT)
        .padding(amane::Padding {
            left: style::row_pad_h(),
            ..amane::Padding::default()
        })
        .align_child(amane::Start, Center)
        .child(
            Row::new(vec![
                boxed(tile),
                boxed(
                    Column::new(vec![
                        boxed(
                            Text::new("Shell Settings")
                                .style(style::page_title_font())
                                .color(style::label_primary()),
                        ),
                        boxed(
                            Text::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                                .style(style::subtitle_font())
                                .color(style::label_secondary()),
                        ),
                    ])
                    .gap(2.0),
                ),
            ])
            .gap(16.0)
            .align(Center),
        )
}

fn files() -> [(&'static str, std::path::PathBuf); 5] {
    [
        ("Appearance", Config::<Appearance>::path()),
        ("Desktop", Config::<Desktop>::path()),
        ("Status Bar", Config::<StatusBar>::path()),
        ("Dock", Config::<Dock>::path()),
        ("Launcher", Config::<Launcher>::path()),
    ]
}

/// `path` with the home folder as `~`.
fn tidy(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// `text` cut in the middle with an ellipsis until `fits` says it fits,
/// so a path keeps its start and its file name.
fn middle_elide(text: &str, fits: impl Fn(&str) -> bool) -> String {
    if fits(text) {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut keep = chars.len().saturating_sub(1);
    while keep > 2 {
        let head = keep / 3;
        let tail = keep - head;
        let shortened: String = chars[..head]
            .iter()
            .chain(std::iter::once(&'…'))
            .chain(&chars[chars.len() - tail..])
            .collect();
        if fits(&shortened) {
            return shortened;
        }
        keep -= 1;
    }
    String::from("…")
}

/// The page's sub-pages: the settings files.
pub fn sub(name: &str) -> Option<(String, Box<dyn Widget>)> {
    if name != "files" {
        return None;
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    // the room beside the button for the path
    let room = crate::page_width() - 2.0 * style::row_pad_h() - 150.0;
    let fits = |text: &str| amane_kit::layout::text_width(text, style::subtitle_font()) <= room;
    let rows = files().map(|(name, path)| {
        let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
        FormRow::new(format!("settings.about.file.{name}"), name)
            .subtitle(middle_elide(&tidy(&path, home.as_deref()), fits))
            .trailing(
                PushButton::new(format!("settings.about.show.{name}"), "Show in Folder")
                    .on_press(move || {
                        let folder = folder.clone();
                        std::thread::spawn(move || {
                            if let Err(failure) =
                                amane_process::Cmd::new("xdg-open").arg(&folder).status()
                            {
                                log::warn!("couldn't show {}: {failure}", folder.display());
                            }
                        });
                    })
                    .build(),
            )
    });
    Some((
        String::from("Settings Files"),
        boxed(groups(vec![group().rows(rows).footer(
            "Editing one by hand works the same as changing it here. AMANE_THEME, \
             AMANE_DENSITY and AMANE_TEXT_SCALE, when set, win over these settings.",
        )])),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_path_keeps_its_ends() {
        let short = middle_elide("/a/b/c/dock.json", |text| text.chars().count() <= 10);
        assert_eq!(short.chars().count(), 10);
        assert!(short.ends_with(".json") && short.starts_with("/a/") && short.contains('…'));
        assert_eq!(middle_elide("dock.json", |_| true), "dock.json");
    }

    #[test]
    fn home_reads_as_a_tilde() {
        let home = Path::new("/home/me");
        assert_eq!(
            tidy(
                Path::new("/home/me/.config/amane-shell/dock.json"),
                Some(home)
            ),
            "~/.config/amane-shell/dock.json"
        );
        assert_eq!(tidy(Path::new("/etc/x"), Some(home)), "/etc/x");
    }
}
