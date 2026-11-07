//! The design tokens, which live in the kit (`amane_kit::tokens`) so apps
//! share them, and the niri layer rules generated from them, which name the
//! shell's own surfaces.

pub use amane_kit::tokens::*;

#[cfg(test)]
mod contract;
pub mod layer_rules;
