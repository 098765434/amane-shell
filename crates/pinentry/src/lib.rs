//! rbw's pinentry and the shell's side of it.
//!
//! rbw-agent asks a pinentry program for the master password (and 2FA codes, and a register
//! key) over the Assuan protocol on its stdin and stdout. `amane-pinentry` speaks that to rbw,
//! and asks the shell to show its own dialog over a private socket:
//!
//! - **The pinentry checks the shell.** SO_PEERCRED's user is its own, and the process at the
//!   other end runs `amane-shell` from the Nix store.
//! - **The shell checks the pinentry** (`peer`). It runs `amane-pinentry` from the store, and
//!   its parent runs `rbw-agent` from the store. A pidfd keeps a recycled pid from passing.
//! - **Not D-Bus.** Any program of the user's can watch the session bus (BecomeMonitor), so a
//!   pin never goes over it.
//!
//! When the shell isn't there or won't ask, the pinentry runs the fallback (pinentry-gnome3)
//! in its place, before it has said a word to rbw.

pub mod assuan;
pub mod peer;
pub mod wire;
