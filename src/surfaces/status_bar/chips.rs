//! The bar's live items, as macOS has them: Now Playing while something
//! plays, a purple chip while the screen is shared (with Stop Sharing), and
//! the privacy dots in macOS's colours (orange microphone, green camera),
//! whose menu says which apps and brings their windows up.

use std::cell::Cell;
use std::time::{Duration, Instant};

use amane::{Center, Color, Image, Media, Rectangle, Row, Service, Size, Widget};
use amane_kit::settings::status_bar;

use crate::components::buttons::DrawnButton;
use crate::components::icons::{ICON_CAMERA, ICON_MIC, ICON_MUSIC, ICON_SCREEN_SHARE};
use crate::components::layout::{boxed, cursor, icon, text_width};
use crate::components::material::{self, Level};
use crate::services::desktop::privacy::{CastTarget, Privacy, User, stop_cast};
use crate::surfaces::context_menu::{self, ContextMenu, Icon, MenuItem};
use crate::surfaces::quick_settings::{self, Page};
use crate::tokens::{Styled, bar, sys};

use super::{BAR_NS, item, menu_edge};

/// macOS's privacy colours: the microphone, the camera, the screen.
const ORANGE: Color = Color::rgb(0xff, 0x9f, 0x0a);
const GREEN: Color = Color::rgb(0x30, 0xd1, 0x58);
const PURPLE: Color = Color::rgb(0xbf, 0x5a, 0xf2);

/// How long a paused player keeps its chip.
const PAUSED_FOR: Duration = Duration::from_secs(5 * 60);

/// How wide a chip's title may be.
const TITLE: f32 = 160.0;

const PRIVACY_KEY: &str = "amane.bar.privacy";
const SHARING_KEY: &str = "amane.bar.sharing";
const MEDIA_KEY: &str = "amane.bar.media";

thread_local! {
    // when something last played
    static LAST_PLAYED: Cell<Option<Instant>> = const { Cell::new(None) };
}

// ───── Now Playing ───────────────────────────────────────────────────────────

/// The art and title of what plays, or played in the last few minutes;
/// with `media_always`, a note glyph when nothing does.
pub fn media() -> Option<Box<dyn Widget>> {
    let media = Media::read();
    let playing = media.playing();
    let title = media.title().to_string();
    let artist = media.artist().to_string();
    let art = quick_settings::tiles::media::art_path(media.art_url());
    drop(media);

    let now = Instant::now();
    if playing {
        LAST_PLAYED.set(Some(now));
    }
    let lately = LAST_PLAYED
        .get()
        .is_some_and(|at| now.duration_since(at) < PAUSED_FOR);
    let active = !title.is_empty() && (playing || lately);
    if !active && !status_bar().media_always {
        return None;
    }

    let style = sys::r#type::caption();
    let shown = if active {
        clip(&title, style, TITLE)
    } else {
        String::from("Not Playing")
    };
    let width = 18.0 + 6.0 + text_width(&shown, style) + sys::space::inset::s() * 2.0;
    let tip = match (active, artist.is_empty()) {
        (false, _) => String::from("Now Playing"),
        (true, true) => title.clone(),
        (true, false) => format!("{title} — {artist}"),
    };
    let art = art.filter(|path| Image::loaded(path));

    Some(boxed(
        item(DrawnButton::new("amane.bar.media", move |color| {
            let cover: Box<dyn Widget> = match art {
                Some(path) => boxed(
                    Rectangle::new()
                        .width(Size::Fixed(18.0))
                        .height(Size::Fixed(18.0))
                        .radius(4.0)
                        .fill(Image::cover(path)),
                ),
                None => boxed(icon(ICON_MUSIC, 16.0, color)),
            };
            Rectangle::new()
                .width(Size::Fixed(width))
                .height(Size::Parent)
                .fill(Color::TRANSPARENT)
                .align_child(Center, Center)
                .child(
                    Row::new(vec![
                        cover,
                        boxed(amane::Text::new(shown).style(style).color(if playing {
                            color
                        } else {
                            material::label(Level::Secondary)
                        })),
                    ])
                    .gap(6.0)
                    .align(Center),
                )
        }))
        .width(width)
        .color(material::label(Level::Primary))
        .on_press(|| media_menu())
        .on_middle(Media::play_pause)
        .tooltip(BAR_NS, tip)
        .build(),
    ))
}

/// A little player: what plays, its controls, and Quick Settings.
fn media_menu() {
    let media = Media::read();
    let (title, artist, playing) = (
        media.title().to_string(),
        media.artist().to_string(),
        media.playing(),
    );
    let player = media
        .active()
        .map(|player| player.identity().to_string())
        .unwrap_or_default();
    drop(media);

    let mut menu = ContextMenu::new();
    if title.is_empty() {
        menu = menu.item(MenuItem::label("Not Playing").disabled());
    } else {
        menu = menu.item(MenuItem::label(title).disabled());
        if !artist.is_empty() {
            menu = menu.item(MenuItem::label(artist).disabled());
        }
        menu = menu
            .separator()
            .item(
                MenuItem::label(if playing { "Pause" } else { "Play" })
                    .on_select(Media::play_pause),
            )
            .item(MenuItem::label("Next").on_select(Media::next))
            .item(MenuItem::label("Previous").on_select(Media::previous));
    }
    let more = if player.is_empty() {
        String::from("Quick Settings…")
    } else {
        format!("{player} in Quick Settings…")
    };
    menu = menu
        .separator()
        .item(MenuItem::label(more).on_select(|| quick_settings::open(Page::Grid)));
    context_menu::show_under(MEDIA_KEY, menu, BAR_NS, (cursor().0 as i32, menu_edge()));
}

/// `text` cut with an ellipsis to fit `width`.
fn clip(text: &str, style: crate::tokens::TextStyle, width: f32) -> String {
    if text_width(text, style) <= width {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let cut = format!("{}…", chars.iter().collect::<String>().trim_end());
        if text_width(&cut, style) <= width {
            return cut;
        }
    }
    String::from("…")
}

// ───── privacy ───────────────────────────────────────────────────────────────

/// The screen-sharing chip and the camera and microphone dots, while in
/// use.
pub fn privacy() -> Vec<Box<dyn Widget>> {
    let privacy = Privacy::read();
    let sharing = privacy.screen();
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
    if sharing {
        widgets.push(boxed(sharing_chip()));
    }
    if camera {
        widgets.push(boxed(dot(
            "amane.bar.privacy.camera",
            ICON_CAMERA,
            GREEN,
            camera_tip,
        )));
    }
    if microphone {
        widgets.push(boxed(dot(
            "amane.bar.privacy.microphone",
            ICON_MIC,
            ORANGE,
            mic_tip,
        )));
    }
    widgets
}

fn dot(id: &'static str, glyph: char, color: Color, tip: String) -> Rectangle {
    item(DrawnButton::new(id, move |_| {
        Rectangle::new()
            .width(Size::Fixed(bar::indicator::width()))
            .height(Size::Parent)
            .fill(Color::TRANSPARENT)
            .align_child(Center, Center)
            .child(icon(glyph, sys::size::icon::s(), color))
    }))
    .width(bar::indicator::width())
    .on_press(privacy_menu)
    .tooltip(BAR_NS, tip)
    .build()
}

/// A purple pill while the screen is shared: what's shared, and a click
/// for Stop Sharing.
fn sharing_chip() -> Rectangle {
    let style = sys::r#type::caption();
    let label = "Sharing";
    let width = 16.0 + 6.0 + text_width(label, style) + 20.0;
    item(DrawnButton::new("amane.bar.sharing", move |_| {
        Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(22.0))
            .radius(11.0)
            .fill(PURPLE)
            .align_child(Center, Center)
            .child(
                Row::new(vec![
                    boxed(icon(ICON_SCREEN_SHARE, 16.0, Color::WHITE)),
                    boxed(amane::Text::new(label).style(style).color(Color::WHITE)),
                ])
                .gap(6.0)
                .align(Center),
            )
    }))
    .width(width + 4.0)
    .on_press(sharing_menu)
    .tooltip(BAR_NS, "Your screen is being shared")
    .build()
}

/// What each screencast shares, each with Stop Sharing.
fn sharing_menu() {
    let casts = Privacy::read().casts().to_vec();
    let windows = window_titles();
    let mut menu = ContextMenu::new();
    for (index, cast) in casts.iter().enumerate() {
        if index > 0 {
            menu = menu.separator();
        }
        let what = match &cast.target {
            CastTarget::Output(name) => format!("Screen {name}"),
            CastTarget::Window(id) => windows
                .iter()
                .find(|(window, _)| window == id)
                .map_or_else(|| String::from("A window"), |(_, title)| title.clone()),
            CastTarget::Nothing => String::from("Nothing yet"),
        };
        let session = cast.session;
        menu = menu
            .item(MenuItem::label(format!("Sharing {what}")).disabled())
            .item(
                MenuItem::label("Stop Sharing")
                    .icon(Icon::glyph(ICON_SCREEN_SHARE.to_string()))
                    .destructive()
                    .on_select(move || stop_cast(session)),
            );
    }
    if casts.is_empty() {
        menu = menu.item(MenuItem::label("Nothing is shared").disabled());
    }
    context_menu::show_under(SHARING_KEY, menu, BAR_NS, (cursor().0 as i32, menu_edge()));
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
        let mut menu = menu.item(MenuItem::label(title).disabled());
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

/// Every window by id with its title, from niri.
fn window_titles() -> Vec<(u64, String)> {
    amane::Windows::read()
        .list()
        .iter()
        .map(|window| (window.id(), window.title().unwrap_or_default().to_string()))
        .collect()
}

/// Brings up the window of process `pid` (or its parent's, for a browser's
/// helper), off the event loop.
fn focus_pid(pid: u64) {
    std::thread::spawn(move || {
        let Some(json) =
            crate::services::run_now("niri", &["msg".into(), "--json".into(), "windows".into()])
        else {
            return;
        };
        let Ok(windows) = serde_json::from_str::<serde_json::Value>(&json) else {
            return;
        };
        let mut wanted = Some(pid);
        // a recording helper is often a child of the app with the window
        for _ in 0..4 {
            let Some(pid) = wanted else { break };
            let found = windows.as_array().into_iter().flatten().find_map(|window| {
                (window["pid"].as_u64() == Some(pid))
                    .then(|| window["id"].as_u64())
                    .flatten()
            });
            if let Some(id) = found {
                amane::spawn(&format!("niri msg action focus-window --id {id}"));
                return;
            }
            wanted = parent(pid);
        }
    });
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
