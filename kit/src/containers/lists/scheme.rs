//! The Material 3 baseline colour scheme (seed `#6750A4`), light and dark:
//! the roles the M3 list styles paint with. These are M3's own static
//! values, not derived from the shell's accent, so the M3 styles look the
//! way Material's own spec pages do. The shell's look never reads them.

use std::cell::Cell;

use amane::Color;

use crate::theme::{self, Mode};

/// The colour roles a list uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scheme {
    pub primary: Color,
    pub on_primary: Color,
    pub primary_container: Color,
    pub on_primary_container: Color,
    pub secondary: Color,
    pub secondary_container: Color,
    pub on_secondary_container: Color,
    pub tertiary_container: Color,
    pub on_tertiary_container: Color,
    pub surface: Color,
    pub on_surface: Color,
    pub on_surface_variant: Color,
    pub outline: Color,
    pub outline_variant: Color,
    pub surface_container_low: Color,
    pub surface_container: Color,
    pub surface_container_highest: Color,
}

const fn hex(v: u32) -> Color {
    Color::rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

pub const LIGHT: Scheme = Scheme {
    primary: hex(0x6750A4),
    on_primary: hex(0xFFFFFF),
    primary_container: hex(0xEADDFF),
    on_primary_container: hex(0x4F378B),
    secondary: hex(0x625B71),
    secondary_container: hex(0xE8DEF8),
    on_secondary_container: hex(0x4A4458),
    tertiary_container: hex(0xFFD8E4),
    on_tertiary_container: hex(0x633B48),
    surface: hex(0xFEF7FF),
    on_surface: hex(0x1D1B20),
    on_surface_variant: hex(0x49454F),
    outline: hex(0x79747E),
    outline_variant: hex(0xCAC4D0),
    surface_container_low: hex(0xF7F2FA),
    surface_container: hex(0xF3EDF7),
    surface_container_highest: hex(0xE6E0E9),
};

pub const DARK: Scheme = Scheme {
    primary: hex(0xD0BCFF),
    on_primary: hex(0x381E72),
    primary_container: hex(0x4F378B),
    on_primary_container: hex(0xEADDFF),
    secondary: hex(0xCCC2DC),
    secondary_container: hex(0x4A4458),
    on_secondary_container: hex(0xE8DEF8),
    tertiary_container: hex(0x633B48),
    on_tertiary_container: hex(0xFFD8E4),
    surface: hex(0x141218),
    on_surface: hex(0xE6E0E9),
    on_surface_variant: hex(0xCAC4D0),
    outline: hex(0x938F99),
    outline_variant: hex(0x49454F),
    surface_container_low: hex(0x1D1B20),
    surface_container: hex(0x211F26),
    surface_container_highest: hex(0x36343B),
};

thread_local! {
    // the mode the M3 scheme is drawn in, when not the shell's
    static MODE: Cell<Option<Mode>> = const { Cell::new(None) };
}

/// Draws the M3 styles in `mode` rather than the shell's, so a showcase can
/// flip them without restarting (the shell's own mode is read once).
pub fn set_mode(mode: Option<Mode>) {
    MODE.set(mode);
}

/// The mode the M3 styles are drawn in now.
pub fn mode() -> Mode {
    MODE.get().unwrap_or_else(theme::mode)
}

/// The scheme for the current mode.
pub fn current() -> &'static Scheme {
    match mode() {
        Mode::Light => &LIGHT,
        Mode::Dark => &DARK,
    }
}
