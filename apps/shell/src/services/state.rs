//! The shell's state directory, where the few settings it keeps live.

use std::path::PathBuf;

/// The file `name` under the shell's state directory
/// (`$XDG_STATE_HOME/amane-shell`), where the few settings it keeps live.
pub fn state_file(name: &str) -> PathBuf {
    let state = amane_kit::paths::state();
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
