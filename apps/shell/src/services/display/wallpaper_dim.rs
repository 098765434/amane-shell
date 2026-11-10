//! The wallpaper under Black: while the shell shows black surfaces
//! (`theme::surface()`), niri dims the wallpaper as `appearance.json`'s
//! `black_wallpaper` asks, over a black backdrop (the niri fork's
//! `SetWallpaperDim`, feature `wallpaper-dim`); otherwise it's as configured.
//! niri keeps the level until it's set again or it restarts, and isn't asked
//! when it's already there. Stock niri, or a fork without the feature, isn't
//! asked at all.

use std::sync::Once;

use amane::{Appearance, Config, Service, niri};
use emaki_kit::settings::{self, BlackWallpaper};
use emaki_kit::theme::{self, Surface};

use crate::services::Wake;

static WAKE: Wake = Wake::new();

/// Starts following the appearance, once.
pub fn watch() {
    static WATCH: Once = Once::new();
    WATCH.call_once(|| {
        Config::<settings::Appearance>::subscribe(|_| WAKE.nudge()).forever();
        // Auto follows the system's light and dark
        Appearance::subscribe(|_| WAKE.nudge()).forever();
        std::thread::spawn(|| {
            if !niri::features().has("wallpaper-dim") {
                return;
            }
            // niri starts undimmed
            let mut sent = 0;
            loop {
                let wanted = percent(theme::surface(), settings::appearance().black_wallpaper);
                if wanted != sent && set(wanted) {
                    sent = wanted;
                }
                WAKE.wait(None);
            }
        });
    });
}

/// How much niri dims the wallpaper: only under black surfaces.
fn percent(surface: Surface, wallpaper: BlackWallpaper) -> u8 {
    match surface {
        Surface::Black => wallpaper.percent(),
        Surface::Standard => 0,
    }
}

fn set(percent: u8) -> bool {
    let args = ["msg", "action", "set-wallpaper-dim", &percent.to_string()].map(String::from);
    emaki_kit::process::run_now("niri", &args).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_black_dims_the_wallpaper() {
        assert_eq!(percent(Surface::Standard, BlackWallpaper::Hidden), 0);
        assert_eq!(percent(Surface::Black, BlackWallpaper::Hidden), 100);
        assert_eq!(percent(Surface::Black, BlackWallpaper::Dimmed), 75);
        assert_eq!(percent(Surface::Black, BlackWallpaper::Shown), 0);
    }
}
