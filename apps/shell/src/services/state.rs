//! The shell's state folder (`$XDG_STATE_HOME/amane-shell`) and the one way
//! its state files are kept there.
//!
//! A state file is JSON with its format's version beside the data:
//!
//! ```json
//! { "version": 1, "data": … }
//! ```
//!
//! It is written whole (through a part file and a rename), so a crash never
//! leaves half of one. A file in an older format, or under an older name, is
//! read by the type's [`State::legacy`] when there's no file in the current
//! one; the next save writes the current one beside it, and the old file is
//! left as it was, so deleting the new file reads the old one again.

use std::path::{Path, PathBuf};

use amane::serde::{Deserialize, Serialize, de::DeserializeOwned};

/// The file `name` under the shell's state folder.
pub fn state_file(name: &str) -> PathBuf {
    #[cfg(test)]
    if let Some(folder) = tests::FOLDER.with_borrow(Clone::clone) {
        return folder.join(name);
    }
    let state = amane_kit::paths::state();
    state.join("amane-shell").join(name)
}

/// Something the shell keeps in a state file of its own.
pub trait State: Serialize + DeserializeOwned {
    /// The file's name under the state folder.
    const FILE: &'static str;

    /// The format's version, raised when `Self` changes in a way an older
    /// file can't be read into.
    const VERSION: u32;

    /// What an older format or file name held, read when there's no file in
    /// this one.
    // legacy: the readers of the formats from before 2026-11; delete after 2027-01
    fn legacy() -> Option<Self> {
        None
    }
}

#[derive(Serialize)]
#[serde(crate = "amane::serde")]
struct Written<'a, T> {
    version: u32,
    data: &'a T,
}

#[derive(Deserialize)]
#[serde(crate = "amane::serde")]
struct Read {
    version: u32,
    data: serde_json::Value,
}

/// What's kept for `T`: its file, else what [`State::legacy`] reads, else
/// `None`. A file that can't be read is logged, unless the old reader can
/// read it (a file kept under the same name before it had a version).
pub fn load<T: State>() -> Option<T> {
    let file = state_file(T::FILE);
    let error = match std::fs::read_to_string(&file) {
        Ok(text) => match parse(&text) {
            Ok(value) => return Some(value),
            Err(error) => Some(error),
        },
        Err(_) => None,
    };
    let legacy = T::legacy();
    if let (None, Some(error)) = (&legacy, error) {
        log::warn!("{}: {error}", file.display());
    }
    legacy
}

/// Keeps `value` in `T`'s file, saying so on failure.
pub fn save<T: State>(value: &T) {
    save_to(&state_file(T::FILE), value);
}

#[cfg(test)]
fn load_from<T: State>(file: &Path) -> Option<T> {
    parse(&std::fs::read_to_string(file).ok()?).ok()
}

fn parse<T: State>(text: &str) -> Result<T, String> {
    let read: Read = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if read.version != T::VERSION {
        return Err(format!(
            "version {} where {} is known; left as it is",
            read.version,
            T::VERSION
        ));
    }
    serde_json::from_value(read.data).map_err(|e| e.to_string())
}

fn save_to<T: State>(file: &Path, value: &T) {
    let written = Written {
        version: T::VERSION,
        data: value,
    };
    let result = serde_json::to_string_pretty(&written)
        .map_err(std::io::Error::other)
        .and_then(|text| amane_kit::files::write_whole(file, text + "\n"));
    if let Err(error) = result {
        log::warn!("couldn't save {}: {error}", file.display());
    }
}

/// The text of the state file `name` in a format from before this module,
/// for a [`State::legacy`].
pub fn legacy_text(name: &str) -> Option<String> {
    std::fs::read_to_string(state_file(name)).ok()
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;

    use super::*;

    thread_local! {
        // the state folder this test thread uses in place of the real one
        pub(super) static FOLDER: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    }

    /// Runs `f` with the state folder at a new, empty folder of its own
    /// holding `files` (name, contents), as a test of a migration.
    pub(crate) fn in_folder<R>(
        test: &str,
        files: &[(&str, &str)],
        f: impl FnOnce(&Path) -> R,
    ) -> R {
        let root = folder(test);
        std::fs::create_dir_all(&root).unwrap();
        for (name, text) in files {
            let file = root.join(name);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        FOLDER.with_borrow_mut(|folder| *folder = Some(root.clone()));
        let result = f(&root);
        FOLDER.with_borrow_mut(|folder| *folder = None);
        std::fs::remove_dir_all(&root).unwrap();
        result
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(crate = "amane::serde")]
    struct Sample {
        names: Vec<String>,
    }

    impl State for Sample {
        const FILE: &'static str = "sample.json";
        const VERSION: u32 = 3;
    }

    /// A folder of its own for one test.
    pub(crate) fn folder(test: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("amane-state-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        folder
    }

    #[test]
    fn a_state_file_round_trips_with_its_version() {
        let file = folder("round-trip").join("sample.json");
        let sample = Sample {
            names: vec![String::from("a"), String::from("b")],
        };
        save_to(&file, &sample);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("\"version\": 3"), "{text}");
        assert_eq!(load_from::<Sample>(&file), Some(sample));
    }

    #[test]
    fn another_version_or_a_broken_file_reads_as_nothing() {
        assert!(parse::<Sample>(r#"{"version":2,"data":{"names":[]}}"#).is_err());
        assert!(parse::<Sample>(r#"{"names":[]}"#).is_err());
        assert!(parse::<Sample>("not json").is_err());
        assert_eq!(
            parse::<Sample>(r#"{"version":3,"data":{"names":["x"]}}"#),
            Ok(Sample {
                names: vec![String::from("x")]
            })
        );
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(crate = "amane::serde")]
    struct Migrated {
        line: String,
    }

    impl State for Migrated {
        const FILE: &'static str = "migrated.json";
        const VERSION: u32 = 1;

        fn legacy() -> Option<Self> {
            legacy_text("migrated").map(|line| Self {
                line: line.trim().to_string(),
            })
        }
    }

    #[test]
    fn an_old_file_is_read_kept_and_read_again_when_the_new_one_goes() {
        in_folder("migrate", &[("migrated", "old\n")], |root| {
            let old = load::<Migrated>().unwrap();
            assert_eq!(old.line, "old");
            save(&Migrated {
                line: String::from("new"),
            });
            assert_eq!(load::<Migrated>().unwrap().line, "new");
            // the old file is left as it was
            assert_eq!(
                std::fs::read_to_string(root.join("migrated")).unwrap(),
                "old\n"
            );
            std::fs::remove_file(root.join("migrated.json")).unwrap();
            assert_eq!(load::<Migrated>().unwrap().line, "old");
        });
    }
}
