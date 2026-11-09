//! The grid: a card that hangs from the bar under Drop's
//! item, its tiles (folders, apps and actions) to let files go on, and the
//! Drop Bar under them to hold files for later. Opened by a click it takes
//! the keys (Escape closes, 1–9 and the arrows pick a grid) and a click
//! outside closes it; opened by files dragged over the item, only its card
//! takes input, and it goes when they leave.
//!
//! It's one clear, full-screen layer surface that frosts only its card
//! (blur shapes), drawn in rectangles, glyphs and arcs: no vello.

use std::path::{Path, PathBuf};
use std::time::Instant;

use amane::{
    Apps, Arc, Button, Canvas, Cap, Center, Color, Column, DragData, FileDialog, Image, InputArea,
    Key, Keyboard, Layer, LayerWindow, Padding, Parent, Rectangle, Row, Service, Shape, Size,
    Stack, Start, Text, TextInput, Widget, WindowSize, Zone, window_size,
};
use emaki_kit::app::{field_box, text_field};
use emaki_kit::buttons::{IconButton, PushButton, Style};
use emaki_kit::icons::{
    ICON_ADD, ICON_CHECK_CIRCLE, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_CLOSE,
    ICON_CLOUD_UPLOAD, ICON_CONTENT_COPY, ICON_DESCRIPTION, ICON_DNS, ICON_ERROR, ICON_IMAGE,
    ICON_PHONE, ICON_TRAY,
};
use emaki_kit::layout::{boxed, frame, icon, text_width};
use emaki_kit::material::{self, FillLevel, Level, Material};
use emaki_kit::settings::{
    DropTile, FolderMode, UploadTo, change_drop_grids, drop_grids, drop_sharing, status_bar,
};
use emaki_kit::tokens::{Styled, sys};
use emaki_phone::KdeConnect;
use emaki_extension::BarEdge;

use crate::state::{self, Changed, Outcome, Prompt, Target};
use crate::tasks::{self, Job, Payload, name};

mod actions;
mod prompt;
mod shelf;
mod tiles;

pub use actions::run_tile;
use actions::*;
use prompt::*;
use shelf::*;
use tiles::*;

/// The grid's layer-surface namespace.
pub const NS: &str = emaki_kit::app_ids::DROP_NS;

/// `folder`, `drive_file_move` and `drive_file_rename_outline`.
const ICON_FOLDER_FILLED: char = '\u{e2c7}';
const ICON_MOVE: char = '\u{e675}';
const ICON_RENAME: char = '\u{e9a2}';

const TILE_W: f32 = 80.0;
const TILE_H: f32 = 84.0;
const GAP: f32 = 8.0;
const PAD: f32 = 16.0;
const HEADER: f32 = 30.0;
const SHELF: f32 = 64.0;
const EDIT_ROW: f32 = 30.0;
const PROMPT: f32 = 132.0;
const RADIUS: f32 = 18.0;

/// The rename prompt's field.
const RENAME: &str = "emaki-drop.rename";

/// The card's width for `columns` tiles a row.
fn card_width(columns: usize) -> f32 {
    let columns = columns.max(1);
    PAD * 2.0 + columns as f32 * TILE_W + (columns as f32 - 1.0) * GAP
}

/// The height of `lines` lines of buttons that add tiles.
fn edit_height(lines: usize) -> f32 {
    let lines = lines as f32;
    lines * EDIT_ROW + (lines - 1.0).max(0.0) * 4.0
}

/// The card's size for `tiles` tiles in `columns`, with `edit_lines` of
/// buttons that add tiles under them while editing.
fn card_size(tiles: usize, columns: usize, edit_lines: usize, asking: bool) -> (f32, f32) {
    let columns = columns.max(1);
    let width = card_width(columns);
    let editing = edit_lines > 0;
    let rows = tiles.div_ceil(columns).max(1) as f32;
    let body = if asking {
        PROMPT
    } else {
        rows * TILE_H
            + (rows - 1.0) * GAP
            + if editing {
                GAP + edit_height(edit_lines)
            } else {
                0.0
            }
    };
    let height = PAD * 2.0 + HEADER + GAP + body + GAP + SHELF;
    (width, height)
}

/// Where the card goes on a screen `screen` big: under the item's middle,
/// on the screen, clear of the bar.
fn card_place(screen: (f32, f32), size: (f32, f32), x: Option<f32>, edge: BarEdge) -> (f32, f32) {
    let margin = 8.0;
    let middle = x.unwrap_or(screen.0 - size.0 / 2.0 - margin);
    let left = (middle - size.0 / 2.0).clamp(margin, (screen.0 - size.0 - margin).max(margin));
    let bar = status_bar().size.height() + 6.0;
    let top = match edge {
        BarEdge::Top => bar,
        BarEdge::Bottom => (screen.1 - bar - size.1).max(margin),
    };
    (left.round(), top.round())
}

/// The grid's window, registered with `App::window`.
pub fn window() -> LayerWindow {
    Changed::watch();
    let grids = drop_grids().clone();
    let (open, by_drag, x, output, edge, editing, asking) = {
        let ui = state::ui();
        (
            ui.open,
            ui.by_drag,
            ui.x,
            ui.output.clone(),
            ui.edge,
            ui.editing,
            ui.prompt.is_some(),
        )
    };
    let index = state::ui().grid.min(grids.grids.len().saturating_sub(1));
    let base = LayerWindow::new()
        .width(WindowSize::Full)
        .height(WindowSize::Full)
        .layer(Layer::Overlay)
        .space(Zone::Ignore)
        .namespace(NS);
    let Some(grid) = grids.grids.get(index).cloned().filter(|_| open) else {
        return base
            .visible(false)
            .keyboard(Keyboard::None)
            .child(emaki_kit::layout::blank());
    };

    let columns = grids.columns();
    let edit_lines = if editing {
        edit_lines(&grid.tiles, card_width(columns) - PAD * 2.0).len()
    } else {
        0
    };
    let size = card_size(grid.tiles.len(), columns, edit_lines, asking);
    let screen = window_size();
    let (left, top) = card_place(screen, size, x, edge);

    let card = card(&grids, index, columns, size);
    let backdrop = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .on_click(|_| state::close())
        .padding(Padding {
            left,
            top,
            ..Padding::default()
        })
        .align_child(Start, Start)
        .child(card);

    let mut window = base
        .visible(true)
        .keyboard(if by_drag {
            Keyboard::None
        } else {
            Keyboard::Exclusive
        })
        .on_key(key)
        .blur_shapes(vec![
            Material::Menu.shape(left, top, size.0, size.1, RADIUS, 1.0),
        ])
        .child(backdrop);
    if by_drag {
        // only the card takes the drag, so it can go on to other programs
        window = window.input_region(vec![InputArea {
            x: left as i32,
            y: top as i32,
            width: size.0 as i32,
            height: size.1 as i32,
        }]);
    }
    if let Some(output) = output {
        window = window.output(Some(output));
    }
    window
}

fn key(key: Key) {
    let asking = state::ui().prompt.is_some();
    match key {
        Key::Escape if asking => state::change(|ui| ui.prompt = None),
        Key::Escape => state::close(),
        _ if asking => {}
        Key::Left => step(-1),
        Key::Right => step(1),
        Key::Character(digit @ '1'..='9') => {
            let index = digit as usize - '1' as usize;
            if index < drop_grids().grids.len() {
                pick(index);
            }
        }
        _ => {}
    }
}

/// Shows grid `index`, and tells the bar's item which it is.
pub fn pick(index: usize) {
    state::change(|ui| {
        ui.grid = index;
        ui.editing = false;
    });
    crate::refresh_item();
}

fn step(by: isize) {
    let count = drop_grids().grids.len();
    if count < 2 {
        return;
    }
    let now = state::ui().grid as isize;
    pick((now + by).rem_euclid(count as isize) as usize);
}

fn card(
    grids: &emaki_kit::settings::DropGrids,
    index: usize,
    columns: usize,
    size: (f32, f32),
) -> Rectangle {
    let grid = &grids.grids[index];
    let (editing, prompt) = {
        let ui = state::ui();
        (ui.editing, ui.prompt.clone())
    };
    let inner = size.0 - PAD * 2.0;

    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(header(grids, index, inner, editing))];
    match prompt {
        Some(prompt) => rows.push(boxed(ask(prompt, inner))),
        None => {
            rows.push(boxed(tiles(&grid.tiles, columns, editing)));
            if editing {
                rows.push(boxed(edit_row(&grid.tiles, inner)));
            }
        }
    }
    rows.push(boxed(shelf(&grid.name, inner)));

    Rectangle::new()
        .width(size.0)
        .height(size.1)
        .radius(RADIUS)
        .fill(Material::Menu.tint())
        .border(sys::border::hairline(), material::separator())
        .padding(Padding {
            left: PAD,
            top: PAD,
            right: PAD,
            bottom: PAD,
        })
        // a click on the card isn't a click outside it
        .on_click(|_| {})
        .on_files_over(|at, _| {
            state::hover(Target::Card, at.is_some());
            false
        })
        .child(Column::new(rows).gap(GAP).width(Parent))
}

fn label(text: &str, style: emaki_kit::tokens::TextStyle, color: Color, width: f32) -> Text {
    Text::new(clip(text, style, width))
        .style(style)
        .color(color)
}

/// The grid's name, with arrows between grids, and Edit or what editing
/// offers.
fn header(
    grids: &emaki_kit::settings::DropGrids,
    index: usize,
    width: f32,
    editing: bool,
) -> Rectangle {
    let count = grids.grids.len();
    let mut left: Vec<Box<dyn Widget>> = Vec::new();
    if count > 1 {
        left.push(boxed(
            IconButton::new("emaki-drop.previous", ICON_CHEVRON_LEFT)
                .on_press(|| step(-1))
                .build(),
        ));
    }
    left.push(boxed(label(
        &grids.grids[index].name,
        sys::r#type::label(),
        material::label(Level::Primary),
        140.0,
    )));
    if count > 1 {
        left.push(boxed(
            IconButton::new("emaki-drop.next", ICON_CHEVRON_RIGHT)
                .on_press(|| step(1))
                .build(),
        ));
    }

    let mut right: Vec<Box<dyn Widget>> = Vec::new();
    if editing {
        right.push(boxed(
            PushButton::new("emaki-drop.add-folder", "Folder…")
                .icon(ICON_ADD)
                .style(Style::Tonal)
                .on_press(add_folder)
                .build(),
        ));
        right.push(boxed(
            PushButton::new("emaki-drop.new-grid", "Grid")
                .icon(ICON_ADD)
                .style(Style::Tonal)
                .on_press(new_grid)
                .build(),
        ));
        if count > 1 {
            right.push(boxed(
                IconButton::new("emaki-drop.delete-grid", ICON_CLOSE)
                    .style(Style::Destructive)
                    .on_press(move || delete_grid(index))
                    .build(),
            ));
        }
    }
    right.push(boxed(
        PushButton::new("emaki-drop.edit", if editing { "Done" } else { "Edit" })
            .style(if editing { Style::Filled } else { Style::Plain })
            .on_press(|| state::change(|ui| ui.editing = !ui.editing))
            .build(),
    ));

    frame(width, HEADER).child(
        Row::new(vec![
            boxed(Row::new(left).gap(2.0).align(Center)),
            boxed(Row::new(right).gap(4.0).align(Center)),
        ])
        .width(Parent)
        .height(Parent)
        .justify(amane::SpaceBetween)
        .align(Center),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_grows_a_row_at_a_time() {
        let (width, one) = card_size(4, 4, 0, false);
        assert_eq!(width, PAD * 2.0 + 4.0 * TILE_W + 3.0 * GAP);
        let (_, two) = card_size(5, 4, 0, false);
        assert_eq!(two - one, TILE_H + GAP);
        let (_, empty) = card_size(0, 4, 0, false);
        assert_eq!(empty, one);
        let (_, editing) = card_size(4, 4, 2, false);
        assert_eq!(editing - one, GAP + 2.0 * EDIT_ROW + 4.0);
    }

    #[test]
    fn buttons_wrap_to_the_width() {
        assert_eq!(wrap(&[50.0, 50.0, 50.0], 160.0, 4.0), [0]);
        assert_eq!(wrap(&[50.0, 50.0, 50.0], 120.0, 4.0), [0, 2]);
        assert_eq!(wrap(&[200.0, 10.0], 120.0, 4.0), [0, 1]);
        assert!(wrap(&[], 100.0, 4.0).is_empty());
    }

    #[test]
    fn the_card_stays_on_screen_and_clear_of_the_bar() {
        let size = (300.0, 200.0);
        let (left, top) = card_place((1600.0, 1000.0), size, Some(1590.0), BarEdge::Top);
        assert_eq!(left, 1600.0 - 300.0 - 8.0);
        assert!(top > 0.0);
        let (left, top) = card_place((1600.0, 1000.0), size, Some(0.0), BarEdge::Bottom);
        assert_eq!(left, 8.0);
        assert!(top + size.1 < 1000.0);
    }
}
