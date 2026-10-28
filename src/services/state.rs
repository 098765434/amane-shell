//! The shell's state directory, where the few settings it keeps live.

use std::path::PathBuf;

/// The file `name` under the shell's state directory
/// (`$XDG_STATE_HOME/amane-shell`), where the few settings it keeps live.
pub fn state_file(name: &str) -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(std::env::temp_dir);
    state.join("amane-shell").join(name)
}

/// Writes `text` to the state file `name`, saying so on failure.
pub fn save_state(name: &str, text: &str) {
    let path = state_file(name);
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, text));
    if let Err(e) = result {
        eprintln!("amane-shell: {}: {e}", path.display());
    }
}
