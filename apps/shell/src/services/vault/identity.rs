//! Which app owns a focused field, checked rather than taken on its word.
//!
//! The compositor says what runs the field (the process's executable, read from /proc, for the
//! pid on the app's socket) and the app id the window claims. The app id is only believed when:
//!
//! - its desktop entry is found outside the home folder (so the system or home manager put it
//!   there, not something the user's programs can write), and lives in the Nix store;
//! - the executable is that entry's own package, or its `-unwrapped` twin that the package
//!   depends on (as firefox has firefox-unwrapped) — never any other part of its closure, which
//!   would let `ld-linux.so ./evil` or a shared python pass;
//! - the executable isn't a loader or an interpreter, which runs whatever it's handed.
//!
//! Anything else (an app id nothing installed provides, a program in the home folder, a shared
//! runtime like electron, a sandbox) is unverified, and gets no suggestions of its own. A browser
//! (Categories=WebBrowser, or http links) is verified as one, and fill stays out of it.

use amane_kit::sync::Locked as _;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use amane::FieldOwner;

const NIX_STORE: &str = "/run/current-system/sw/bin/nix-store";

/// An app the checks passed for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    /// Its desktop entry's id, without .desktop.
    pub desktop_id: String,

    /// The package the desktop entry comes from, without its version, like "telegram-desktop".
    pub package: String,

    /// The app's name from its desktop entry.
    pub name: String,

    /// Its desktop entry says it's a web browser: many sites in one app.
    pub browser: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identity {
    Verified(Verified),
    Unverified(Why),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    UnknownProcess,
    Sandboxed,
    NoDesktopEntry,
    NotFromStore,
    OtherProgram,
    Runtime,
}

impl Why {
    pub fn describe(self) -> &'static str {
        match self {
            Why::UnknownProcess => "Its program couldn't be told",
            Why::Sandboxed => "It runs in a sandbox",
            Why::NoDesktopEntry => "No installed app has its name",
            Why::NotFromStore => "It isn't installed by the system",
            Why::OtherProgram => "Another program uses its name",
            Why::Runtime => "It runs in a shared runtime",
        }
    }
}

// the store paths each package needs, asked of nix-store once each
static CLOSURES: Mutex<Option<HashMap<PathBuf, HashSet<PathBuf>>>> = Mutex::new(None);

/// Checks a field's owner. Blocks while nix-store answers the first time, so call it off the
/// UI thread.
pub fn check(owner: &FieldOwner) -> Identity {
    if owner.sandboxed {
        return Identity::Unverified(Why::Sandboxed);
    }

    let Some(exe) = owner.exe.as_deref() else {
        return Identity::Unverified(Why::UnknownProcess);
    };

    if is_runtime(exe) {
        return Identity::Unverified(Why::Runtime);
    }

    let Some(program) = store_path(exe) else {
        return Identity::Unverified(Why::NotFromStore);
    };

    let Some(entry) = desktop_entry(&owner.app_id) else {
        return Identity::Unverified(Why::NoDesktopEntry);
    };

    // an entry the user's own programs could have put there (or linked from there) proves nothing
    if in_home(&entry.path) {
        return Identity::Unverified(Why::NotFromStore);
    }

    let Some(package) = fs::canonicalize(&entry.path)
        .ok()
        .and_then(|path| store_path(&path))
    else {
        return Identity::Unverified(Why::NotFromStore);
    };

    if !same_program(&package, &program) {
        return Identity::Unverified(Why::OtherProgram);
    }

    let desktop_file = entry.path;
    let name = entry.name;

    let desktop_id = desktop_file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();

    Identity::Verified(Verified {
        desktop_id,
        package: package_name(&package),
        name,
        browser: entry.browser,
    })
}

/*
 * whether the program is the package's own: the same store path, or the package's
 * `<name>-unwrapped` (or `-bin`) twin, which the package must depend on. Any other store path
 * in the closure is refused: glibc's loader, a shared interpreter or electron are in many
 */
fn same_program(package: &Path, program: &Path) -> bool {
    if package == program {
        return true;
    }

    let (package_name, program_name) = (package_name(package), package_name(program));

    let twin = [
        format!("{package_name}-unwrapped"),
        format!("{package_name}-bin"),
    ];

    twin.contains(&program_name) && needs(package, program)
}

// loaders and interpreters run whatever they're handed, so they say nothing about the app
fn is_runtime(exe: &Path) -> bool {
    let Some(name) = exe.file_name().and_then(|name| name.to_str()) else {
        return true;
    };

    let name = name
        .strip_prefix('.')
        .and_then(|name| name.strip_suffix("-wrapped"))
        .unwrap_or(name);

    let starts = [
        "ld-linux", "ld.so", "ld64", "python", "perl", "ruby", "node", "deno", "bun", "java",
        "electron", "gjs", "lua", "php", "qemu", "wine", "bash", "sh", "zsh", "fish", "dash",
        "busybox", "env", "tclsh", "wish", "guile", "racket", "Rscript",
    ];

    starts.iter().any(|start| {
        name == *start
            || name.strip_prefix(start).is_some_and(|rest| {
                rest.starts_with(|letter: char| {
                    letter.is_ascii_digit() || letter == '.' || letter == '-'
                })
            })
    })
}

// a path in the home folder, where the user's own programs can write
fn in_home(path: &Path) -> bool {
    let homes = [std::env::var_os("HOME"), std::env::var_os("XDG_DATA_HOME")];

    homes
        .into_iter()
        .flatten()
        .filter(|home| !home.is_empty())
        .any(|home| path.starts_with(home))
}

/// The store path a file is in: `/nix/store/<hash>-<name>`.
pub fn store_path(file: &Path) -> Option<PathBuf> {
    let rest = file.strip_prefix("/nix/store/").ok()?;
    let top = rest.components().next()?;

    let top = top.as_os_str().to_str()?;

    // <32 letters of hash>-<name>
    let (hash, name) = top.split_at_checked(32)?;
    if !hash.bytes().all(|byte| byte.is_ascii_alphanumeric()) || !name.starts_with('-') {
        return None;
    }

    Some(Path::new("/nix/store").join(top))
}

/// A store path's package name without its version: "telegram-desktop" from
/// `/nix/store/<hash>-telegram-desktop-5.6.3`.
pub fn package_name(store: &Path) -> String {
    let name = store
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();

    let name = name.get(33..).unwrap_or("");

    // the version starts at the first part that starts with a digit
    let mut kept: Vec<&str> = Vec::new();
    for part in name.split('-') {
        if part.starts_with(|letter: char| letter.is_ascii_digit()) && !kept.is_empty() {
            break;
        }
        kept.push(part);
    }

    kept.join("-")
}

// whether the package refers to the program's store path directly, asked of nix-store
fn needs(package: &Path, program: &Path) -> bool {
    let mut closures = CLOSURES.locked();
    let closures = closures.get_or_insert_with(HashMap::new);

    if !closures.contains_key(package) {
        let paths: Option<HashSet<PathBuf>> = Command::new(NIX_STORE)
            .args(["--query", "--references"])
            .arg(package)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(PathBuf::from)
                    .collect()
            });

        // a failed query isn't kept: it's asked again next time
        let Some(paths) = paths else {
            return false;
        };

        if closures.len() > 256 {
            closures.clear();
        }

        closures.insert(package.to_path_buf(), paths);
    }

    closures
        .get(package)
        .is_some_and(|paths| paths.contains(program))
}

/*
 * the desktop entry an app id names: <app id>.desktop in the data folders, or an entry whose
 * StartupWMClass is the app id, with the app's name. The first found wins, as for launching,
 * so an entry in the home folder shadows the system's and leaves the app unverified
 */
struct DesktopEntry {
    path: PathBuf,
    name: String,
    browser: bool,
}

fn desktop_entry(app_id: &str) -> Option<DesktopEntry> {
    if app_id.is_empty() || app_id.contains('/') || app_id.starts_with('.') {
        return None;
    }

    let folders = application_folders();

    for folder in &folders {
        let path = folder.join(format!("{app_id}.desktop"));

        if let Some(entry) = read_entry(path) {
            return Some(entry);
        }
    }

    for folder in &folders {
        let Ok(files) = fs::read_dir(folder) else {
            continue;
        };

        for file in files.flatten() {
            let path = file.path();

            if path
                .extension()
                .is_none_or(|extension| extension != "desktop")
            {
                continue;
            }

            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };

            let class = text
                .lines()
                .find_map(|line| line.strip_prefix("StartupWMClass="))
                .map(str::trim);

            if class.is_some_and(|class| class.eq_ignore_ascii_case(app_id))
                && let Some(entry) = read_entry(path)
            {
                return Some(entry);
            }
        }
    }

    None
}

fn read_entry(path: PathBuf) -> Option<DesktopEntry> {
    let text = fs::read_to_string(&path).ok()?;

    let main: Vec<&str> = text
        .lines()
        .skip_while(|line| line.trim() != "[Desktop Entry]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .collect();

    let value = |key: &str| {
        main.iter()
            .find_map(|line| line.strip_prefix(key))
            .map(|value| value.trim().to_string())
    };

    let name = value("Name=")?;

    let categories = value("Categories=").unwrap_or_default();
    let mime = value("MimeType=").unwrap_or_default();

    let browser = categories
        .split(';')
        .any(|category| category == "WebBrowser")
        || mime
            .split(';')
            .any(|kind| kind == "x-scheme-handler/http" || kind == "x-scheme-handler/https");

    Some(DesktopEntry {
        path,
        name,
        browser,
    })
}

fn application_folders() -> Vec<PathBuf> {
    let mut folders = Vec::new();

    let home_data = std::env::var_os("XDG_DATA_HOME")
        .filter(|folder| !folder.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    folders.extend(home_data);

    let data = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| String::from("/usr/local/share:/usr/share"));
    folders.extend(
        data.split(':')
            .filter(|folder| !folder.is_empty())
            .map(PathBuf::from),
    );

    folders
        .into_iter()
        .map(|folder| folder.join("applications"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_paths_and_package_names() {
        let exe = Path::new(
            "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-telegram-desktop-5.6.3/bin/.telegram-desktop-wrapped",
        );

        let store = store_path(exe).unwrap();
        assert_eq!(
            store,
            Path::new("/nix/store/0123456789abcdfghijklmnpqrsvwxyz-telegram-desktop-5.6.3")
        );
        assert_eq!(package_name(&store), "telegram-desktop");

        assert_eq!(
            package_name(Path::new(
                "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-vscode-1.105.1"
            )),
            "vscode"
        );
        assert_eq!(
            package_name(Path::new(
                "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-7zz-25.01"
            )),
            "7zz"
        );

        assert!(store_path(Path::new("/home/me/.local/bin/telegram")).is_none());
        assert!(store_path(Path::new("/nix/store/short-name/bin/x")).is_none());
    }

    #[test]
    fn an_owner_without_a_program_is_unverified() {
        let owner = FieldOwner {
            app_id: String::from("org.telegram.desktop"),
            ..FieldOwner::default()
        };
        assert_eq!(check(&owner), Identity::Unverified(Why::UnknownProcess));

        let owner = FieldOwner {
            exe: Some(PathBuf::from("/tmp/evil")),
            app_id: String::from("org.telegram.desktop"),
            ..FieldOwner::default()
        };
        assert_eq!(check(&owner), Identity::Unverified(Why::NotFromStore));

        let owner = FieldOwner {
            sandboxed: true,
            ..FieldOwner::default()
        };
        assert_eq!(check(&owner), Identity::Unverified(Why::Sandboxed));
    }

    #[test]
    fn loaders_and_interpreters_are_runtimes() {
        let store = |name: &str| {
            PathBuf::from(format!(
                "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-x-1/bin/{name}"
            ))
        };

        for name in [
            "ld-linux-x86-64.so.2",
            "python3",
            "python3.12",
            "node",
            ".bash-wrapped",
            "electron",
            "java",
            "sh",
        ] {
            assert!(is_runtime(&store(name)), "{name}");
        }
        for name in [
            "telegram-desktop",
            "code",
            ".firefox-wrapped",
            "shotwell",
            "nodepad",
        ] {
            assert!(!is_runtime(&store(name)), "{name}");
        }
    }

    #[test]
    fn only_the_package_or_its_twin_is_the_program() {
        let package =
            Path::new("/nix/store/0123456789abcdfghijklmnpqrsvwxyz-telegram-desktop-5.6.3");
        let glibc = Path::new("/nix/store/0123456789abcdfghijklmnpqrsvwxyz-glibc-2.40-66");
        let lookalike =
            Path::new("/nix/store/0123456789abcdfghijklmnpqrsvwxyz-telegram-desktop-evil-1");

        assert!(same_program(package, package));
        assert!(!same_program(package, glibc));
        assert!(!same_program(package, lookalike));
    }

    #[test]
    fn odd_app_ids_find_no_entry() {
        assert!(desktop_entry("../../etc/passwd").is_none());
        assert!(desktop_entry("").is_none());
        assert!(desktop_entry(".hidden").is_none());
    }
}
