//! The marks edit mode lays over an item on a grid, Quick Settings' controls
//! and the desktop's widgets alike: a remove badge at its top-left corner
//! and, when it comes in more than one size, a resize handle at its
//! bottom-right.

use amane::{Cursor, End, Radius, Rectangle, Start};

use crate::tokens::sys;
use amane_kit::buttons::{IconButton, Shape, Style};
use amane_kit::icons::{ICON_REMOVE, ICON_RESIZE};
use amane_kit::layout::{frame, icon};
use amane_kit::material::{self, FillLevel, Level};

/// The remove badge, `side` across, that `remove`s the item.
pub fn remove_badge(id: String, side: f32, remove: impl Fn() + 'static) -> Rectangle {
    IconButton::new(id, ICON_REMOVE)
        .style(Style::Tonal)
        .shape(Shape::Round)
        .size(sys::size::icon::s())
        .height(side)
        .width(side)
        .color(material::label(Level::Primary))
        .on_press(remove)
        .build()
}

/// The resize handle, `side` across, the pointer showing it pulls.
pub fn resize_handle(side: f32) -> Rectangle {
    Rectangle::new()
        .width(side)
        .height(side)
        .radius(Radius::Full)
        .fill(material::fill_role(FillLevel::Secondary))
        .align_child(amane::Center, amane::Center)
        .cursor(Cursor::ResizeBottomRight)
        .child(icon(
            ICON_RESIZE,
            sys::size::icon::s(),
            material::label(Level::Primary),
        ))
}

/// `badge` at the top-left corner of an item `width` by `height`.
pub fn top_left(width: f32, height: f32, badge: Rectangle) -> Rectangle {
    frame(width, height).align_child(Start, Start).child(badge)
}

/// `handle` at the bottom-right corner of an item `width` by `height`.
pub fn bottom_right(width: f32, height: f32, handle: Rectangle) -> Rectangle {
    frame(width, height).align_child(End, End).child(handle)
}
