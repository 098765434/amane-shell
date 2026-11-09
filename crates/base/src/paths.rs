//! The XDG base folders, as the spec says to find them: the variable when it's
//! an absolute path, else its default under `$HOME`. Each app adds its own
//! name (`emaki`, `emaki-drop`) below.

use std::path::PathBuf;

fn base(variable: &str, under_home: &str) -> PathBuf {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(under_home)))
        .unwrap_or_default()
}

/// `$XDG_CONFIG_HOME`, else `~/.config`: settings people edit.
pub fn config() -> PathBuf {
    base("XDG_CONFIG_HOME", ".config")
}

/// `$XDG_STATE_HOME`, else `~/.local/state`: what a program keeps between runs that isn't a
/// setting, like a layout or a history.
pub fn state() -> PathBuf {
    base("XDG_STATE_HOME", ".local/state")
}

/// `$XDG_CACHE_HOME`, else `~/.cache`: what can be made again.
pub fn cache() -> PathBuf {
    base("XDG_CACHE_HOME", ".cache")
}

/// `$XDG_DATA_HOME`, else `~/.local/share`: data a program installs or downloads.
pub fn data() -> PathBuf {
    base("XDG_DATA_HOME", ".local/share")
}

/// `$XDG_RUNTIME_DIR`, else a folder of this user's own in the temporary folder: sockets and
/// files for this session only, never reachable by another user.
pub fn runtime() -> PathBuf {
    private_runtime().unwrap_or_else(|| {
        static FALLBACK: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        FALLBACK.get_or_init(private_temp).clone()
    })
}

/*
 * without a runtime directory: /tmp/emaki-<uid>, made 0700, and taken only when it's a real
 * folder of this user's that no one else can enter; another user could have made it first (to
 * read or swap what's put there), so then a fresh folder with a name nobody can guess
 */
fn private_temp() -> PathBuf {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};

    // SAFETY: getuid can't fail
    let uid = unsafe { libc::getuid() };
    let temp = std::env::temp_dir();
    let folder = temp.join(format!("emaki-{uid}"));

    let _ = std::fs::DirBuilder::new().mode(0o700).create(&folder);

    let ours = std::fs::symlink_metadata(&folder)
        .is_ok_and(|meta| meta.is_dir() && meta.uid() == uid && meta.mode() & 0o077 == 0);
    if ours {
        return folder;
    }

    log::warn!(
        "{} isn't this user's own; using a fresh folder",
        folder.display()
    );
    let mut template = temp
        .join("emaki-XXXXXX")
        .into_os_string()
        .into_encoded_bytes();
    template.push(0);
    // SAFETY: a writable, NUL-ended template ending in XXXXXX, which mkdtemp fills in
    let made = unsafe { libc::mkdtemp(template.as_mut_ptr().cast()) };
    if made.is_null() {
        // nowhere private at all: the runtime directory's place, which won't exist
        return PathBuf::from(format!("/run/user/{uid}"));
    }
    template.pop();
    // SAFETY: mkdtemp gave back the template, now naming a 0700 folder
    PathBuf::from(unsafe { std::ffi::OsString::from_encoded_bytes_unchecked(template) })
}

/// `$XDG_RUNTIME_DIR` alone, which only this user can enter: for a socket
/// that must not be reachable by anyone else, which goes without rather
/// than fall back to the shared temporary folder.
pub fn private_runtime() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_under_home_for_a_relative_or_missing_variable() {
        // the variables are the process's, so one test covers them in turn
        let home = std::env::var_os("HOME");
        // SAFETY: tests touching the environment run in this one test
        unsafe {
            std::env::set_var("HOME", "/home/someone");
            std::env::set_var("XDG_CONFIG_HOME", "relative/config");
            std::env::remove_var("XDG_STATE_HOME");
            std::env::set_var("XDG_CACHE_HOME", "/var/cache/someone");
        }
        assert_eq!(config(), PathBuf::from("/home/someone/.config"));
        assert_eq!(state(), PathBuf::from("/home/someone/.local/state"));
        assert_eq!(cache(), PathBuf::from("/var/cache/someone"));
        // SAFETY: as above, this test alone touches these variables
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("XDG_CACHE_HOME");
            match home {
                Some(home) => std::env::set_var("HOME", home),
                None => std::env::remove_var("HOME"),
            }
        }
    }
}
