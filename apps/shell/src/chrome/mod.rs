//! What several of the shell's surfaces share and only the shell has: which
//! panel is open (and its exclusive edge), where the bar is, and the battery
//! glyph the bar and Quick Settings draw. Generic pieces are the kit's (`amane-kit`); see
//! `docs/kit/components.md` and `docs/kit/controls.md`.

pub mod bar;
pub mod battery;
pub mod panel;
pub mod per_output;
pub mod workspace_menu;
