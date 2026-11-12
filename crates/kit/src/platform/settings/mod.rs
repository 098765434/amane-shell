//! The shell's settings that a settings app changes and the shell follows
//! live: `~/.config/emaki/appearance.json`, `desktop.json`,
//! `launcher.json`, `status-bar.json`, `dock.json`, `nixos.json`,
//! `notifications.json`, `power.json` and `sound.json`,
//! through amane's `Config`. [`theme`](crate::theme) reads the appearance,
//! between the `EMAKI_*` overrides and what the system says.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::RwLockReadGuard;

use amane::serde::{Deserialize, Serialize};
use amane::{Color, Config, Service, Settings};

use crate::color::Accent;
use crate::icons::{self, Glyph};
use crate::theme::{Density, Glass, Mode};

/// Makes `$ty` the settings file `$file` (`.json`) in `$app`'s folder, read
/// with `$read()` and changed with `$change(…)`; anything after the `;`
/// goes in its `Settings` impl.
macro_rules! settings_file {
    ($ty:ty, $app:expr, $file:literal, $read:ident, $change:ident $(; $($extra:tt)*)?) => {
        impl Settings for $ty {
            const APP: &'static str = $app;
            const FILE: &'static str = $file;
            $($($extra)*)?
        }

        #[doc = concat!("`", $file, ".json` as it is now; a view that reads it redraws when it changes.")]
        pub fn $read() -> RwLockReadGuard<'static, Config<$ty>> {
            Config::<$ty>::read()
        }

        #[doc = concat!("Changes `", $file, ".json` and saves it, for input handlers.")]
        pub fn $change(edit: impl FnOnce(&mut $ty)) {
            Config::<$ty>::change(edit);
        }
    };
}

mod appearance;
mod desktop;
mod displays;
mod dock;
mod drop;
mod launcher;
mod nixos;
mod notifications;
mod passwords;
mod power;
mod seen;
mod sound;
mod status_bar;

pub use appearance::*;
pub use desktop::*;
pub use displays::*;
pub use dock::*;
pub use drop::*;
pub use launcher::*;
pub use nixos::*;
pub use notifications::*;
pub use passwords::*;
pub use power::*;
pub use seen::*;
pub use sound::*;
pub use status_bar::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_accent_and_turns_down_a_broken_one() {
        let with = |accent: &str| Appearance {
            accent: Some(String::from(accent)),
            ..Appearance::default()
        };

        assert_eq!(
            with("#0a84ff").accent_color(),
            Some(Color::rgb(10, 132, 255))
        );
        assert_eq!(with("0a84ff").accent_color(), None);
        assert_eq!(with("#0a84f").accent_color(), None);
        assert_eq!(Appearance::default().accent_color(), None);
    }

    #[test]
    fn an_older_file_keeps_the_defaults_for_what_it_lacks() {
        let appearance: Appearance =
            serde_json::from_str(r#"{ "mode": "light" }"#).unwrap_or_default();

        assert_eq!(appearance.mode, Some(Mode::Light));
        assert_eq!(appearance.text_scale, 1.0);
        assert_eq!(appearance.glass, Glass::Tinted);
        assert_eq!(appearance.reduce_motion, None);
    }

    #[test]
    fn the_desktop_shows_its_widgets_until_told_otherwise() {
        let desktop: Desktop =
            serde_json::from_str(r#"{ "widget_style": "dim" }"#).unwrap_or_default();

        assert!(desktop.show_widgets);
        assert_eq!(desktop.widget_style, WidgetStyle::Dim);
    }

    #[test]
    fn the_status_bar_reads_an_older_file_and_hides_by_name() {
        let mut bar: StatusBar =
            serde_json::from_str(r#"{ "position": "top", "hidden": ["volume"] }"#)
                .unwrap_or_default();

        assert_eq!(bar.position, BarPosition::Top);
        assert_eq!(bar.style, BarStyle::Frosted);
        assert_eq!(bar.size, BarSize::Compact);
        assert!(bar.show_workspaces);
        assert!(!bar.shows("volume"));

        bar.set_shown("volume", true);
        bar.set_shown("battery", false);
        bar.set_shown("battery", false);

        assert!(bar.shows("volume"));
        assert_eq!(bar.hidden, vec![String::from("battery")]);
    }

    #[test]
    fn the_status_bar_keeps_its_size() {
        let bar: StatusBar = serde_json::from_str(r#"{ "size": "comfortable" }"#).unwrap();
        assert_eq!(bar.size, BarSize::Comfortable);
        let json = serde_json::to_string(&bar).unwrap();
        assert!(json.contains(r#""size":"comfortable""#));
    }

    #[test]
    fn the_clock_follows_its_format() {
        let mut bar = StatusBar::default();
        assert_eq!(bar.time_pattern(), "%H:%M");

        bar.clock_24h = false;
        bar.clock_seconds = true;
        assert_eq!(bar.time_pattern(), "%-I:%M:%S %p");
    }

    #[test]
    fn the_dock_keeps_its_icons_within_bounds() {
        let dock: Dock = serde_json::from_str(r#"{ "icon_size": 200, "magnification": "off" }"#)
            .unwrap_or_default();

        assert_eq!(dock.icon(), 64.0);
        assert_eq!(dock.magnification, Magnify::Off);
        assert!(dock.auto_hide);
        assert_eq!(Dock::default().icon(), 44.0);
    }
}

#[cfg(test)]
mod bar_order_tests {
    use super::*;

    fn bar(order: &[&str]) -> StatusBar {
        StatusBar {
            order: order.iter().map(|name| String::from(*name)).collect(),
            ..StatusBar::default()
        }
    }

    #[test]
    fn no_order_is_the_usual_one() {
        assert_eq!(bar(&[]).items(), BAR_ITEMS.map(|item| item.0));
    }

    #[test]
    fn items_left_out_follow_their_usual_neighbour() {
        let items = bar(&["clock", "volume", "nonsense", "battery"]).items();
        assert_eq!(items.len(), BAR_ITEMS.len());
        assert_eq!(
            &items[..6],
            [
                "tray",
                "privacy",
                "status-icons",
                "activities",
                "weather",
                "clock"
            ]
        );
        let volume = items.iter().position(|item| *item == "volume").unwrap();
        assert_eq!(items[volume + 1], "bluetooth");
        let battery = items.iter().position(|item| *item == "battery").unwrap();
        assert_eq!(items[battery + 1], "quick-settings");
    }

    #[test]
    fn now_playing_and_the_timer_became_live_activities() {
        let items = bar(&["timer", "clock", "media"]).items();
        assert_eq!(items.len(), BAR_ITEMS.len());
        let at = |name| items.iter().position(|item| *item == name);
        assert!(at("activities") < at("clock"));
        assert!(at("media").is_none() && at("timer").is_none());
    }

    #[test]
    fn drop_tiles_read_as_written() {
        let grids = DropGrids::default();
        let text = serde_json::to_string(&grids).unwrap();
        assert_eq!(serde_json::from_str::<DropGrids>(&text).unwrap(), grids);
        let tile: DropTile = serde_json::from_str(r#"{"kind":"folder","path":"/tmp"}"#).unwrap();
        assert_eq!(
            tile,
            DropTile::Folder {
                path: PathBuf::from("/tmp"),
                mode: FolderMode::Copy
            }
        );
        assert_eq!(grids.columns(), 4);
        let upload: DropTile = serde_json::from_str(r#"{"kind":"upload","to":"0x0"}"#).unwrap();
        assert_eq!(
            upload,
            DropTile::Upload {
                to: UploadTo::ZeroX0
            }
        );
        let phone: DropTile = serde_json::from_str(r#"{"kind":"phone"}"#).unwrap();
        assert_eq!(phone, DropTile::Phone { device: None });
    }

    #[test]
    fn an_account_says_what_it_lacks() {
        let mut sharing = DropSharing::default();
        assert_eq!(sharing.missing(UploadTo::ZeroX0), None);
        assert_eq!(sharing.missing(UploadTo::Sftp), Some("No server"));
        sharing.sftp.host = String::from("example.org");
        sharing.sftp.user = String::from("me");
        assert_eq!(sharing.missing(UploadTo::Sftp), None);
        assert_eq!(sharing.sftp.port, 22);
        assert!(sharing.copy_link);
    }

    #[test]
    fn activities_are_allowed_until_denied() {
        let mut now = StatusBar::default();
        assert!(now.allows_activity("Drop"));
        now.set_activity_allowed("Drop", false);
        now.set_activity_allowed("Drop", false);
        assert_eq!(now.activities_denied, ["Drop"]);
        now.set_activity_allowed("Drop", true);
        assert!(now.allows_activity("Drop"));
        assert_eq!(now.compact_activities(), 2);
        now.activities_compact = 9;
        assert_eq!(now.compact_activities(), 3);
    }

    #[test]
    fn a_new_program_goes_after_the_tray_and_keeps_its_place() {
        let apps = [String::from("app:org.emaki.drop")];
        let mut now = bar(&[]);
        let items = now.items_with(&apps);
        assert_eq!(items.len(), BAR_ITEMS.len() + 1);
        assert_eq!(&items[..2], ["tray", "app:org.emaki.drop"]);

        now.move_item_with("app:org.emaki.drop", 5, &apps);
        assert_eq!(now.items_with(&apps)[5], "app:org.emaki.drop");

        // gone a while, it still has its place when it's back
        now.move_item_with("clock", 0, &[]);
        assert!(now.order.contains(&apps[0]));
        let items = now.items_with(&apps);
        let at = |name: &str| items.iter().position(|item| item == name);
        // one on, as the clock went before it
        assert_eq!(at("app:org.emaki.drop"), Some(6));
        assert_eq!(now.items().len(), BAR_ITEMS.len());
    }

    #[test]
    fn moving_keeps_every_item_once() {
        let mut now = bar(&[]);
        now.move_item("clock", 0);
        assert_eq!(now.items()[0], "clock");
        assert_eq!(now.items().len(), BAR_ITEMS.len());
        now.move_item("clock", 99);
        assert_eq!(now.items().last(), Some(&"clock"));
    }

    #[test]
    fn moving_the_bars_own_item_keeps_programs_places() {
        let apps = [String::from("app:org.emaki.drop")];
        let mut now = bar(&[]);
        now.move_item_with("app:org.emaki.drop", 5, &apps);
        now.move_item("clock", 0);
        assert!(now.order.contains(&apps[0]));
        assert_eq!(now.items_with(&apps)[0], "clock");
    }
}

#[cfg(test)]
mod bar_setting_tests {
    use super::*;

    #[test]
    fn hiding_was_a_switch_once() {
        let old: StatusBar = serde_json::from_str(r#"{ "auto_hide": true }"#).unwrap();
        assert_eq!(old.auto_hide, AutoHide::Always);
        let old: StatusBar = serde_json::from_str(r#"{ "auto_hide": false }"#).unwrap();
        assert_eq!(old.auto_hide, AutoHide::Never);
        let new: StatusBar = serde_json::from_str(r#"{ "auto_hide": "fullscreen" }"#).unwrap();
        assert_eq!(new.auto_hide, AutoHide::Fullscreen);
        let text = serde_json::to_string(&new).unwrap();
        assert!(text.contains(r#""auto_hide":"fullscreen""#));
    }

    #[test]
    fn the_clock_says_the_day_when_asked() {
        let mut bar = StatusBar::default();
        assert_eq!(bar.clock_pattern(true), "%H:%M");
        bar.show_weekday = true;
        assert_eq!(bar.clock_pattern(true), "%a  %H:%M");
        bar.show_date = ShowDate::WhenSpace;
        assert_eq!(bar.clock_pattern(true), "%a %-d %b  %H:%M");
        assert_eq!(bar.clock_pattern(false), "%a  %H:%M");
        bar.show_date = ShowDate::Always;
        assert_eq!(bar.clock_pattern(false), "%a %-d %b  %H:%M");
    }
}
