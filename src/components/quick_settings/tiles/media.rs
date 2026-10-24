//! Now Playing — what the active media player is playing, with previous,
//! play/pause and next, like macOS Control Center's. (GNOME keeps its media
//! controls in the notification list instead.) Shown only while a player
//! has something.

use amane::{Center, Column, Image, Media, Rectangle, Row, Service, Size, Start};

use super::GridCell;
use crate::components::buttons::IconButton;
use crate::components::icons::{
    ICON_MUSIC, ICON_PAUSE, ICON_PLAY, ICON_SKIP_NEXT, ICON_SKIP_PREVIOUS,
};
use crate::components::layout::{boxed, icon, pad, split_row};
use crate::components::quick_settings::grid::Cell;
use crate::components::quick_settings::tile::{QuickToggle, Span, line};
use crate::tokens::{quick, sys};

pub(crate) fn cell() -> Option<GridCell> {
    let media = Media::read();
    let player = media.active()?;
    let title = player.title().to_string();
    let artist = player.artist().to_string();
    let art = player.art_url().to_string();
    let playing = player.playing();
    drop(media);

    if title.is_empty() {
        return None;
    }

    let side = quick::media::art();
    let art_path = art.strip_prefix("file://").map(String::from);
    let art: Rectangle = match art_path {
        Some(path) if Image::loaded(&path) => Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(sys::radius::raised())
            .fill(Image::cover(path)),
        _ => Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(sys::radius::raised())
            .fill(sys::color::state::selected())
            .align_child(Center, Center)
            .child(icon(
                ICON_MUSIC,
                sys::size::icon::m(),
                sys::color::surface::panel::fg(),
            )),
    };

    let buttons = vec![
        boxed(control(
            "amane.qs.media.previous",
            ICON_SKIP_PREVIOUS,
            Media::previous,
        )),
        boxed(control(
            "amane.qs.media.play",
            if playing { ICON_PAUSE } else { ICON_PLAY },
            Media::play_pause,
        )),
        boxed(control("amane.qs.media.next", ICON_SKIP_NEXT, Media::next)),
    ];

    let gap = sys::space::gap::s();
    let controls_w = sys::size::control::height() * 3.0 + gap * 2.0;
    let label_w = (Span::Full.width() - gap * 4.0 - side - controls_w).max(1.0);
    let mut labels = vec![boxed(line(
        title,
        sys::r#type::title(),
        sys::color::surface::panel::fg(),
        label_w,
    ))];
    if !artist.is_empty() {
        labels.push(boxed(line(
            artist,
            sys::r#type::caption(),
            sys::color::content::secondary(),
            label_w,
        )));
    }

    let leading = Row::new(vec![
        boxed(art),
        boxed(Column::new(labels).gap(sys::space::gap::_2xs())),
    ])
    .gap(gap)
    .align(Center);

    let tile = Rectangle::new()
        .width(Size::Fixed(Span::Full.width()))
        .height(Size::Fixed(QuickToggle::height()))
        .radius(quick::tile::radius())
        .fill(sys::color::surface::raised::bg())
        .padding(pad(gap, 0.0))
        .align_child(Start, Center)
        .clip()
        .child(split_row(leading, buttons));

    Some(Cell::Full(boxed(tile), QuickToggle::height()))
}

fn control(id: &'static str, glyph: char, press: fn()) -> Rectangle {
    IconButton::new(id, glyph)
        .color(sys::color::surface::panel::fg())
        .on_press(press)
        .build()
}
