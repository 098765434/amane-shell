//! Now Playing — what the active media player is playing, with previous,
//! play/pause and next, as a module of its own. (GNOME keeps its media
//! controls in the notification list instead.) The module is always
//! there, saying "Not Playing" when nothing is. Two cells
//! square it's the module beside the Wi‑Fi and Bluetooth tiles: art, two lines,
//! the transport along the bottom; a whole row wide they sit side by side.

use std::cell::Cell;
use std::time::{Duration, Instant};

use amane::{Center, Color, Column, Image, Media, Rectangle, Row, Service, Stack, Start, Widget};

use super::super::grid::{Metrics, Span};
use super::super::tile::{line, module};
use crate::services::media_art;
use crate::surfaces::quick_settings::grid::FromTokens as _;
use crate::tokens::{quick, sys};
use amane_kit::buttons::{IconButton, expect, optimistic};
use amane_kit::icons::{ICON_MUSIC, ICON_PAUSE, ICON_PLAY, ICON_SKIP_NEXT, ICON_SKIP_PREVIOUS};
use amane_kit::layout::{boxed, icon, pad};
use amane_kit::material::{self, FillLevel, Level};

amane_kit::marker! {
    /// A playing track moved on: the progress line draws again. Players
    /// don't say, so the card asks for a frame each second while one plays.
    Progress
}

/// How often the progress line moves on while a track plays.
const PROGRESS_EVERY: Duration = Duration::from_secs(1);

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
    if progress.is_some() && player.playing() {
        progress_in(PROGRESS_EVERY);
    }
    Some(Playing {
        title: player.title().to_string(),
        artist: player.artist().to_string(),
        art: media_art::path(player.art_url()),
        playing: player.playing(),
        progress,
    })
}

thread_local! {
    // when the next progress frame is asked for, so a frame drawn early asks once
    static PROGRESS_DUE: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Asks for a frame `wait` from now, unless one is already asked for; the
/// card's window reads the marker, so only it draws again.
fn progress_in(wait: Duration) {
    drop(Progress::read());
    let now = Instant::now();
    if PROGRESS_DUE.get().is_some_and(|due| due > now) {
        return;
    }
    PROGRESS_DUE.set(Some(now + wait));
    amane_kit::motion::redraw_after::<Progress>(wait);
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
            .width(side)
            .height(side)
            .radius(quick::media::art_radius())
            .fill(Image::cover(path)),
        _ => Rectangle::new()
            .width(side)
            .height(side)
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
                    .width(inner_w)
                    .height(track_h)
                    .radius(track_h / 2.0)
                    .fill(material::fill_role(FillLevel::Secondary))
                    .align_child(Start, Center)
                    .child(
                        Rectangle::new()
                            .width((inner_w * progress).max(track_h))
                            .height(track_h)
                            .radius(track_h / 2.0)
                            .fill(material::label(Level::Secondary)),
                    ),
            ));
        }
        rows.push(boxed(controls.width(inner_w).justify(amane::SpaceBetween)));
        boxed(Column::new(rows).gap(gap).width(inner_w).align(Start))
    } else {
        boxed(
            Row::new(vec![boxed(art), boxed(labels), boxed(controls)])
                .gap(gap)
                .width(inner_w)
                .align(Center),
        )
    };

    let inner = Rectangle::new()
        .width(width)
        .height(height)
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
