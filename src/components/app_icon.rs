//! App icon — an app's icon from its desktop entry, by a window's app_id.
//!
//! The dock and the notification cards draw apps this way: the icon its
//! desktop entry names, else a theme icon named after the app_id, else a
//! first-letter placeholder on a raised tile while nothing resolves (or the
//! image is still decoding). The launcher draws installed apps from their
//! entries the same way ([`entry_tile`]).

use std::path::PathBuf;

use amane::{Apps, Center, DesktopApp, Image, Rectangle, Service, Size, Text};

use crate::tokens::{Styled, sys};

/// A window's app icon, or a first-letter placeholder when none resolves.
pub(crate) fn tile(app: &str, title: &str, side: f32) -> Rectangle {
    drawn(icon_for(app), first_letter(app, title), side)
}

/// A desktop entry's icon, or the first letter of its name, for a list of
/// installed apps rather than open windows.
pub(crate) fn entry_tile(app: &DesktopApp, side: f32) -> Rectangle {
    let path = app.icon_path().map(|path| path.to_path_buf());
    drawn(path, first_letter("", app.name()), side)
}

fn drawn(path: Option<PathBuf>, letter: String, side: f32) -> Rectangle {
    if let Some(path) = path {
        // `loaded` kicks off the async decode and wakes us when ready; until
        // then fall through to the placeholder for this frame
        if Image::loaded(&path) {
            return Rectangle::new()
                .width(Size::Fixed(side))
                .height(Size::Fixed(side))
                .fill(Image::contain(path));
        }
    }

    Rectangle::new()
        .width(Size::Fixed(side))
        .height(Size::Fixed(side))
        .radius(sys::radius::raised())
        .fill(sys::color::surface::raised::bg())
        .align_child(Center, Center)
        .child(
            Text::new(letter)
                .style(sys::r#type::title())
                .color(sys::color::surface::panel::fg())
                .tight(),
        )
}

/// Resolve a niri window app_id to a desktop-entry icon file, matching the
/// id (and its last dotted segment) against each app's icon name, exec
/// basename, or name — case-insensitively — else to a theme icon named
/// after either.
pub(crate) fn icon_for(app_id: &str) -> Option<PathBuf> {
    if app_id.is_empty() {
        return None;
    }

    let needle = app_id.to_lowercase();
    let last = needle.rsplit('.').next().unwrap_or(&needle).to_string();

    entry_for(app_id)
        .and_then(|app| app.icon_path().map(|path| path.to_path_buf()))
        // an app with no desktop entry may still have a theme icon by its id
        .or_else(|| Apps::find_icon(&needle))
        .or_else(|| Apps::find_icon(&last))
}

/// The name an app with this app_id goes by: its desktop entry's, else the
/// id's last dotted segment.
pub(crate) fn name_for(app_id: &str) -> Option<String> {
    if let Some(app) = entry_for(app_id) {
        return Some(app.name().to_string());
    }
    app_id
        .rsplit('.')
        .next()
        .filter(|last| !last.is_empty())
        .map(String::from)
}

/// The desktop entry a window's app_id belongs to, matched as [`icon_for`]
/// says.
fn entry_for(app_id: &str) -> Option<DesktopApp> {
    if app_id.is_empty() {
        return None;
    }

    let apps = Apps::read();
    let needle = app_id.to_lowercase();
    let last = needle.rsplit('.').next().unwrap_or(&needle).to_string();

    let matched = apps.list().iter().find(|app| {
        let icon = app.icon().unwrap_or("").to_lowercase();
        let name = app.name().to_lowercase();
        let exec_base = app
            .exec()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_lowercase();

        [icon, name, exec_base]
            .iter()
            .any(|field| *field == needle || *field == last)
    });

    matched.cloned()
}

fn first_letter(app: &str, title: &str) -> String {
    let base = app
        .rsplit('.')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(app);
    let source = if base.is_empty() { title } else { base };

    source
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_placeholder_is_the_ids_last_part_capitalised() {
        assert_eq!(first_letter("org.gnome.Nautilus", "Files"), "N");
        assert_eq!(first_letter("", "files"), "F");
        assert_eq!(first_letter("", ""), "?");
    }
}
