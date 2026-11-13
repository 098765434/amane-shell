//! The small pictures the pickers show: a desktop in miniature (a
//! wallpaper, the bar, a window, the dock) drawn the way each choice would
//! draw it. Flat shapes only, so they stay on amane's quads.

use amane::{Color, Column, Rectangle, Stack, Widget};
use emaki_kit::color::with_alpha;
use emaki_kit::layout::{boxed, frame};
use emaki_kit::settings::{BarPosition, BarSize, BarStyle, WidgetStyle};
use emaki_kit::theme::{Glass, Mode};

/// Every picture's size.
pub const WIDTH: f32 = 96.0;
pub const HEIGHT: f32 = 62.0;

/// The wallpaper's sky and its hills.
const SKY: &str = "#5b8fd1";
const HILLS: &str = "#2f5f8f";
const FAR_HILLS: &str = "#4675a9";

/// Something at `x`, `y`, `w` × `h` in a picture.
fn at(x: f32, y: f32, shape: Rectangle) -> Box<dyn Widget> {
    boxed(
        frame(WIDTH, HEIGHT)
            .padding(amane::Padding {
                left: x,
                top: y,
                ..amane::Padding::default()
            })
            .align_child(amane::Start, amane::Start)
            .child(shape),
    )
}

fn shape(w: f32, h: f32, radius: f32, fill: Color) -> Rectangle {
    Rectangle::new()
        .width(w)
        .height(h)
        .radius(radius)
        .fill(fill)
}

/// The wallpaper: a sky over two rows of hills.
fn wallpaper() -> Vec<Box<dyn Widget>> {
    vec![
        boxed(shape(WIDTH, HEIGHT, 0.0, Color::from(SKY))),
        at(-20.0, 30.0, shape(80.0, 60.0, 30.0, Color::from(FAR_HILLS))),
        at(40.0, 36.0, shape(90.0, 60.0, 30.0, Color::from(HILLS))),
    ]
}

fn picture(layers: Vec<Box<dyn Widget>>) -> Rectangle {
    Rectangle::new()
        .width(WIDTH)
        .height(HEIGHT)
        .clip()
        .child(Stack::new(layers))
}

/// A desktop in `dark` or light, or `black` (dark with black surfaces over a
/// dimmed wallpaper): the bar, a window, the dock.
fn desktop(dark: bool, black: bool) -> Rectangle {
    let (bar, window, title, line, dock) = if black {
        (
            Color::BLACK,
            Color::BLACK,
            Color::from("#141414"),
            Color::from("#4a4a4e"),
            Color::BLACK,
        )
    } else if dark {
        (
            with_alpha(Color::from("#1c1c1e"), 0.85),
            Color::from("#2c2c2e"),
            Color::from("#3a3a3c"),
            Color::from("#5a5a5e"),
            with_alpha(Color::from("#1c1c1e"), 0.7),
        )
    } else {
        (
            with_alpha(Color::WHITE, 0.8),
            Color::from("#f5f5f7"),
            Color::from("#e5e5ea"),
            Color::from("#c7c7cc"),
            with_alpha(Color::WHITE, 0.6),
        )
    };
    let mut layers = wallpaper();
    if black {
        layers.push(boxed(shape(
            WIDTH,
            HEIGHT,
            0.0,
            with_alpha(Color::BLACK, 0.75),
        )));
    }
    layers.push(at(0.0, 0.0, shape(WIDTH, 6.0, 0.0, bar)));
    let mut body = shape(60.0, 36.0, 4.0, window);
    if black {
        body = body.border(1.0, with_alpha(Color::WHITE, 0.12));
    }
    layers.push(at(18.0, 13.0, body));
    layers.push(at(18.0, 13.0, shape(60.0, 7.0, 0.0, title).radius(4.0)));
    for (index, red) in ["#ff5f57", "#febc2e", "#28c840"].into_iter().enumerate() {
        layers.push(at(
            21.0 + index as f32 * 4.0,
            15.0,
            shape(2.5, 2.5, 1.25, Color::from(red)),
        ));
    }
    for (row, width) in [(25.0, 40.0), (30.0, 32.0), (35.0, 44.0)] {
        layers.push(at(24.0, row, shape(width, 2.0, 1.0, line)));
    }
    layers.push(at(32.0, 54.0, shape(32.0, 6.0, 3.0, dock)));
    picture(layers)
}

/// Light, dark (`black` for black surfaces), or half of each for following
/// the system.
pub fn appearance(mode: Option<Mode>, black: bool) -> Rectangle {
    match mode {
        Some(Mode::Light) => desktop(false, false),
        Some(Mode::Dark) => desktop(true, black),
        None => {
            let half = |dark: bool, side: amane::Align| -> Box<dyn Widget> {
                boxed(
                    Rectangle::new()
                        .width(WIDTH / 2.0)
                        .height(HEIGHT)
                        .clip()
                        .align_child(side, amane::Start)
                        .child(desktop(dark, dark && black)),
                )
            };
            Rectangle::new()
                .width(WIDTH)
                .height(HEIGHT)
                .clip()
                .child(amane::Row::new(vec![
                    half(false, amane::Start.into()),
                    half(true, amane::End.into()),
                ]))
        }
    }
}

/// The current mode's panel colour, for glass drawn solid.
fn panel(dark: bool) -> Color {
    if dark {
        Color::from("#2c2c2e")
    } else {
        Color::from("#f5f5f7")
    }
}

fn dark_now() -> bool {
    emaki_kit::theme::mode() == Mode::Dark
}

/// A panel of glass over the wallpaper, as clear, tinted or solid.
pub fn glass(glass: Glass) -> Rectangle {
    let dark = dark_now();
    let (fill, edge) = match glass {
        Glass::Clear => (
            with_alpha(Color::WHITE, 0.16),
            with_alpha(Color::WHITE, 0.55),
        ),
        Glass::Tinted => (with_alpha(panel(dark), 0.6), with_alpha(Color::WHITE, 0.3)),
        Glass::Solid => (panel(dark), Color::TRANSPARENT),
    };
    let ink = if dark || glass == Glass::Clear {
        with_alpha(Color::WHITE, 0.85)
    } else {
        with_alpha(Color::BLACK, 0.7)
    };
    let mut layers = wallpaper();
    layers.push(at(
        20.0,
        12.0,
        shape(56.0, 38.0, 8.0, fill).border(1.0, edge),
    ));
    for (index, row) in [20.0, 28.0, 36.0].into_iter().enumerate() {
        layers.push(at(28.0, row, shape(14.0, 5.0, 2.5, ink)));
        layers.push(at(
            46.0,
            row + 1.0,
            shape(20.0 - index as f32 * 4.0, 3.0, 1.5, with_alpha(ink, 0.6)),
        ));
    }
    picture(layers)
}

/// The bar at `position`, `size` tall, in `style`, with a few items in it.
pub fn bar(style: BarStyle, size: BarSize, position: BarPosition) -> Rectangle {
    let dark = dark_now();
    let strip = match style {
        BarStyle::Clear => None,
        BarStyle::Frosted => Some(with_alpha(panel(dark), 0.55)),
        BarStyle::Glass => Some(with_alpha(Color::WHITE, 0.18)),
        BarStyle::Solid => Some(if dark { Color::BLACK } else { Color::WHITE }),
    };
    let ink = match style {
        BarStyle::Solid if !dark => with_alpha(Color::BLACK, 0.75),
        BarStyle::Frosted if !dark => with_alpha(Color::BLACK, 0.7),
        _ => with_alpha(Color::WHITE, 0.9),
    };
    // the compact bar is 12 tall here, the others as much taller as they are
    let height = (12.0 * size.height() / BarSize::Compact.height()).round();
    let grow = if size == BarSize::Comfortable {
        1.5
    } else {
        1.0
    };
    let top = match position {
        BarPosition::Top => 0.0,
        BarPosition::Bottom => HEIGHT - height,
    };
    let middle = |tall: f32| top + (height - tall) / 2.0;
    let mut layers = wallpaper();
    if let Some(strip) = strip {
        let mut bar = shape(WIDTH, height, 0.0, strip);
        if style == BarStyle::Glass {
            bar = bar.border(1.0, with_alpha(Color::WHITE, 0.4));
        }
        layers.push(at(0.0, top, bar));
    }
    // the workspace's shape at the left, icons and the clock at the right
    layers.push(at(5.0, middle(5.0), shape(5.0, 5.0, 2.5, ink)));
    let icon = 4.0 * grow;
    for index in 0..3 {
        layers.push(at(
            52.0 + index as f32 * 7.0 * grow - if grow > 1.0 { 10.0 } else { 0.0 },
            middle(icon),
            shape(icon, icon, icon / 2.0, ink),
        ));
    }
    layers.push(at(74.0, middle(3.0), shape(17.0, 3.0, 1.5, ink)));
    picture(layers)
}

/// Widgets on the desktop, at full strength or dimmed behind a window.
pub fn widgets(look: WidgetStyle) -> Rectangle {
    let dark = dark_now();
    let card = with_alpha(panel(dark), 0.75);
    let dim = match look {
        WidgetStyle::Full => 1.0,
        WidgetStyle::Dim => 0.45,
        WidgetStyle::Automatic => 0.45,
    };
    let mut layers = wallpaper();
    for (x, y, w, h) in [
        (6.0, 8.0, 26.0, 26.0),
        (6.0, 38.0, 26.0, 18.0),
        (36.0, 8.0, 18.0, 18.0),
    ] {
        layers.push(at(x, y, shape(w, h, 5.0, card).opacity(dim)));
    }
    if look == WidgetStyle::Automatic {
        // dimmed because a window is open
        layers.push(at(46.0, 20.0, shape(44.0, 34.0, 4.0, panel(dark))));
        layers.push(at(
            46.0,
            20.0,
            shape(44.0, 6.0, 0.0, panel(!dark)).radius(4.0).opacity(0.2),
        ));
    }
    picture(layers)
}

/// The launcher's apps as a grid of icons or a list of rows.
pub fn launcher(grid: bool) -> Rectangle {
    let dark = dark_now();
    let ink = if dark {
        with_alpha(Color::WHITE, 0.8)
    } else {
        with_alpha(Color::BLACK, 0.6)
    };
    let mut layers = wallpaper();
    layers.push(at(
        14.0,
        8.0,
        shape(68.0, 46.0, 8.0, with_alpha(panel(dark), 0.85)),
    ));
    layers.push(at(20.0, 13.0, shape(56.0, 6.0, 3.0, with_alpha(ink, 0.25))));
    let tiles = [
        "#ff9230", "#30d158", "#0091ff", "#db34f2", "#ffd600", "#ff4245",
    ];
    if grid {
        for (index, tile) in tiles.into_iter().enumerate() {
            let (col, row) = (index % 3, index / 3);
            layers.push(at(
                26.0 + col as f32 * 16.0,
                24.0 + row as f32 * 14.0,
                shape(10.0, 10.0, 3.0, Color::from(tile)),
            ));
        }
    } else {
        for (index, tile) in tiles.into_iter().take(4).enumerate() {
            let row = 23.0 + index as f32 * 7.5;
            layers.push(at(21.0, row, shape(5.5, 5.5, 1.5, Color::from(tile))));
            layers.push(at(
                30.0,
                row + 1.5,
                shape(30.0 - index as f32 * 3.0, 2.5, 1.25, ink),
            ));
        }
    }
    picture(layers)
}

/// A clock as the bar shows it: digits, or a small face with hands.
pub fn clock(analog: bool) -> Rectangle {
    let dark = dark_now();
    let ink = if dark {
        with_alpha(Color::WHITE, 0.9)
    } else {
        with_alpha(Color::BLACK, 0.75)
    };
    let mut layers: Vec<Box<dyn Widget>> = vec![boxed(shape(
        WIDTH,
        HEIGHT,
        0.0,
        if dark {
            Color::from("#1c1c1e")
        } else {
            Color::from("#f2f2f7")
        },
    ))];
    if analog {
        const FACE: f32 = 34.0;
        let (x, y) = ((WIDTH - FACE) / 2.0, (HEIGHT - FACE) / 2.0);
        layers.push(at(
            x,
            y,
            shape(FACE, FACE, FACE / 2.0, Color::TRANSPARENT).border(2.0, ink),
        ));
        let hand = |length: f32, degrees: f32| -> Box<dyn Widget> {
            at(
                x,
                y,
                frame(FACE, FACE)
                    .rotate(degrees)
                    .align_child(amane::Center, amane::Start)
                    .padding(amane::Padding {
                        top: FACE / 2.0 - length,
                        ..amane::Padding::default()
                    })
                    .child(shape(2.0, length, 1.0, ink)),
            )
        };
        layers.push(hand(9.0, 300.0));
        layers.push(hand(13.0, 60.0));
    } else {
        layers.push(boxed(
            frame(WIDTH, HEIGHT)
                .align_child(amane::Center, amane::Center)
                .child(Column::new(vec![boxed({
                    use emaki_kit::tokens::Styled;
                    amane::Text::new("9:41")
                        .style(emaki_kit::style::page_title_font())
                        .color(ink)
                })])),
        ));
    }
    picture(layers)
}
