//! The privacy dots (orange microphone, green camera), whose menu says
//! which apps and brings their windows up. Now Playing and screen sharing
//! are activities now (`activities.rs`).

use amane::{Center, Color, Rectangle, Service, Size, Widget};

use crate::services::desktop::privacy::{Privacy, User};
use crate::surfaces::quick_settings::{self, Page};
use emaki_kit::buttons::DrawnButton;
use emaki_kit::icons::{ICON_CAMERA, ICON_MIC};
use emaki_kit::layout::{boxed, cursor, icon};
use emaki_kit::menu::layer as context_menu;
use emaki_kit::menu::{ContextMenu, Icon, MenuItem};

use super::{BAR_NS, item, menu_edge};

/// The privacy colours: the microphone and the camera.
const ORANGE: Color = Color::rgb(0xff, 0x9f, 0x0a);
const GREEN: Color = Color::rgb(0x30, 0xd1, 0x58);

const PRIVACY_KEY: &str = "emaki.bar.privacy";

// ───── privacy ───────────────────────────────────────────────────────────────

/// The screen-sharing chip and the camera and microphone dots, while in
/// use.
pub fn privacy() -> Vec<Box<dyn Widget>> {
    let privacy = Privacy::read();
    let camera = privacy.camera();
    let microphone = privacy.recording();
    let names = |users: &[User]| {
        users
            .iter()
            .map(|user| user.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mic_tip = format!("Microphone · {}", names(privacy.microphone()));
    let camera_tip = match names(privacy.camera_users()) {
        users if users.is_empty() => String::from("Camera in use"),
        users => format!("Camera · {users}"),
    };
    drop(privacy);

    let mut widgets = Vec::new();
    if camera {
        widgets.push(boxed(dot(
            "emaki.bar.privacy.camera",
            ICON_CAMERA,
            GREEN,
            camera_tip,
        )));
    }
    if microphone {
        widgets.push(boxed(dot(
            "emaki.bar.privacy.microphone",
            ICON_MIC,
            ORANGE,
            mic_tip,
        )));
    }
    widgets
}

fn dot(id: &'static str, glyph: char, color: Color, tip: String) -> Rectangle {
    let metrics = super::metrics();
    item(DrawnButton::new(id, move |_| {
        Rectangle::new()
            .width(metrics.item_width)
            .height(Size::Parent)
            .align_child(Center, Center)
            .child(icon(glyph, metrics.small_icon, color))
    }))
    .width(metrics.item_width)
    .on_press(privacy_menu)
    .tooltip(BAR_NS, tip)
    .build()
}

/// Which apps use the camera and the microphone; picking one brings its
/// window up.
fn privacy_menu() {
    let privacy = Privacy::read();
    let camera = privacy.camera().then(|| privacy.camera_users().to_vec());
    let microphone = privacy.microphone().to_vec();
    drop(privacy);

    let mut menu = ContextMenu::new();
    let section = |menu: ContextMenu, title: &str, users: &[User], glyph: char| {
        let mut menu = menu.header(title);
        if users.is_empty() {
            menu = menu.item(MenuItem::label("An app").icon(Icon::glyph(glyph.to_string())));
        }
        for user in users {
            let pid = user.pid;
            let mut row = MenuItem::label(user.name.clone()).icon(Icon::glyph(glyph.to_string()));
            if let Some(pid) = pid {
                row = row.on_select(move || focus_pid(pid));
            }
            menu = menu.item(row);
        }
        menu
    };
    let has_camera = camera.is_some();
    if let Some(users) = camera {
        menu = section(menu, "Camera", &users, ICON_CAMERA);
    }
    if !microphone.is_empty() {
        if has_camera {
            menu = menu.separator();
        }
        menu = section(menu, "Microphone", &microphone, ICON_MIC);
    }
    menu = menu
        .separator()
        .item(MenuItem::label("Sound Settings…").on_select(|| quick_settings::open(Page::Sound)));
    context_menu::show_under(PRIVACY_KEY, menu, BAR_NS, (cursor().0 as i32, menu_edge()));
}

/// Brings up the window of process `pid` (or its parent's, for a browser's
/// helper), off the event loop.
fn focus_pid(pid: u64) {
    std::thread::spawn(move || {
        let Ok(windows) = amane::niri::windows() else {
            return;
        };
        let windows: Vec<(u64, Option<i32>)> = windows.iter().map(|w| (w.id, w.pid)).collect();
        let mut wanted = Some(pid);
        // a recording helper is often a child of the app with the window
        for _ in 0..4 {
            let Some(pid) = wanted else { break };
            if let Some(id) = window_for_pid(&windows, pid) {
                if let Err(error) = amane::niri::focus_window(id) {
                    log::warn!("couldn't focus window {id}: {error}");
                }
                return;
            }
            wanted = parent(pid);
        }
    });
}

/// The id of the window process `pid` owns, among windows' ids and processes.
fn window_for_pid(windows: &[(u64, Option<i32>)], pid: u64) -> Option<u64> {
    windows
        .iter()
        .find(|(_, owner)| owner.and_then(|owner| u64::try_from(owner).ok()) == Some(pid))
        .map(|(id, _)| *id)
}

fn parent(pid: u64) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // the name is in parentheses and may hold spaces
    let after = &stat[stat.rfind(')')? + 2..];
    after
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
        .filter(|ppid| *ppid > 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_process_finds_its_window() {
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/niri/windows.json"))
                .expect("niri's windows JSON");
        let windows: Vec<(u64, Option<i32>)> = json
            .as_array()
            .expect("a list")
            .iter()
            .map(|w| {
                let pid = w["pid"].as_i64().and_then(|pid| i32::try_from(pid).ok());
                (w["id"].as_u64().expect("an id"), pid)
            })
            .collect();
        assert_eq!(window_for_pid(&windows, 3100), Some(5));
        assert_eq!(window_for_pid(&windows, 2315), Some(2));
        assert_eq!(window_for_pid(&windows, 1), None);
        assert_eq!(window_for_pid(&[], 3100), None);
    }
}
