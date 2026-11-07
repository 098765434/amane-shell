//! The launcher's assistant, the part the shell and the daemon share: what a
//! command is ([`Command`]), what the machine has for one to name
//! ([`Snapshot`]), reading a command from what was typed or said, first by
//! [`rules`] and then by Needle 3 through the [`tools`] it's offered, and
//! how the shell and the daemon talk ([`protocol`], [`client`]).
//!
//! Nothing here runs a command: the shell does, on its own thread, through
//! the actions and services it already has. A command found here is only a
//! proposal, with how sure it is and how much care running it needs.

#![warn(missing_docs)]

pub mod client;
pub mod command;
pub mod protocol;
pub mod rules;
pub mod snapshot;
pub mod text;
pub mod tools;

pub use command::{Command, Level, Media, Power, Risk, State, Target};
pub use snapshot::{App, Snapshot, Switch};

/// A command read from a query, and how it was read.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// The command read.
    pub command: Command,
    /// How sure the reading is, 0 … 1: 1 for a rule's.
    pub confidence: f32,
    /// What read it.
    pub source: Source,
}

/// What read a [`Reading`]: a rule, or the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The phrasing matched one of [`rules`]: certain.
    Rules,
    /// Needle 3 called a tool for it.
    Model,
}

/// The name of the daemon's socket in `$XDG_RUNTIME_DIR`.
pub const SOCKET: &str = "amane-assistant.sock";

/// The daemon's socket: `$XDG_RUNTIME_DIR/amane-assistant.sock`.
pub fn socket_path() -> std::path::PathBuf {
    amane_base::paths::runtime().join(SOCKET)
}
