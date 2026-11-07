//! Dark Style — the system's light/dark preference, which apps (and the
//! shell's own theme) follow.
//!
//! It's read from the Settings portal through amane's `Appearance`, which
//! has no way to write it: the portal can't. The GNOME and GTK portal
//! backends read it from dconf (`/org/gnome/desktop/interface/
//! color-scheme`), so that's where it's written; the portal then announces
//! the change and `Appearance` follows. There's no state here, only the
//! write.

use amane::{Appearance, Service};

use crate::services::run;

const KEY: &str = "/org/gnome/desktop/interface/color-scheme";

/// Whether the shell can read the preference back at all.
pub fn present() -> bool {
    Appearance::read().present() && crate::services::installed("dconf")
}

pub fn dark() -> bool {
    Appearance::read().prefers_dark()
}

pub fn set(dark: bool) {
    run(
        "dconf",
        vec!["write".into(), KEY.into(), value(dark).into()],
    );
}

/// GVariant text for dconf: a quoted string.
fn value(dark: bool) -> &'static str {
    if dark { "'prefer-dark'" } else { "'default'" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_gnomes_values() {
        assert_eq!(value(true), "'prefer-dark'");
        assert_eq!(value(false), "'default'");
    }
}
