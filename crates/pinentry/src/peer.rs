//! Who is at the other end of the pinentry's socket: [`emaki_base::peer`], and the socket
//! itself.

use std::path::PathBuf;

pub use emaki_base::peer::*;

/// The socket the shell answers on: in the runtime directory, which only this user can enter.
pub fn socket_path() -> Option<PathBuf> {
    let runtime = emaki_base::paths::private_runtime()?;
    Some(
        runtime
            .join(emaki_base::app_ids::SHELL)
            .join("pinentry.sock"),
    )
}
