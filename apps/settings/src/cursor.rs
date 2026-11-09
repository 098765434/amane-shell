//! The pointer's look, on Appearance: the cursor theme and size, and when
//! it hides. niri's `cursor` section draws it and hands the theme and size
//! to the apps it starts; GTK apps read them from GNOME's settings, which
//! are set too, so they follow at once.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::OnceLock;

use emaki_kit::dropdown::Dropdown;
use emaki_kit::form::{FormRow, Group};

use crate::group;
use crate::niri_settings::{self, Key, Shown};

const THEME: Key = ("cursor", "xcursor-theme");
const SIZE: Key = ("cursor", "xcursor-size");
const HIDE_TYPING: Key = ("cursor", "hide-when-typing");
const HIDE_AFTER: Key = ("cursor", "hide-after-inactive-ms");

const SIZES: [u8; 4] = [24, 32, 48, 64];

const HIDE_AFTERS: [(&str, Option<u32>); 4] = [
    ("Never", None),
    ("5 seconds", Some(5_000)),
    ("10 seconds", Some(10_000)),
    ("30 seconds", Some(30_000)),
];

/// The cursor themes installed: icon themes with a `cursors` folder.
fn themes() -> &'static [String] {
    static FOUND: OnceLock<Vec<String>> = OnceLock::new();
    FOUND.get_or_init(|| {
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            dirs.push(home.join(".icons"));
        }
        dirs.push(emaki_kit::paths::data().join("icons"));
        let system = std::env::var("XDG_DATA_DIRS")
            .ok()
            .filter(|dirs| !dirs.is_empty())
            .unwrap_or_else(|| String::from("/usr/local/share:/usr/share"));
        dirs.extend(std::env::split_paths(&system).map(|dir| dir.join("icons")));
        let mut found = BTreeSet::new();
        for dir in dirs {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                if entry.path().join("cursors").is_dir() {
                    found.insert(entry.file_name().to_string_lossy().into_owned());
                }
            }
        }
        found.into_iter().collect()
    })
}

/// Tells GNOME's settings too, which GTK apps read the cursor from.
fn tell_gtk(key: &str, value: String) {
    emaki_kit::process::run(
        "gsettings",
        vec![
            String::from("set"),
            String::from("org.gnome.desktop.interface"),
            key.to_string(),
            value,
        ],
    );
}

pub fn group_for(shown: &Shown) -> Group {
    let editable = shown.editable;
    let now_theme = shown.word(THEME, "default");
    let mut names: Vec<String> = themes().to_vec();
    if !names.contains(&now_theme) {
        names.insert(0, now_theme.clone());
    }
    let at = names
        .iter()
        .position(|name| *name == now_theme)
        .unwrap_or(0);
    let picked = names.clone();
    let theme_menu = Dropdown::new("settings.appearance.cursor.theme.menu", names, at)
        .on_select(move |index| {
            if let Some(name) = picked.get(index) {
                niri_settings::set("cursor theme", THEME, crate::niri_files::quote(name));
                tell_gtk("cursor-theme", name.clone());
            }
        })
        .width(200.0)
        .disabled(!editable)
        .build();

    let size: u8 = shown.number(SIZE, 24);
    let size_at = SIZES
        .iter()
        .position(|s| *s >= size)
        .unwrap_or(SIZES.len() - 1);
    let size_menu = Dropdown::new(
        "settings.appearance.cursor.size.menu",
        SIZES.iter().map(|s| format!("{s} px")).collect(),
        size_at,
    )
    .on_select(|index| {
        let size = SIZES[index.min(SIZES.len() - 1)];
        niri_settings::set("cursor size", SIZE, size.to_string());
        tell_gtk("cursor-size", size.to_string());
    })
    .width(200.0)
    .disabled(!editable)
    .build();

    let hide_after: Option<u32> = shown.get(HIDE_AFTER).and_then(|ms| ms.parse().ok());
    let hide_at = HIDE_AFTERS
        .iter()
        .position(|(_, ms)| *ms == hide_after)
        .unwrap_or(0);
    let hide_menu = Dropdown::new(
        "settings.appearance.cursor.hide-after.menu",
        HIDE_AFTERS
            .iter()
            .map(|(label, _)| label.to_string())
            .collect(),
        hide_at,
    )
    .on_select(
        |index| match HIDE_AFTERS.get(index).and_then(|(_, ms)| *ms) {
            Some(ms) => niri_settings::set("cursor hide after", HIDE_AFTER, ms.to_string()),
            None => niri_settings::change("cursor hide after", std::time::Duration::ZERO, |ours| {
                ours.remove(HIDE_AFTER)
            }),
        },
    )
    .width(200.0)
    .disabled(!editable)
    .build();

    group()
        .header("Pointer")
        .row(FormRow::new("settings.appearance.cursor.theme", "Cursor").trailing(theme_menu))
        .row(FormRow::new("settings.appearance.cursor.size", "Size").trailing(size_menu))
        .row(niri_settings::switch(
            shown,
            "settings.appearance.cursor.hide-typing",
            "Hide while typing",
            "cursor hide while typing",
            HIDE_TYPING,
        ))
        .row(
            FormRow::new(
                "settings.appearance.cursor.hide-after",
                "Hide when still for",
            )
            .trailing(hide_menu),
        )
        .footer("Apps already open take a new cursor as they start again.")
}

/// The Pointer group, with what stands in the way of changing it.
pub fn groups() -> Vec<Group> {
    let shown = niri_settings::shown();
    let mut list = niri_settings::notices("settings.appearance.cursor");
    list.push(group_for(&shown));
    list
}
