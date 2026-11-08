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

/// `$XDG_RUNTIME_DIR`, else the temporary folder: sockets and files for this session only.
pub fn runtime() -> PathBuf {
    private_runtime().unwrap_or_else(std::env::temp_dir)
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
