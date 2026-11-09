//! What every emaki program needs below its UI, written once.
//!
//! A leaf crate, std, serde, log and libc only, so the crates without a window (the
//! phone, the assistant, the pinentry, the NixOS model) share it with the
//! kit, which re-exports it:
//!
//! | Module | What |
//! |---|---|
//! | [`paths`] | the XDG base folders |
//! | [`peer`] | who is at the other end of a socket, and what it runs |
//! | [`fs`] | writing a file whole, and JSON to a file |
//! | [`harden`] | no core dumps, and memory no other process reads |
//! | [`time`] | now, as Unix time |
//! | [`tools`] | the programs emaki runs, where only root or the build put them |
//! | [`format`](mod@format) | times, lengths, sizes and counts in words |
//! | [`sync`] | locks that outlive a panic |
//! | [`args`] | a `--flag value` from the command line |
//! | [`mime`] | a file's type from its name |
//! | [`nix`] | text quoted for a Nix expression |
//! | [`percent`] | `%20` escapes |
//! | [`intern`] | a string that lives for the program's run |
//! | [`app_ids`] | the programs' ids |
//! | [`migrate`] | the one-time move from the names before emaki |

#![warn(missing_docs)]

pub mod app_ids;
pub mod args;
pub mod format;
pub mod fs;
pub mod harden;
pub mod intern;
pub mod migrate;
pub mod mime;
pub mod nix;
pub mod paths;
pub mod peer;
pub mod percent;
pub mod sync;
pub mod time;
pub mod tools;
