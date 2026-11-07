//! App icon — an app's icon from its desktop entry, by a window's app_id.
//!
//! The dock and the notification cards draw apps this way: the icon its
//! desktop entry names, else a theme icon named after the app_id, else a
//! first-letter placeholder on a raised tile while nothing resolves (or the
//! image is still decoding). The launcher draws installed apps from their
//! entries the same way ([`entry_tile`]).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use amane::{Apps, Center, DesktopApp, Image, Rectangle, Service, Size, Text};

use crate::material::{self, FillLevel, Level};
use crate::tokens::{Styled, sys};

/// A window's app icon, or a first-letter placeholder when none resolves.
pub fn tile(app: &str, title: &str, side: f32) -> Rectangle {
    drawn(icon_for(app), first_letter(app, title), side)
}

/// A desktop entry's icon, or the first letter of its name, for a list of
/// installed apps rather than open windows.
pub fn entry_tile(app: &DesktopApp, side: f32) -> Rectangle {
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
        .fill(material::fill_role(FillLevel::Secondary))
        .align_child(Center, Center)
        .child(
            Text::new(letter)
                .style(sys::r#type::title())
                .color(material::label(Level::Primary))
                .tight(),
        )
}

/// Resolve a niri window app_id to a desktop-entry icon file, matching the
/// id (and its last dotted segment) against each app's icon name, exec
/// basename, or name — case-insensitively — else to a theme icon named
/// after either.
pub fn icon_for(app_id: &str) -> Option<PathBuf> {
    if app_id.is_empty() {
        return None;
    }

    if let Some(path) = with_entry(app_id, |app| app.icon_path().map(Path::to_path_buf)).flatten() {
        return Some(path);
    }
    // an app with no desktop entry may still have a theme icon by its id, or its last part
    let needle = app_id.to_lowercase();
    let last = needle.rsplit('.').next().unwrap_or(&needle);
    Apps::find_icon(&needle).or_else(|| (last != needle).then(|| Apps::find_icon(last)).flatten())
}

/// The name an app with this app_id goes by: its desktop entry's, else the
/// id's last dotted segment.
pub fn name_for(app_id: &str) -> Option<String> {
    if let Some(name) = with_entry(app_id, |app| app.name().to_string()) {
        return Some(name);
    }
    app_id
        .rsplit('.')
        .next()
        .filter(|last| !last.is_empty())
        .map(String::from)
}

/// The desktop entry a window's app_id belongs to, matched as [`icon_for`]
/// says.
pub fn entry_for(app_id: &str) -> Option<DesktopApp> {
    with_entry(app_id, DesktopApp::clone)
}

/// Runs `f` on the desktop entry a window's app_id belongs to, without copying it: the dock
/// and notifications ask for every icon on every frame.
pub fn with_entry<R>(app_id: &str, f: impl FnOnce(&DesktopApp) -> R) -> Option<R> {
    if app_id.is_empty() {
        return None;
    }

    let apps = Apps::read();
    let needle = app_id.to_lowercase();
    let last = needle.rsplit('.').next().unwrap_or(&needle);

    // the first app any of whose names is the app_id, or its last part
    let index = INDEX.with_borrow_mut(|index| {
        let list = apps.list();
        let made_for = (list.as_ptr() as usize, list.len());
        if index.as_ref().is_none_or(|(made, _)| *made != made_for) {
            *index = Some((made_for, names(list)));
        }
        let (_, names) = index.as_ref()?;
        match (names.get(&needle), names.get(last)) {
            (Some(a), Some(b)) => Some(*a.min(b)),
            (a, b) => a.or(b).copied(),
        }
    })?;

    apps.list().get(index).map(f)
}

// the list an index was made from (its address and length), and each name to
// the position of the first app in it that has the name
type NameIndex = ((usize, usize), HashMap<String, usize>);

thread_local! {
    /*
     * each app's names (icon, name, executable) lowercased, to the first
     * app in the list that has it, for the list it was made from: the dock
     * and the notifications look an app up for every icon on every frame
     */
    static INDEX: RefCell<Option<NameIndex>> =
        const { RefCell::new(None) };
}

fn names(apps: &[DesktopApp]) -> HashMap<String, usize> {
    let mut names = HashMap::new();
    for (index, app) in apps.iter().enumerate() {
        let exec_base = app
            .exec()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_lowercase();
        for name in [
            app.icon().unwrap_or("").to_lowercase(),
            app.name().to_lowercase(),
            exec_base,
        ] {
            names.entry(name).or_insert(index);
        }
    }
    names
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
