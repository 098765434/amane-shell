//! Now Playing — what the active media player is playing, with previous,
//! play/pause and next, like macOS Control Center's module. (GNOME keeps
//! its media controls in the notification list instead.) As on macOS the
//! module is always there, saying "Not Playing" when nothing is. Two cells
//! square it's the module beside the Wi‑Fi and Bluetooth tiles: art, two lines,
//! the transport along the bottom; a whole row wide they sit side by side.

use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;

use amane::{
    Center, Color, Column, Image, Media, Rectangle, Row, Service, Size, Stack, Start, Widget,
};

use super::super::grid::{Metrics, Span};
use super::super::tile::{line, module};
use crate::components::buttons::{IconButton, expect, optimistic};
use crate::components::icons::{
    ICON_MUSIC, ICON_PAUSE, ICON_PLAY, ICON_SKIP_NEXT, ICON_SKIP_PREVIOUS,
};
use crate::components::layout::{boxed, icon, pad};
use crate::components::material::{self, FillLevel, Level};
use crate::surfaces::quick_settings::QuickSettings;
use crate::tokens::{quick, sys};

/// What the card shows.
struct Playing {
    title: String,
    artist: String,
    art: Option<String>,
    playing: bool,
    /// How far through the track, 0 … 1, when the player says.
    progress: Option<f32>,
}

fn playing() -> Option<Playing> {
    let media = Media::read();
    let player = media.active()?;
    if player.title().is_empty() {
        return None;
    }
    let length = player.length();
    let progress = (length > Duration::ZERO)
        .then(|| (player.position().as_secs_f32() / length.as_secs_f32()).clamp(0.0, 1.0));
    Some(Playing {
        title: player.title().to_string(),
        artist: player.artist().to_string(),
        art: art_path(player.art_url()),
        playing: player.playing(),
        progress,
    })
}

thread_local! {
    // the web art being fetched, so each is asked for once
    static FETCHING: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// The file holding the art at `url`: the file itself for a `file://`
/// URL, else a copy in the cache, fetched in the background the first
/// time (the card redraws when it lands).
fn art_path(url: &str) -> Option<String> {
    if url.is_empty() {
        return None;
    }
    if let Some(path) = url.strip_prefix("file://") {
        return Some(percent_decode(path));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    let path = art_cache(url);
    if path.exists() {
        return Some(path.to_string_lossy().into_owned());
    }
    let fresh = FETCHING.with_borrow_mut(|fetching| fetching.insert(url.to_string()));
    if fresh {
        let url = url.to_string();
        std::thread::spawn(move || {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let part = path.with_extension("part");
            let fetched = std::process::Command::new("curl")
                .args(["-fsSL", "--max-time", "15", "-o"])
                .arg(&part)
                .arg(&url)
                .status()
                .is_ok_and(|status| status.success());
            if fetched && std::fs::rename(&part, &path).is_ok() {
                QuickSettings::changed();
            } else {
                let _ = std::fs::remove_file(&part);
            }
        });
    }
    None
}

/// Where the art at `url` is kept: `$XDG_CACHE_HOME/amane-shell/art/<hash>`.
fn art_cache(url: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("amane-shell")
        .join("art")
        .join(format!("{:016x}", hasher.finish()))
}

/// `%20` and the like in a file URL's path.
fn percent_decode(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&path[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| path.to_string())
}

/// The module, `span` big.
pub(crate) fn cell(span: Span, inert: bool) -> Box<dyn Widget> {
    let now = playing();
    let side = quick::media::art();
    let px = Metrics::tokens();
    let inset = quick::tile::pad();
    let gap = quick::tile::gap();
    let (width, height) = (px.width(span), px.height(span));
    let inner_w = (width - inset * 2.0).max(1.0);
    let square = span.rows > 1;

    let art = match now.as_ref().and_then(|now| now.art.as_deref()) {
        Some(path) if Image::loaded(path) => Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(quick::media::art_radius())
            .fill(Image::cover(path)),
        _ => Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(quick::media::art_radius())
            .fill(sys::color::state::selected())
            .align_child(Center, Center)
            .child(icon(
                ICON_MUSIC,
                sys::size::icon::m(),
                material::label(Level::Primary),
            )),
    };

    let (title, artist, is_playing, idle) = match &now {
        Some(now) => (now.title.clone(), now.artist.clone(), now.playing, false),
        None => (String::from("Not Playing"), String::new(), false, true),
    };
    let progress = now.as_ref().and_then(|now| now.progress);
    let title_color = if idle {
        material::label(Level::Secondary)
    } else {
        material::label(Level::Primary)
    };

    let controls_w = sys::size::control::height() * 3.0 + sys::space::gap::xs() * 2.0;
    let label_w = if square {
        inner_w
    } else {
        inner_w - side - controls_w - gap * 2.0
    }
    .max(1.0);
    let mut lines = vec![boxed(line(
        title,
        quick::r#type::title(),
        title_color,
        label_w,
    ))];
    // a player with no artist leaves no empty line
    if !artist.is_empty() {
        lines.push(boxed(line(
            artist,
            quick::r#type::caption(),
            material::label(Level::Secondary),
            label_w,
        )));
    }
    let labels = Column::new(lines);

    // a click on play shows before the player answers
    let is_playing = optimistic(PLAY_ID, is_playing);
    let play_glyph = if is_playing { ICON_PAUSE } else { ICON_PLAY };
    let controls = Row::new(vec![
        boxed(control(
            "amane.qs.media.previous",
            ICON_SKIP_PREVIOUS,
            sys::size::icon::s(),
            idle,
            inert,
            Media::previous,
        )),
        boxed(control(
            PLAY_ID,
            play_glyph,
            quick::media::play(),
            idle,
            inert,
            move || {
                expect(PLAY_ID, !is_playing);
                Media::play_pause();
            },
        )),
        boxed(control(
            "amane.qs.media.next",
            ICON_SKIP_NEXT,
            sys::size::icon::s(),
            idle,
            inert,
            Media::next,
        )),
    ])
    .gap(sys::space::gap::xs())
    .align(Center);

    let content: Box<dyn Widget> = if square {
        let mut rows: Vec<Box<dyn Widget>> = vec![boxed(art), boxed(labels)];
        // how far through the track, as a thin line under the title
        if let Some(progress) = progress {
            let track_h = sys::size::slider::height() / 2.0;
            rows.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(inner_w))
                    .height(Size::Fixed(track_h))
                    .radius(track_h / 2.0)
                    .fill(material::fill_role(FillLevel::Secondary))
                    .align_child(Start, Center)
                    .child(
                        Rectangle::new()
                            .width(Size::Fixed((inner_w * progress).max(track_h)))
                            .height(Size::Fixed(track_h))
                            .radius(track_h / 2.0)
                            .fill(material::label(Level::Secondary)),
                    ),
            ));
        }
        rows.push(boxed(
            controls
                .width(Size::Fixed(inner_w))
                .justify(amane::SpaceBetween),
        ));
        boxed(
            Column::new(rows)
                .gap(gap)
                .width(Size::Fixed(inner_w))
                .align(Start),
        )
    } else {
        boxed(
            Row::new(vec![boxed(art), boxed(labels), boxed(controls)])
                .gap(gap)
                .width(Size::Fixed(inner_w))
                .align(Center),
        )
    };

    let inner = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .padding(pad(inset, inset))
        .align_child(Start, Center)
        .child(Stack::new(vec![content]));

    boxed(module(width, height, quick::card::radius(), inner))
}

const PLAY_ID: &str = "amane.qs.media.play";

fn control(
    id: &'static str,
    glyph: char,
    size: f32,
    disabled: bool,
    inert: bool,
    press: impl Fn() + 'static,
) -> Rectangle {
    let button = IconButton::new(id, glyph)
        .size(size)
        .color(material::label(Level::Primary))
        .disabled(disabled || inert);
    button.on_press(press).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_url_path_is_decoded() {
        assert_eq!(percent_decode("/a/b%20c/d%C3%A9.png"), "/a/b c/dé.png");
        assert_eq!(percent_decode("/plain.png"), "/plain.png");
        assert_eq!(percent_decode("/bad%2"), "/bad%2");
    }

    #[test]
    fn web_art_is_cached_by_its_url() {
        let a = art_cache("https://x/1.jpg");
        assert_eq!(a, art_cache("https://x/1.jpg"));
        assert_ne!(a, art_cache("https://x/2.jpg"));
        assert!(a.ends_with(a.file_name().unwrap()));
        assert!(a.to_string_lossy().contains("amane-shell/art"));
    }
}
