//! One app in the dock, drawn: its icon, a dot under it for each window it
//! has open, and its name over it while the pointer is on it. All of it is
//! inert: the dock's still layer takes the pointer for every icon (see
//! [`super`]), so nothing here moves a hit area as the icons swell.

use amane::{Center, Color, Rectangle, Row, Text, Widget};

use super::model::App;
use crate::components::app_icon;
use crate::components::glass_region;
use crate::components::layout::{boxed, text_height, text_width};
use crate::tokens::{Styled, sys};

// TODO(token): `dock.running.size` and `dock.running.max` once Penpot has
// them (manual §1.1).
/// How big a running dot is.
pub const RUNNING: f32 = 4.0;
/// The most dots an app shows, however many windows it has.
const MOST_RUNNING: usize = 3;

/// `app`'s icon, `side` big.
pub fn icon(app: &App, side: f32) -> Rectangle {
    app_icon::tile(&app.app_id, &app.title, side)
}

/// A dot for each of `app`'s windows, up to three, brighter while one of
/// them has the focus.
pub fn running(app: &App) -> (Box<dyn Widget>, f32) {
    let count = app.windows.len().min(MOST_RUNNING);
    let color = if app.focused() {
        sys::color::surface::panel::fg()
    } else {
        sys::color::content::secondary()
    };
    let gap = sys::space::gap::_2xs();
    let dots: Vec<Box<dyn Widget>> = (0..count)
        .map(|_| {
            boxed(
                Rectangle::new()
                    .width(RUNNING)
                    .height(RUNNING)
                    .radius(RUNNING / 2.0)
                    .fill(color),
            )
        })
        .collect();
    let width = RUNNING * count as f32 + gap * count.saturating_sub(1) as f32;
    (boxed(Row::new(dots).gap(gap).align(Center)), width)
}

/// What an app is called over its icon: its desktop entry's name, else
/// its app_id's last part, else its window's title.
pub fn name(app: &App) -> String {
    app_icon::name_for(&app.app_id).unwrap_or_else(|| app.title.clone())
}

/// The name label, and its size: a line of caption text on a small piece
/// of glass, as the dock's own.
pub fn label(text: &str) -> (Rectangle, f32, f32) {
    let style = sys::r#type::caption();
    let (x, y) = (sys::space::inset::s(), sys::space::gap::xs());
    let width = text_width(text, style) + x * 2.0;
    let height = text_height(style) + y * 2.0;
    let label = Rectangle::new()
        .width(width)
        .height(height)
        .radius(sys::radius::raised())
        .fill(glass_region::tint())
        .align_child(Center, Center)
        .child(
            Text::new(text)
                .style(style)
                .color(sys::color::surface::panel::fg())
                .tight(),
        );
    (label, width, height)
}

/// Room for the label over an icon.
pub fn label_height() -> f32 {
    text_height(sys::r#type::caption()) + sys::space::gap::xs() * 2.0
}

/// A clear `width` by `height` frame for absolute placement.
pub fn frame(width: f32, height: f32) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(height)
        .fill(Color::TRANSPARENT)
}
