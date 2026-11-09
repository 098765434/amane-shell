//! The programs emaki runs (nix, git, curl, wl-copy, …), found in places only root or the
//! build controls rather than wherever `PATH` says: a `PATH` with a folder of the user's own in
//! front can't put another program in their place.
//!
//! Looked for, in order:
//! 1. the tools the nix build pinned, by store path (`EMAKI_TOOLS`, folders joined by `:`);
//! 2. `/run/wrappers/bin`, NixOS's setuid wrappers (pkexec must be this one);
//! 3. the system profile, `/run/current-system/sw/bin`;
//! 4. the user's profiles, `/etc/profiles/per-user/$USER/bin` and `~/.nix-profile/bin`.
//!
//! Only where none of those folders exists (not NixOS) is the bare name left for `PATH`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

// set by the nix build to the bin folders of the tools it pins
const PINNED: Option<&str> = option_env!("EMAKI_TOOLS");

const WRAPPERS: &str = "/run/wrappers/bin";
const SYSTEM: &str = "/run/current-system/sw/bin";

/// The folders looked in, in order.
fn folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = PINNED
        .unwrap_or("")
        .split(':')
        .filter(|folder| folder.starts_with('/'))
        .map(PathBuf::from)
        .collect();

    folders.push(PathBuf::from(WRAPPERS));
    folders.push(PathBuf::from(SYSTEM));

    if let Some(user) = std::env::var_os("USER").filter(|user| !user.is_empty()) {
        folders.push(Path::new("/etc/profiles/per-user").join(user).join("bin"));
    }
    if let Some(home) = std::env::var_os("HOME").filter(|home| !home.is_empty()) {
        folders.push(PathBuf::from(home).join(".nix-profile/bin"));
    }

    folders
}

/// Where `name` is, if it's in one of the folders; a name with a `/` in it is a path already.
pub fn find(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        let path = PathBuf::from(name);
        return path.is_file().then_some(path);
    }

    folders()
        .into_iter()
        .map(|folder| folder.join(name))
        .find(|path| path.is_file())
}

/// What to run for `name`: its path from [`find`], else (off NixOS) the bare name, for
/// `PATH`. A name that's a path already stays as it is.
pub fn path(name: &str) -> OsString {
    if name.contains('/') {
        return OsString::from(name);
    }

    if let Some(found) = find(name) {
        return found.into_os_string();
    }

    let nixos = [WRAPPERS, SYSTEM]
        .iter()
        .any(|folder| Path::new(folder).is_dir());

    if nixos {
        // a program that isn't installed fails to start, rather than being anyone's on PATH
        log::warn!("{name} isn't installed in the system or user profile");
        return Path::new(SYSTEM).join(name).into_os_string();
    }

    OsString::from(name)
}

/// Whether `name` is installed where [`find`] looks, or (off NixOS) on `PATH`.
pub fn installed(name: &str) -> bool {
    if find(name).is_some() {
        return true;
    }

    let nixos = [WRAPPERS, SYSTEM]
        .iter()
        .any(|folder| Path::new(folder).is_dir());

    !nixos
        && std::env::var_os("PATH").is_some_and(|path| {
            std::env::split_paths(&path).any(|folder| folder.join(name).is_file())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_paths() {
        assert_eq!(path("/bin/sh"), OsString::from("/bin/sh"));
        assert_eq!(path("./x"), OsString::from("./x"));
    }

    #[test]
    fn a_missing_program_is_never_left_to_path_on_nixos() {
        let resolved = path("emaki-surely-not-installed");
        if Path::new(SYSTEM).is_dir() {
            assert_eq!(
                resolved,
                Path::new(SYSTEM).join("emaki-surely-not-installed")
            );
        } else {
            assert_eq!(resolved, OsString::from("emaki-surely-not-installed"));
        }
        assert!(!installed("emaki-surely-not-installed"));
    }

    #[test]
    fn the_system_shell_is_found() {
        if Path::new(SYSTEM).is_dir() {
            assert!(find("sh").is_some_and(|sh| sh.is_absolute()));
        }
    }
}
