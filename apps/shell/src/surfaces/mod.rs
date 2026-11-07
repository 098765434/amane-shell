//! The shell's surfaces: the status bar, the dock, the launcher,
//! notifications, Quick Settings, the desktop widgets, the password fill, the
//! alert and the on-screen display. Each is documented in `docs/shell/`.
//!
//! Each is a folder whose `mod.rs` is its only face to the others: its
//! surface, and the functions and types it offers. What it keeps and draws
//! is in its own files (`state`, `view`, `input`, `ipc` as it needs them),
//! which no other surface reaches into; the tests below hold that. What
//! several of them share and only the shell has lives in [`crate::chrome`]
//! and [`crate::services`]; the rest is the kit's.

pub mod alert;
pub mod dock;
pub mod fill;
pub mod launcher;
pub mod notifications;
pub mod osd;
pub mod quick_settings;
pub mod status_bar;
pub mod widgets;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    /// How long a surface's file may be. Past this it gets split along what
    /// it does (docs/development/shell-structure.md).
    const MOST_LINES: usize = 650;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
    }

    fn files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// Each surface by name, and the modules its `mod.rs` declares.
    fn surfaces() -> BTreeMap<String, Vec<String>> {
        let mut out = BTreeMap::new();
        for entry in std::fs::read_dir(root().join("surfaces"))
            .unwrap()
            .flatten()
        {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(path.join("mod.rs")).unwrap();
            let modules = text
                .lines()
                .filter_map(|line| {
                    let line = line
                        .trim_start_matches("pub ")
                        .trim_start_matches("pub(crate) ");
                    line.strip_prefix("mod ")?
                        .strip_suffix(';')
                        .map(String::from)
                })
                .collect();
            out.insert(name, modules);
        }
        out
    }

    #[test]
    fn every_surface_is_a_folder_with_its_own_mod_rs() {
        for entry in std::fs::read_dir(root().join("surfaces"))
            .unwrap()
            .flatten()
        {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            assert!(
                path.is_dir() || name == "mod.rs",
                "surfaces/{name}: a surface is a folder"
            );
        }
    }

    #[test]
    fn no_surface_reaches_into_another() {
        let surfaces = surfaces();
        let mut found = Vec::new();
        let mut all = Vec::new();
        files(&root().join("surfaces"), &mut all);
        for file in all {
            let rel = file.strip_prefix(root().join("surfaces")).unwrap();
            let own = rel
                .components()
                .next()
                .unwrap()
                .as_os_str()
                .to_string_lossy()
                .into_owned();
            let text = std::fs::read_to_string(&file).unwrap();
            for (other, modules) in &surfaces {
                if *other == own {
                    continue;
                }
                for module in modules {
                    let path = format!("surfaces::{other}::{module}::");
                    let braced = format!("surfaces::{other}::{{{module}");
                    if text.contains(&path) || text.contains(&braced) {
                        found.push(format!("{}: {path}", rel.display()));
                    }
                }
                // another surface's IPC is its own; surfaces call each other's functions
                if text.contains(&format!("{other}::run(")) && !text.contains("fn run(") {
                    found.push(format!("{}: {other}::run(", rel.display()));
                }
            }
        }
        assert!(
            found.is_empty(),
            "reaching past another surface's mod.rs: {found:#?}"
        );
    }

    #[test]
    fn services_and_chrome_know_no_surface() {
        let mut all = Vec::new();
        files(&root().join("services"), &mut all);
        files(&root().join("chrome"), &mut all);
        let found: Vec<_> =
            all.iter()
                .filter(|file| {
                    std::fs::read_to_string(file).unwrap().lines().any(|line| {
                        !line.trim_start().starts_with("//") && line.contains("surfaces::")
                    })
                })
                .collect();
        assert!(
            found.is_empty(),
            "services and chrome use surfaces: {found:#?}"
        );
    }

    #[test]
    fn no_surface_file_is_too_long() {
        let mut all = Vec::new();
        files(&root().join("surfaces"), &mut all);
        let long: Vec<_> = all
            .iter()
            .map(|file| (file, std::fs::read_to_string(file).unwrap().lines().count()))
            .filter(|(_, lines)| *lines > MOST_LINES)
            .collect();
        assert!(long.is_empty(), "over {MOST_LINES} lines: {long:#?}");
    }
}
