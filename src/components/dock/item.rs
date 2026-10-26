//! One icon in the dock, drawn: a window's app icon with a dot under it,
//! or a pinned app's without, and its name over it while the pointer is on
//! it. All of it is
//! inert: the dock's still layer takes the pointer for every icon (see
//! [`super`]), so nothing here moves a hit area as the icons swell.

use amane::{Center, Color, Rectangle, Row, Service, Text, Widget, Windows};

use super::model::Icon;
use crate::components::app_icon;
use crate::components::layout::{boxed, text_height, text_width};
use crate::components::material::{self, Level, Material};
use crate::tokens::{Styled, sys};

// TODO(token): `dock.running.size` once Penpot has it (manual §1.1).
/// How big a running dot is.
pub const RUNNING: f32 = 4.0;
/// How much of a window's title its name label shows.
const TITLE_CHARS: usize = 48;

/// `icon`'s app icon, `side` big.
pub fn icon(icon: &Icon, side: f32) -> Rectangle {
    app_icon::tile(icon.app_id(), icon.title(), side)
}

/// The dot under a window's icon, brighter while it has the focus; a
/// pinned app that isn't open there has none.
pub fn running(icon: &Icon) -> (Box<dyn Widget>, f32) {
    let count = usize::from(matches!(icon, Icon::Window(_)));
    let color = if icon.focused() {
        material::label(Level::Primary)
    } else {
        material::label(Level::Secondary)
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

/// What an icon is called over it: its window's title, else its desktop
/// entry's name, else its app_id's last part.
pub fn name(icon: &Icon) -> String {
    let title = match icon {
        Icon::Window(window) => Windows::read()
            .list()
            .iter()
            .find(|w| w.id() == window.id)
            .and_then(|w| w.title().map(str::to_string)),
        Icon::Pin(_) => None,
    };
    match title {
        Some(title) if !title.is_empty() => short(&title),
        _ => app_icon::name_for(icon.app_id()).unwrap_or_else(|| icon.app_id().to_string()),
    }
}

/// `title` cut to what a label over an icon has room for.
fn short(title: &str) -> String {
    if title.chars().count() <= TITLE_CHARS {
        return title.to_string();
    }
    let cut: String = title.chars().take(TITLE_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
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
        .fill(Material::Control.fill())
        .align_child(Center, Center)
        .child(
            Text::new(text)
                .style(style)
                .color(material::label(Level::Primary))
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
