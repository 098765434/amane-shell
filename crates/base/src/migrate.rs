//! The one-time move from the names before emaki: the shell was `amane-shell`
//! and its programs `amane-*` until 2026-11, so their folders, and the ids
//! their settings keep, still carry those names on a machine that ran them.
//!
//! [`from_amane`] runs first thing in every program's `main`. Each folder
//! moves to its new name: renamed whole when the new one isn't there yet,
//! else what the new one doesn't have moves in and the rest stays put. Then
//! the JSON under the new config and state folders has its ids renamed
//! (`org.amane.drop` → `org.emaki.drop`, `amane-settings.desktop` →
//! `emaki-settings.desktop`), and a marker,
//! `$XDG_CONFIG_HOME/emaki/.migrated-from-amane`, makes every later run a
//! single look. Nothing here panics: what can't move is logged and left.
//!
//! `scripts/migrate-from-amane` does the same moves by hand, and the niri
//! config's part too.

use std::fs;
use std::path::{Path, PathBuf};

use crate::app_ids;

/// Each program's old folder name and its new one, under every base folder.
const PROGRAMS: [(&str, &str); 9] = [
    ("amane-shell", app_ids::SHELL),
    ("amane-assistant", app_ids::ASSISTANT),
    ("amane-pinentry", app_ids::PINENTRY),
    ("amane-train-watch", app_ids::TRAIN_WATCH),
    ("amane-settings", app_ids::SETTINGS),
    ("amane-messages", app_ids::MESSAGES),
    ("amane-mirror", app_ids::MIRROR),
    ("amane-shader-lab", app_ids::SHADER_LAB),
    ("amane-drop", app_ids::DROP),
];

/// The file that says the move is done, in the shell's new config folder.
const MARKER: &str = ".migrated-from-amane";

/// A JSON file larger than this is left as it is: settings and state are a
/// few kilobytes.
const JSON_LIMIT: u64 = 1024 * 1024;

/// How deep below a config or state folder JSON is looked for.
const JSON_DEPTH: usize = 4;

/// Old ids in JSON and their new spelling: the status bar's items
/// (`org.amane.drop`, `app:org.amane.weather`), and the desktop ids the
/// dock's pins and the launcher's history keep (`amane-settings.desktop`,
/// `app:amane-mirror.desktop`, `amane-mirror.<package>`).
const IDS: [(&str, &str); 6] = [
    ("org.amane.", "org.emaki."),
    ("amane-settings", "emaki-settings"),
    ("amane-messages", "emaki-messages"),
    ("amane-mirror", "emaki-mirror"),
    ("amane-shader-lab", "emaki-shader-lab"),
    ("amane-drop", "emaki-drop"),
];

/// What an id is found after in the JSON: the start of a string, or a
/// prefix's colon (`app:`).
const BEFORE: [char; 2] = ['"', ':'];

/// The XDG base folders the move works in.
#[derive(Clone, Debug)]
struct Bases {
    config: PathBuf,
    state: PathBuf,
    cache: PathBuf,
    data: PathBuf,
}

/// Moves the folders and ids from before the rename to emaki, once.
pub fn from_amane() {
    // until the rename lands every pair is the same name: nothing to do
    if PROGRAMS.iter().all(|(old, new)| old == new) {
        return;
    }
    let bases = Bases {
        config: crate::paths::config(),
        state: crate::paths::state(),
        cache: crate::paths::cache(),
        data: crate::paths::data(),
    };
    run(&bases, &PROGRAMS);
}

/// The name `id` was known by before the rename, when it had another one:
/// what a keyring entry kept under the old name is found by.
pub fn old_id(id: &str) -> Option<&'static str> {
    PROGRAMS
        .iter()
        .find(|(old, new)| *new == id && old != new)
        .map(|(old, _)| *old)
}

/// The move under `bases`, with `programs` as its (old, new) folder names, the
/// shell's first. Returns whether it ran (no marker yet).
fn run(bases: &Bases, programs: &[(&str, &str)]) -> bool {
    let Some(&(_, shell)) = programs.first() else {
        return false;
    };
    let roots = [&bases.config, &bases.state, &bases.cache, &bases.data];
    // no home to find the folders under: nothing to move, nowhere to mark
    if roots.iter().any(|root| !root.is_absolute()) {
        return false;
    }
    let marker = bases.config.join(shell).join(MARKER);
    if marker.exists() {
        return false;
    }

    for root in roots {
        for (old, new) in programs {
            move_folder(&root.join(old), &root.join(new));
        }
    }
    // the phone's message pictures were under the library's cache folder
    move_folder(
        &bases.cache.join("amane").join("messages"),
        &bases.cache.join(shell).join("phone").join("messages"),
    );

    for root in [&bases.config, &bases.state] {
        for (_, new) in programs {
            rename_ids(&root.join(new), JSON_DEPTH);
        }
    }

    let written = fs::create_dir_all(marker.parent().unwrap_or(&bases.config)).and_then(|()| {
        fs::write(
            &marker,
            "the folders and ids from before the rename to emaki were moved\n",
        )
    });
    if let Err(error) = written {
        log::warn!("migrate: couldn't write {}: {error}", marker.display());
    }
    true
}

/// `old` moved to `new`: renamed whole when `new` isn't there, else each of
/// its entries `new` doesn't have, the rest left in `old`.
fn move_folder(old: &Path, new: &Path) {
    if old == new || !old.is_dir() {
        return;
    }
    if fs::symlink_metadata(new).is_err() {
        if let Some(parent) = new.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            log::warn!("migrate: couldn't make {}: {error}", parent.display());
            return;
        }
        match fs::rename(old, new) {
            Ok(()) => log::info!("migrate: moved {} to {}", old.display(), new.display()),
            Err(error) => log::warn!(
                "migrate: couldn't move {} to {}: {error}",
                old.display(),
                new.display()
            ),
        }
        return;
    }

    let Ok(entries) = fs::read_dir(old) else {
        return;
    };
    let (mut moved, mut kept) = (0, 0);
    for entry in entries.flatten() {
        let to = new.join(entry.file_name());
        if fs::symlink_metadata(&to).is_ok() || fs::rename(entry.path(), &to).is_err() {
            kept += 1;
        } else {
            moved += 1;
        }
    }
    if moved > 0 {
        log::info!(
            "migrate: moved {moved} entries of {} into {}",
            old.display(),
            new.display()
        );
    }
    if kept > 0 {
        log::warn!(
            "migrate: left {kept} entries in {}: {} has them too",
            old.display(),
            new.display()
        );
    } else {
        // empty now; one that isn't stays
        let _ = fs::remove_dir(old);
    }
}

/// The old ids renamed in every small JSON file under `folder`, `depth`
/// folders down.
fn rename_ids(folder: &Path, depth: usize) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() && depth > 0 {
            rename_ids(&path, depth - 1);
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            rename_ids_in(&path);
        }
    }
}

fn rename_ids_in(file: &Path) {
    let Ok(meta) = fs::metadata(file) else {
        return;
    };
    if meta.len() > JSON_LIMIT {
        return;
    }
    let Ok(text) = fs::read_to_string(file) else {
        return;
    };
    let renamed = renamed_ids(&text);
    if renamed == text {
        return;
    }
    // written whole, with the file's own permissions (secrets.json is 0600)
    let written = crate::fs::write_whole(file, &renamed)
        .and_then(|()| fs::set_permissions(file, meta.permissions()));
    match written {
        Ok(()) => log::info!("migrate: renamed the ids in {}", file.display()),
        Err(error) => log::warn!("migrate: couldn't rewrite {}: {error}", file.display()),
    }
}

/// `text` with every old id after a quote or a colon spelled the new way.
fn renamed_ids(text: &str) -> String {
    let mut text = text.to_string();
    for (old, new) in IDS {
        for before in BEFORE {
            text = text.replace(&format!("{before}{old}"), &format!("{before}{new}"));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAMES: [(&str, &str); 3] = [
        ("amane-shell", "emaki"),
        ("amane-drop", "emaki-drop"),
        ("amane-assistant", "emaki-assistant"),
    ];

    /// A home of its own, its XDG folders under it.
    fn home(name: &str) -> (PathBuf, Bases) {
        let home =
            std::env::temp_dir().join(format!("emaki-migrate-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let bases = Bases {
            config: home.join(".config"),
            state: home.join(".local/state"),
            cache: home.join(".cache"),
            data: home.join(".local/share"),
        };
        (home, bases)
    }

    fn write(file: &Path, text: &str) {
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    fn read(file: &Path) -> String {
        fs::read_to_string(file).unwrap()
    }

    #[test]
    fn old_folders_move_and_their_ids_are_renamed() {
        let (home, bases) = home("move");
        write(
            &bases.config.join("amane-shell/status-bar.json"),
            r#"{"items":["clock","org.amane.drop","app:org.amane.weather"]}"#,
        );
        write(
            &bases.state.join("amane-shell/dock-pins/pins.json"),
            r#"["amane-settings.desktop","firefox.desktop"]"#,
        );
        write(
            &bases.state.join("amane-shell/launcher-usage.json"),
            r#"{"app:amane-mirror.desktop":3,"text":"the amane-drop folder"}"#,
        );
        write(&bases.state.join("amane-drop/shared.json"), "[]");
        write(
            &bases.data.join("amane-assistant/models/needle3.cact"),
            "model",
        );
        write(&bases.cache.join("amane/messages/photos/a.png"), "png");
        write(&bases.cache.join("amane/library/kept"), "the library's");

        assert!(run(&bases, &NAMES));

        assert_eq!(
            read(&bases.config.join("emaki/status-bar.json")),
            r#"{"items":["clock","org.emaki.drop","app:org.emaki.weather"]}"#
        );
        assert_eq!(
            read(&bases.state.join("emaki/dock-pins/pins.json")),
            r#"["emaki-settings.desktop","firefox.desktop"]"#
        );
        // an id inside words isn't one
        assert_eq!(
            read(&bases.state.join("emaki/launcher-usage.json")),
            r#"{"app:emaki-mirror.desktop":3,"text":"the amane-drop folder"}"#
        );
        assert!(bases.state.join("emaki-drop/shared.json").exists());
        assert!(
            bases
                .data
                .join("emaki-assistant/models/needle3.cact")
                .exists()
        );
        assert!(
            bases
                .cache
                .join("emaki/phone/messages/photos/a.png")
                .exists()
        );
        assert!(!bases.config.join("amane-shell").exists());
        assert!(!bases.cache.join("amane/messages").exists());
        // the library's own folder stays
        assert!(bases.cache.join("amane/library/kept").exists());
        assert!(bases.config.join("emaki").join(MARKER).exists());

        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn both_folders_keep_what_collides_and_the_marker_stops_a_second_run() {
        let (home, bases) = home("both");
        write(&bases.config.join("amane-shell/dock.json"), "old dock");
        write(&bases.config.join("amane-shell/appearance.json"), "{}");
        write(&bases.config.join("emaki/dock.json"), "new dock");

        assert!(run(&bases, &NAMES));
        assert_eq!(read(&bases.config.join("emaki/dock.json")), "new dock");
        assert!(bases.config.join("emaki/appearance.json").exists());
        assert_eq!(
            read(&bases.config.join("amane-shell/dock.json")),
            "old dock"
        );
        assert!(!bases.config.join("amane-shell/appearance.json").exists());

        // marked: a later old folder is left alone
        write(&bases.state.join("amane-shell/late.json"), "{}");
        assert!(!run(&bases, &NAMES));
        assert!(bases.state.join("amane-shell/late.json").exists());
        assert!(!bases.state.join("emaki").exists());

        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn nothing_to_move_only_marks() {
        let (home, bases) = home("fresh");
        assert!(run(&bases, &NAMES));
        assert!(bases.config.join("emaki").join(MARKER).exists());
        assert!(!bases.state.exists());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn a_relative_base_does_nothing() {
        let mut bases = home("relative").1;
        bases.state = PathBuf::from("relative/state");
        assert!(!run(&bases, &NAMES));
        assert!(!bases.config.exists());
    }
}
