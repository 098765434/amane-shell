//! Where niri saves screenshots, on Desktop: its `screenshot-path`, a
//! folder and niri's file name pattern, or nothing, which only copies them
//! to the clipboard.

use std::path::{Path, PathBuf};

use amane::FileDialog;
use emaki_kit::buttons::PushButton;
use emaki_kit::form::{FormRow, Group};
use emaki_kit::icons::ICON_SCREENSHOT;

use crate::group;
use crate::niri_settings::{self, Key};

const PATH: Key = ("", "screenshot-path");

/// niri's own: a file name with the date and time.
const PATTERN: &str = "Screenshot from %Y-%m-%d %H-%M-%S.png";
const DEFAULT: &str = "~/Pictures/Screenshots/Screenshot from %Y-%m-%d %H-%M-%S.png";

/// The folder a `screenshot-path` saves into, `~` for the home folder.
fn folder_of(path: &str) -> String {
    Path::new(path)
        .parent()
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `dir`, with the home folder as `~`, as niri takes it.
fn tilde(dir: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home.and_then(|home| dir.strip_prefix(&home).ok().map(Path::to_path_buf)) {
        Some(rest) if rest.as_os_str().is_empty() => String::from("~"),
        Some(rest) => format!("~/{}", rest.display()),
        None => dir.display().to_string(),
    }
}

/// The `screenshot-path` value saving into `dir` with niri's pattern.
fn path_in(dir: &Path) -> String {
    crate::niri_files::quote(&format!("{}/{PATTERN}", tilde(dir)))
}

pub fn groups() -> Vec<Group> {
    let shown = niri_settings::shown();
    let value = shown.get(PATH).unwrap_or_default();
    let clipboard_only = value == "null";
    let folder = if clipboard_only {
        String::new()
    } else {
        folder_of(&crate::niri_files::unquote(&value).unwrap_or_else(|| String::from(DEFAULT)))
    };
    let mut list = niri_settings::notices("settings.desktop.screenshots");
    let mut screenshots = group().header("Screenshots").row(
        niri_settings::switch_with(
            &shown,
            "settings.desktop.screenshots.clipboard",
            "Only copy screenshots",
            clipboard_only,
            |on| {
                if on {
                    niri_settings::set("screenshots clipboard only", PATH, "null");
                } else {
                    niri_settings::change("screenshots saved", std::time::Duration::ZERO, |ours| {
                        ours.remove(PATH)
                    });
                }
            },
        )
        .icon(ICON_SCREENSHOT, emaki_kit::color::Accent::Gray)
        .subtitle("To the clipboard, without saving a file"),
    );
    if !clipboard_only {
        let choose = PushButton::new("settings.desktop.screenshots.folder.choose", "Choose…")
            .disabled(!shown.editable)
            .on_press(|| {
                FileDialog::open("Save Screenshots In")
                    .directory(true)
                    .show(|picked| {
                        if let Some(dir) = picked.into_iter().next() {
                            niri_settings::set("screenshots folder", PATH, path_in(&dir));
                        }
                    });
            })
            .build();
        screenshots = screenshots.row(
            FormRow::new("settings.desktop.screenshots.folder", "Save in")
                .subtitle(folder)
                .trailing(choose),
        );
    }
    list.push(
        screenshots
            .footer("Print Screen and Quick Settings' Screenshot take them; niri names the files."),
    );
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_written_with_niris_pattern() {
        assert_eq!(folder_of(DEFAULT), "~/Pictures/Screenshots");
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        assert_eq!(
            path_in(&home.join("Shots")),
            "\"~/Shots/Screenshot from %Y-%m-%d %H-%M-%S.png\""
        );
        assert_eq!(
            path_in(Path::new("/srv/shots")),
            "\"/srv/shots/Screenshot from %Y-%m-%d %H-%M-%S.png\""
        );
    }
}
