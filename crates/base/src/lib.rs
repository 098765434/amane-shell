//! What every amane-shell program needs below its UI, written once.
//!
//! A leaf crate, std, serde and log only, so the crates without a window (the
//! phone, the assistant, the pinentry, the NixOS model) share it with the
//! kit, which re-exports it:
//!
//! | Module | What |
//! |---|---|
//! | [`paths`] | the XDG base folders |
//! | [`fs`] | writing a file whole, and JSON to a file |
//! | [`time`] | now, as Unix time |
//! | [`format`] | times, lengths, sizes and counts in words |
//! | [`sync`] | locks that outlive a panic |
//! | [`args`] | a `--flag value` from the command line |
//! | [`mime`] | a file's type from its name |
//! | [`percent`] | `%20` escapes |
//! | [`intern`] | a string that lives for the program's run |
//! | [`app_ids`] | the programs' ids |
//! | [`migrate`] | the one-time move from the names before emaki |

#![warn(missing_docs)]

pub mod app_ids;
pub mod args;
pub mod format;
pub mod fs;
pub mod intern;
pub mod migrate;
pub mod mime;
pub mod paths;
pub mod percent;
pub mod sync;
pub mod time;
