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
use amane_kit::app::{field_box, text_field};
use amane_kit::buttons::{IconButton, PushButton, Style};
use amane_kit::icons::{
    ICON_ADD, ICON_CHECK_CIRCLE, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_CLOSE,
    ICON_CLOUD_UPLOAD, ICON_CONTENT_COPY, ICON_DESCRIPTION, ICON_DNS, ICON_ERROR, ICON_IMAGE,
    ICON_PHONE, ICON_TRAY,
};
use amane_kit::layout::{boxed, icon, text_width};
use amane_kit::material::{self, FillLevel, Level, Material};
use amane_kit::settings::{
    DropTile, FolderMode, UploadTo, change_drop_grids, drop_grids, drop_sharing, status_bar,
};
use amane_kit::tokens::{Styled, sys};
use amane_phone::KdeConnect;
use amane_status::BarEdge;

use crate::state::{self, Changed, Outcome, Prompt, Target};
use crate::tasks::{self, Job, Payload, name};

/// The grid's layer-surface namespace.
pub const NS: &str = amane_kit::app_ids::DROP_NS;

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
const RENAME: &str = "amane-drop.rename";

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
            .child(amane_kit::layout::blank());
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
        .fill(Color::TRANSPARENT)
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
    grids: &amane_kit::settings::DropGrids,
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
        .width(Size::Fixed(size.0))
        .height(Size::Fixed(size.1))
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

fn label(text: &str, style: amane_kit::tokens::TextStyle, color: Color, width: f32) -> Text {
    Text::new(clip(text, style, width))
        .style(style)
        .color(color)
}

/// The grid's name, with arrows between grids, and Edit or what editing
/// offers.
fn header(
    grids: &amane_kit::settings::DropGrids,
    index: usize,
    width: f32,
    editing: bool,
) -> Rectangle {
    let count = grids.grids.len();
    let mut left: Vec<Box<dyn Widget>> = Vec::new();
    if count > 1 {
        left.push(boxed(
            IconButton::new("amane-drop.previous", ICON_CHEVRON_LEFT)
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
            IconButton::new("amane-drop.next", ICON_CHEVRON_RIGHT)
                .on_press(|| step(1))
                .build(),
        ));
    }

    let mut right: Vec<Box<dyn Widget>> = Vec::new();
    if editing {
        right.push(boxed(
            PushButton::new("amane-drop.add-folder", "Folder…")
                .icon(ICON_ADD)
                .style(Style::Tonal)
                .on_press(add_folder)
                .build(),
        ));
        right.push(boxed(
            PushButton::new("amane-drop.new-grid", "Grid")
                .icon(ICON_ADD)
                .style(Style::Tonal)
                .on_press(new_grid)
                .build(),
        ));
        if count > 1 {
            right.push(boxed(
                IconButton::new("amane-drop.delete-grid", ICON_CLOSE)
                    .style(Style::Destructive)
                    .on_press(move || delete_grid(index))
                    .build(),
            ));
        }
    }
    right.push(boxed(
        PushButton::new("amane-drop.edit", if editing { "Done" } else { "Edit" })
            .style(if editing { Style::Filled } else { Style::Plain })
            .on_press(|| state::change(|ui| ui.editing = !ui.editing))
            .build(),
    ));

    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(HEADER))
        .fill(Color::TRANSPARENT)
        .child(
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

// ───── tiles ─────────────────────────────────────────────────────────────────

fn tiles(tiles: &[DropTile], columns: usize, editing: bool) -> Rectangle {
    let rows: Vec<Box<dyn Widget>> = tiles
        .chunks(columns)
        .enumerate()
        .map(|(row, chunk)| {
            let cells: Vec<Box<dyn Widget>> = chunk
                .iter()
                .enumerate()
                .map(|(column, tile)| boxed(tile_cell(row * columns + column, tile, editing)))
                .collect();
            boxed(Row::new(cells).gap(GAP))
        })
        .collect();
    let rows_count = tiles.len().div_ceil(columns).max(1) as f32;
    let hovered = state::ui().hover == Some(Target::Gap);

    // the gap between tiles takes folders and apps as new tiles
    Rectangle::new()
        .width(Parent)
        .height(Size::Fixed(rows_count * TILE_H + (rows_count - 1.0) * GAP))
        .radius(12.0)
        .fill(if hovered {
            material::fill_role(FillLevel::Tertiary)
        } else {
            Color::TRANSPARENT
        })
        .on_files_over(|at, files| {
            let takes = files.iter().any(|file| becomes_tile(file));
            state::hover(Target::Gap, at.is_some() && takes);
            takes
        })
        .on_drop(add_tiles)
        .align_child(Start, Start)
        .child(Column::new(rows).gap(GAP))
}

/// Whether a file let go between the tiles becomes one: a folder or an app.
fn becomes_tile(file: &Path) -> bool {
    file.is_dir()
        || file
            .extension()
            .is_some_and(|extension| extension == "desktop")
}

fn add_tiles(files: Vec<PathBuf>) {
    let grid = state::ui().grid;
    let new: Vec<DropTile> = files
        .into_iter()
        .filter(|file| becomes_tile(file))
        .map(|file| {
            if file.is_dir() {
                DropTile::Folder {
                    path: file,
                    mode: FolderMode::Copy,
                }
            } else {
                DropTile::App { id: name(&file) }
            }
        })
        .collect();
    change_drop_grids(|grids| {
        if let Some(grid) = grids.grids.get_mut(grid) {
            grid.tiles.extend(new);
        }
    });
    state::hover(Target::Gap, false);
}

/// A tile's picture and name.
fn look(tile: &DropTile) -> (Picture, String) {
    let folder_name = |folder: &Option<PathBuf>, what: &str| match folder {
        Some(folder) => format!("{what} {}", name(folder)),
        None => format!("{what}…"),
    };
    match tile {
        DropTile::Folder { path, .. } => (Picture::Glyph(ICON_FOLDER_FILLED), name(path)),
        DropTile::App { id } => {
            let apps = Apps::read();
            let app = apps.list().iter().find(|app| app.id() == id);
            let title = app.map_or_else(
                || id.trim_end_matches(".desktop").to_string(),
                |app| app.name().to_string(),
            );
            let picture = app
                .and_then(|app| app.icon_path())
                .map(Path::to_path_buf)
                .filter(|path| Image::loaded(path))
                .map_or(Picture::Glyph(ICON_TRAY), Picture::File);
            (picture, title)
        }
        DropTile::CopyTo { folder } => (
            Picture::Glyph(ICON_CONTENT_COPY),
            folder_name(folder, "Copy to"),
        ),
        DropTile::MoveTo { folder } => (Picture::Glyph(ICON_MOVE), folder_name(folder, "Move to")),
        DropTile::RenameMove { .. } => (Picture::Glyph(ICON_RENAME), String::from("Rename & Move")),
        DropTile::SaveText { .. } => (Picture::Glyph(ICON_DESCRIPTION), String::from("Save Text")),
        DropTile::Phone { device } => {
            let named = device.as_deref().and_then(|id| {
                KdeConnect::read()
                    .device(id)
                    .map(|device| device.name().to_string())
            });
            (
                Picture::Glyph(ICON_PHONE),
                named.unwrap_or_else(|| String::from("Send to Phone")),
            )
        }
        DropTile::Upload { to } => (Picture::Glyph(upload_glyph(*to)), to.title().to_string()),
    }
}

fn upload_glyph(to: UploadTo) -> char {
    match to {
        UploadTo::ZeroX0 | UploadTo::S3 => ICON_CLOUD_UPLOAD,
        UploadTo::Imgur => ICON_IMAGE,
        UploadTo::Sftp => ICON_DNS,
    }
}

/// Whether a tile takes text let go on it.
fn takes_text(tile: &DropTile) -> bool {
    matches!(
        tile,
        DropTile::SaveText { .. } | DropTile::Phone { .. } | DropTile::Upload { .. }
    )
}

enum Picture {
    Glyph(char),
    File(PathBuf),
}

fn tile_cell(index: usize, tile: &DropTile, editing: bool) -> Rectangle {
    let (picture, title) = look(tile);
    let (hovered, running, outcome) = {
        let ui = state::ui();
        let outcome = ui
            .outcomes
            .get(&index)
            .filter(|(_, until)| *until > Instant::now())
            .map(|(outcome, _)| *outcome);
        (
            ui.hover == Some(Target::Tile(index)),
            ui.running.get(&index).copied(),
            outcome,
        )
    };
    let primary = material::label(Level::Primary);
    let accent = sys::color::accent::bg();

    let picture: Box<dyn Widget> = match (running, outcome, picture) {
        (Some(done), ..) => boxed(ring(done.unwrap_or(0.05), 36.0, accent)),
        (None, Some(Outcome::Done), _) => boxed(icon(
            ICON_CHECK_CIRCLE,
            34.0,
            amane_kit::color::Accent::Green.color(),
        )),
        (None, Some(Outcome::Failed), _) => boxed(icon(
            ICON_ERROR,
            34.0,
            amane_kit::color::Accent::Red.color(),
        )),
        (None, None, Picture::Glyph(glyph)) => boxed(icon(glyph, 34.0, accent)),
        (None, None, Picture::File(path)) => boxed(
            Rectangle::new()
                .width(Size::Fixed(36.0))
                .height(Size::Fixed(36.0))
                .fill(Image::contain(path)),
        ),
    };
    let mut parts: Vec<Box<dyn Widget>> = vec![
        picture,
        boxed(label(&title, sys::r#type::caption(), primary, TILE_W - 8.0)),
    ];
    if editing {
        let note = match tile {
            DropTile::Folder {
                mode: FolderMode::Move,
                ..
            } => Some("Moves"),
            DropTile::Folder { .. } => Some("Copies"),
            DropTile::CopyTo { .. }
            | DropTile::MoveTo { .. }
            | DropTile::RenameMove { .. }
            | DropTile::SaveText { .. } => Some("Choose…"),
            DropTile::App { .. } => None,
            DropTile::Phone { device: None } => Some("Asks"),
            DropTile::Phone { .. } => Some("Next phone"),
            DropTile::Upload { to } => Some(if drop_sharing().missing(*to).is_some() {
                "Set up…"
            } else {
                "Settings…"
            }),
        };
        if let Some(note) = note {
            parts.push(boxed(label(
                note,
                sys::r#type::caption(),
                accent,
                TILE_W - 8.0,
            )));
        }
    }

    let body = Rectangle::new()
        .width(Size::Fixed(TILE_W))
        .height(Size::Fixed(TILE_H))
        .radius(12.0)
        .fill(if hovered {
            with_alpha(accent, 0.25)
        } else {
            material::fill_role(FillLevel::Tertiary)
        })
        .border(if hovered { 2.0 } else { 0.0 }, accent)
        .align_child(Center, Center)
        .child(Column::new(parts).gap(4.0).align(Center));

    let (pressed, dropped, texted, over) = (tile.clone(), tile.clone(), tile.clone(), tile.clone());
    let mut cell = body
        .on_click(move |button| {
            if button == Button::Left {
                clicked(index, &pressed, editing);
            }
        })
        .on_files_over(move |at, _| {
            let takes = !matches!(over, DropTile::SaveText { .. });
            state::hover(Target::Tile(index), at.is_some() && takes);
            takes
        })
        .on_drop(move |files| {
            state::hover(Target::Tile(index), false);
            dropped_on(index, &dropped, files);
        });
    if takes_text(tile) {
        cell = cell.on_text_drop(move |text| text_dropped(index, &texted, text));
    }
    if !editing {
        return cell;
    }
    // a remove button in its corner
    let remove = Rectangle::new()
        .width(Size::Fixed(20.0))
        .height(Size::Fixed(20.0))
        .radius(10.0)
        .fill(material::fill_role(FillLevel::Primary))
        .align_child(Center, Center)
        .on_click(move |_| remove_tile(index))
        .child(icon(ICON_CLOSE, 14.0, primary));
    Rectangle::new()
        .width(Size::Fixed(TILE_W))
        .height(Size::Fixed(TILE_H))
        .fill(Color::TRANSPARENT)
        .child(Stack::new(vec![
            boxed(cell),
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(TILE_W))
                    .height(Size::Fixed(24.0))
                    .fill(Color::TRANSPARENT)
                    .align_child(amane::End, Start)
                    .child(remove),
            ),
        ]))
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    amane_kit::color::with_alpha(color, alpha)
}

/// A ring `diameter` across, filled `done` of the way round from the top.
fn ring(done: f32, diameter: f32, color: Color) -> Canvas {
    let track = material::fill_role(FillLevel::Secondary);
    let mut shapes: Vec<Box<dyn Shape>> = vec![Box::new(Arc::new().stroke(3.0, track))];
    let sweep = 360.0 * done.clamp(0.0, 1.0);
    if sweep > 0.0 {
        shapes.push(Box::new(
            Arc::new()
                .sweep(sweep.max(1.0))
                .stroke(3.0, color)
                .cap(Cap::Round),
        ));
    }
    Canvas::new()
        .width(Size::Fixed(diameter))
        .height(Size::Fixed(diameter))
        .shapes(shapes)
}

/// `text` cut with an ellipsis to fit `width`.
fn clip(text: &str, style: amane_kit::tokens::TextStyle, width: f32) -> String {
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

// ───── what tiles do ─────────────────────────────────────────────────────────

fn clicked(index: usize, tile: &DropTile, editing: bool) {
    if editing {
        match tile {
            DropTile::Folder { .. } => edit_tile(index, |tile| {
                if let DropTile::Folder { mode, .. } = tile {
                    *mode = match mode {
                        FolderMode::Copy => FolderMode::Move,
                        FolderMode::Move => FolderMode::Copy,
                    };
                }
            }),
            DropTile::App { .. } => {}
            DropTile::Phone { device } => {
                let next = next_phone(device.as_deref());
                edit_tile(index, move |tile| {
                    if let DropTile::Phone { device } = tile {
                        *device = next;
                    }
                });
            }
            DropTile::Upload { .. } => open_settings(),
            _ => choose_folder(index, |_| {}),
        }
        return;
    }
    match tile {
        DropTile::Folder { path, .. } => open_folder(path),
        DropTile::App { id } => {
            if let Some(app) = Apps::read().list().iter().find(|app| app.id() == id) {
                app.launch_with(&[]);
            }
            state::close();
        }
        DropTile::CopyTo { folder }
        | DropTile::MoveTo { folder }
        | DropTile::RenameMove { folder }
        | DropTile::SaveText { folder } => match folder {
            Some(folder) => open_folder(folder),
            None => choose_folder(index, |_| {}),
        },
        DropTile::Phone { .. } => {}
        DropTile::Upload { to } => {
            if drop_sharing().missing(*to).is_some() {
                open_settings();
            }
        }
    }
}

/// Shell Settings, on Drop's page.
fn open_settings() {
    amane::spawn("amane-settings --page drop");
    state::close();
}

/// The paired phone after `now` in the list, then back to asking.
fn next_phone(now: Option<&str>) -> Option<String> {
    let ids: Vec<String> = KdeConnect::read()
        .devices()
        .iter()
        .filter(|device| device.paired())
        .map(|device| device.id().to_string())
        .collect();
    match now.and_then(|now| ids.iter().position(|id| id == now)) {
        Some(at) => ids.get(at + 1).cloned(),
        None => ids.first().cloned(),
    }
}

fn open_folder(folder: &Path) {
    amane::spawn(&format!(
        "xdg-open '{}'",
        folder.display().to_string().replace('\'', "'\\''")
    ));
    state::close();
}

/// Tile `index` of the grid showing does with `files` what a drop on it
/// would.
pub fn run_tile(index: usize, files: Vec<PathBuf>) {
    let grid = state::ui().grid;
    let tile = drop_grids()
        .grids
        .get(grid)
        .and_then(|grid| grid.tiles.get(index))
        .cloned();
    if let Some(tile) = tile {
        dropped_on(index, &tile, files);
    }
}

fn dropped_on(index: usize, tile: &DropTile, files: Vec<PathBuf>) {
    match tile.clone() {
        DropTile::Folder { path, mode } => tasks::start(match mode {
            FolderMode::Copy => Job::Copy {
                files,
                folder: path,
                tile: index,
            },
            FolderMode::Move => Job::Move {
                files,
                folder: path,
                tile: index,
            },
        }),
        DropTile::App { id } => {
            if let Some(app) = Apps::read().list().iter().find(|app| app.id() == id) {
                app.launch_with(&files);
            }
            state::close();
        }
        DropTile::CopyTo { folder } => with_folder(index, folder, move |folder| {
            tasks::start(Job::Copy {
                files,
                folder,
                tile: index,
            });
        }),
        DropTile::MoveTo { folder } => with_folder(index, folder, move |folder| {
            tasks::start(Job::Move {
                files,
                folder,
                tile: index,
            });
        }),
        DropTile::RenameMove { folder } => with_folder(index, folder, move |folder| {
            let first = files.first().map(|file| {
                file.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
            TextInput::set_text(RENAME, &first.unwrap_or_default());
            state::ask(Prompt::Rename {
                files,
                folder,
                tile: index,
            });
        }),
        DropTile::SaveText { .. } => {}
        DropTile::Phone { device } => to_phone(index, device, Payload::Files(files)),
        DropTile::Upload { to } => tasks::start(Job::Upload {
            what: Payload::Files(files),
            to,
            tile: index,
        }),
    }
}

fn text_dropped(index: usize, tile: &DropTile, text: String) {
    state::hover(Target::Tile(index), false);
    match tile.clone() {
        DropTile::SaveText { folder } => with_folder(index, folder, move |folder| {
            tasks::start(Job::SaveText {
                text,
                to: folder.join("Dropped Text.txt"),
                tile: index,
            });
        }),
        DropTile::Phone { device } => to_phone(index, device, Payload::Text(text)),
        DropTile::Upload { to } => tasks::start(Job::Upload {
            what: Payload::Text(text),
            to,
            tile: index,
        }),
        _ => {}
    }
}

/// Sends `what` to the tile's phone; with none set, to the one connected,
/// else asks which.
fn to_phone(index: usize, device: Option<String>, what: Payload) {
    let connected: Vec<(String, String)> = KdeConnect::read()
        .devices()
        .iter()
        .filter(|device| device.connected())
        .map(|device| (device.id().to_string(), device.name().to_string()))
        .collect();
    let chosen = match device {
        Some(id) => {
            let name = KdeConnect::read()
                .device(&id)
                .map_or_else(|| String::from("phone"), |device| device.name().to_string());
            Some((id, name))
        }
        None if connected.len() == 1 => connected.first().cloned(),
        None => None,
    };
    match chosen {
        Some((device, device_name)) => tasks::start(Job::Phone {
            what,
            device,
            device_name,
            tile: index,
        }),
        None => state::ask(Prompt::Phone {
            what,
            devices: connected,
            tile: index,
        }),
    }
}

/// Runs `then` with the tile's folder, asking for one first when it has
/// none, and keeping what was chosen.
fn with_folder(index: usize, folder: Option<PathBuf>, then: impl FnOnce(PathBuf) + Send + 'static) {
    match folder {
        Some(folder) => then(folder),
        None => choose_folder(index, then),
    }
}

/// Asks for the tile's folder and keeps it.
fn choose_folder(index: usize, then: impl FnOnce(PathBuf) + Send + 'static) {
    FileDialog::open("Choose a Folder")
        .directory(true)
        .show(move |picked| {
            let Some(folder) = picked.into_iter().next() else {
                return;
            };
            let chosen = folder.clone();
            edit_tile(index, move |tile| match tile {
                DropTile::CopyTo { folder }
                | DropTile::MoveTo { folder }
                | DropTile::RenameMove { folder }
                | DropTile::SaveText { folder } => *folder = Some(chosen),
                _ => {}
            });
            then(folder);
        });
}

fn edit_tile(index: usize, edit: impl FnOnce(&mut DropTile)) {
    let grid = state::ui().grid;
    change_drop_grids(|grids| {
        if let Some(tile) = grids
            .grids
            .get_mut(grid)
            .and_then(|grid| grid.tiles.get_mut(index))
        {
            edit(tile);
        }
    });
}

fn remove_tile(index: usize) {
    let grid = state::ui().grid;
    change_drop_grids(|grids| {
        if let Some(grid) = grids.grids.get_mut(grid)
            && index < grid.tiles.len()
        {
            grid.tiles.remove(index);
        }
    });
}

fn add_folder() {
    let grid = state::ui().grid;
    FileDialog::open("Add a Folder")
        .directory(true)
        .show(move |picked| {
            change_drop_grids(|grids| {
                if let Some(grid) = grids.grids.get_mut(grid) {
                    grid.tiles
                        .extend(picked.into_iter().map(|path| DropTile::Folder {
                            path,
                            mode: FolderMode::Copy,
                        }));
                }
            });
        });
}

fn new_grid() {
    let mut index = 0;
    change_drop_grids(|grids| {
        let name = format!("Grid {}", grids.grids.len() + 1);
        grids.grids.push(amane_kit::settings::DropGrid {
            name,
            tiles: Vec::new(),
        });
        index = grids.grids.len() - 1;
    });
    state::change(|ui| ui.grid = index);
    crate::refresh_item();
}

fn delete_grid(index: usize) {
    change_drop_grids(|grids| {
        if grids.grids.len() > 1 && index < grids.grids.len() {
            grids.grids.remove(index);
        }
    });
    state::change(|ui| ui.grid = ui.grid.saturating_sub(1));
    crate::refresh_item();
}

/// A button that adds a tile, while editing.
struct Offer {
    title: &'static str,
    tile: DropTile,
}

/// The actions this grid hasn't got: the file ones, and the ways to share.
fn offers(tiles: &[DropTile]) -> (Vec<Offer>, Vec<Offer>) {
    let missing = |offers: Vec<Offer>| -> Vec<Offer> {
        offers
            .into_iter()
            .filter(|offer| !tiles.iter().any(|tile| same_kind(tile, &offer.tile)))
            .collect()
    };
    let offer = |title, tile| Offer { title, tile };
    let files = missing(vec![
        offer("Copy To", DropTile::CopyTo { folder: None }),
        offer("Move To", DropTile::MoveTo { folder: None }),
        offer("Rename & Move", DropTile::RenameMove { folder: None }),
        offer("Save Text", DropTile::SaveText { folder: None }),
    ]);
    let mut sharing = vec![offer("Phone", DropTile::Phone { device: None })];
    sharing.extend(
        UploadTo::ALL
            .iter()
            .map(|to| offer(to.title(), DropTile::Upload { to: *to })),
    );
    (files, missing(sharing))
}

fn offer_button(offer: Offer) -> Rectangle {
    let tile = offer.tile;
    PushButton::new(format!("amane-drop.add.{}", offer.title), offer.title)
        .icon(ICON_ADD)
        .style(Style::Tonal)
        .on_press(move || {
            let grid = state::ui().grid;
            let tile = tile.clone();
            change_drop_grids(|grids| {
                if let Some(grid) = grids.grids.get_mut(grid) {
                    grid.tiles.push(tile);
                }
            });
        })
        .build()
}

/// Lays `widths` out in lines `width` wide, `gap` apart: where each line
/// starts.
fn wrap(widths: &[f32], width: f32, gap: f32) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut used = f32::INFINITY;
    for (index, item) in widths.iter().enumerate() {
        if used + gap + item > width {
            starts.push(index);
            used = *item;
        } else {
            used += gap + item;
        }
    }
    starts
}

/// While editing, the lines of buttons that add what this grid hasn't got:
/// the file actions, then "Share" and the ways to share, wrapped to fit.
fn edit_lines(tiles: &[DropTile], width: f32) -> Vec<Vec<Box<dyn Widget>>> {
    let (files, sharing) = offers(tiles);
    let caption = |text: &str, width: f32| {
        label(
            text,
            sys::r#type::caption(),
            material::label(Level::Secondary),
            width,
        )
    };
    let mut lines: Vec<Vec<Box<dyn Widget>>> = Vec::new();
    for (index, section) in [files, sharing].into_iter().enumerate() {
        let mut items: Vec<Box<dyn Widget>> = Vec::new();
        if index == 1 && !section.is_empty() {
            items.push(boxed(caption("Share", 40.0)));
        }
        items.extend(
            section
                .into_iter()
                .map(|offer| boxed(offer_button(offer)) as Box<dyn Widget>),
        );
        let widths: Vec<f32> = items.iter().map(|item| fixed_size(item.as_ref())).collect();
        let starts = wrap(&widths, width, 4.0);
        let mut items = items.into_iter();
        for (at, start) in starts.iter().enumerate() {
            let end = starts.get(at + 1).copied().unwrap_or(widths.len());
            lines.push(items.by_ref().take(end - start).collect());
        }
    }
    if lines.is_empty() {
        lines.push(vec![boxed(caption(
            "Drop folders or apps between the tiles to add them.",
            width,
        ))]);
    }
    lines
}

/// A widget's width, when it's fixed.
fn fixed_size(widget: &dyn Widget) -> f32 {
    match widget.width() {
        Size::Fixed(width) => width,
        Size::Parent => 0.0,
    }
}

fn edit_row(tiles: &[DropTile], width: f32) -> Rectangle {
    let lines = edit_lines(tiles, width);
    let height = edit_height(lines.len());
    let rows: Vec<Box<dyn Widget>> = lines
        .into_iter()
        .map(|line| {
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(width))
                    .height(Size::Fixed(EDIT_ROW))
                    .fill(Color::TRANSPARENT)
                    .align_child(Start, Center)
                    .child(Row::new(line).gap(4.0).align(Center)),
            )
        })
        .collect();
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .child(Column::new(rows).gap(4.0))
}

fn same_kind(a: &DropTile, b: &DropTile) -> bool {
    match (a, b) {
        (DropTile::Upload { to: a }, DropTile::Upload { to: b }) => a == b,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}

// ───── questions ─────────────────────────────────────────────────────────────

/// A question's title, its detail, a field if it has one, and its answers.
type Asked = (
    String,
    String,
    Option<Box<dyn Widget>>,
    Vec<Box<dyn Widget>>,
);

fn ask(prompt: Prompt, width: f32) -> Rectangle {
    let primary = material::label(Level::Primary);
    let secondary = material::label(Level::Secondary);
    let (title, detail, field, buttons): Asked = match prompt {
        Prompt::Conflict { job, names } => {
            let folder = match &job {
                Job::Copy { folder, .. } | Job::Move { folder, .. } => name(folder),
                _ => String::new(),
            };
            let title = match names.as_slice() {
                [one] => format!("“{one}” is already in {folder}"),
                many => format!("{} items are already in {folder}", many.len()),
            };
            let (keep, replace) = (job.clone(), job);
            (
                title,
                String::from("Keep both, with a number after the new one's name, or replace?"),
                None,
                vec![
                    boxed(button("amane-drop.stop", "Stop", Style::Plain, || {
                        state::change(|ui| ui.prompt = None);
                    })),
                    boxed(button(
                        "amane-drop.replace",
                        "Replace",
                        Style::Destructive,
                        move || {
                            state::change(|ui| ui.prompt = None);
                            tasks::run(replace.clone(), false);
                        },
                    )),
                    boxed(button(
                        "amane-drop.keep",
                        "Keep Both",
                        Style::Filled,
                        move || {
                            state::change(|ui| ui.prompt = None);
                            tasks::run(keep.clone(), true);
                        },
                    )),
                ],
            )
        }
        Prompt::Phone {
            what,
            devices,
            tile,
        } => {
            let mut buttons: Vec<Box<dyn Widget>> = vec![boxed(button(
                "amane-drop.cancel",
                "Cancel",
                Style::Plain,
                || {
                    state::change(|ui| ui.prompt = None);
                },
            ))];
            for (id, device_name) in devices.iter().take(3) {
                let (what, device, device_name) = (what.clone(), id.clone(), device_name.clone());
                buttons.push(boxed(
                    PushButton::new(format!("amane-drop.phone.{id}"), &device_name)
                        .icon(ICON_PHONE)
                        .style(Style::Filled)
                        .on_press(move || {
                            state::change(|ui| ui.prompt = None);
                            tasks::start(Job::Phone {
                                what: what.clone(),
                                device: device.clone(),
                                device_name: device_name.clone(),
                                tile,
                            });
                        })
                        .build(),
                ));
            }
            let (title, detail) = if devices.is_empty() {
                (
                    String::from("No phone is connected"),
                    String::from(
                        "Pair one with KDE Connect in Quick Settings › Phone, on the same network.",
                    ),
                )
            } else {
                (
                    String::from("Send to which phone?"),
                    String::from("Choose one for this tile while editing to skip this."),
                )
            };
            (title, detail, None, buttons)
        }
        Prompt::Rename {
            files,
            folder,
            tile,
        } => {
            let title = match files.as_slice() {
                [one] => format!("Rename “{}”", name(one)),
                many => format!("Rename {} items", many.len()),
            };
            let detail = format!("Moved into {}", name(&folder));
            let (typed, moved) = ((files.clone(), folder.clone()), (files, folder));
            let input = text_field(RENAME, "Name")
                .focused()
                .on_submit(move |text| rename(&typed.0, &typed.1, &text, tile));
            (
                title,
                detail,
                Some(boxed(field_box(RENAME, input, width))),
                vec![
                    boxed(button("amane-drop.cancel", "Cancel", Style::Plain, || {
                        state::change(|ui| ui.prompt = None);
                    })),
                    boxed(button(
                        "amane-drop.rename",
                        "Move",
                        Style::Filled,
                        move || {
                            rename(&moved.0, &moved.1, &TextInput::text(RENAME), tile);
                        },
                    )),
                ],
            )
        }
    };
    let mut rows: Vec<Box<dyn Widget>> = vec![
        boxed(label(&title, sys::r#type::label(), primary, width)),
        boxed(label(&detail, sys::r#type::caption(), secondary, width)),
    ];
    rows.extend(field);
    rows.push(boxed(Row::new(buttons).gap(6.0).align(Center)));
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(PROMPT))
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .child(Column::new(rows).gap(8.0))
}

fn button(id: &'static str, title: &str, style: Style, press: impl Fn() + 'static) -> Rectangle {
    PushButton::new(id, title)
        .style(style)
        .on_press(press)
        .build()
}

fn rename(files: &[PathBuf], folder: &Path, typed: &str, tile: usize) {
    if typed.trim().is_empty() {
        return;
    }
    state::change(|ui| ui.prompt = None);
    tasks::start(Job::Rename {
        moves: tasks::renamed(files, folder, typed),
        tile,
    });
}

// ───── the Drop Bar ──────────────────────────────────────────────────────────

/// The files held, a stack a drop: drag one out to another program or onto
/// a tile; a right-click lets it go.
fn shelf(grid: &str, width: f32) -> Rectangle {
    let stacks = state::stacks(grid);
    let hovered = state::ui().hover == Some(Target::Bar);
    let secondary = material::label(Level::Secondary);
    let accent = sys::color::accent::bg();

    let content: Box<dyn Widget> = if stacks.is_empty() {
        boxed(label(
            "Drop Bar: drop files here to hold them for later",
            sys::r#type::caption(),
            secondary,
            width - 24.0,
        ))
    } else {
        let mut chips: Vec<Box<dyn Widget>> = stacks
            .into_iter()
            .enumerate()
            .take(((width - 48.0) / 52.0) as usize)
            .map(|(index, files)| boxed(stack_chip(grid, index, files)))
            .collect();
        let cleared = grid.to_string();
        chips.push(boxed(
            IconButton::new("amane-drop.clear", ICON_CLOSE)
                .on_press(move || state::let_go(&cleared, None))
                .build(),
        ));
        boxed(Row::new(chips).gap(4.0).align(Center))
    };
    let held = grid.to_string();
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(SHELF))
        .radius(12.0)
        .fill(if hovered {
            with_alpha(accent, 0.2)
        } else {
            material::fill_role(FillLevel::Tertiary)
        })
        .border(if hovered { 2.0 } else { 0.0 }, accent)
        .padding(Padding {
            left: 8.0,
            right: 8.0,
            ..Padding::default()
        })
        .on_files_over(|at, _| {
            state::hover(Target::Bar, at.is_some());
            true
        })
        .on_drop(move |files| {
            state::hover(Target::Bar, false);
            state::hold(&held, files);
        })
        .align_child(Start, Center)
        .child(Row::new(vec![content]))
}

fn stack_chip(grid: &str, index: usize, files: Vec<PathBuf>) -> Rectangle {
    let gone = files.iter().all(|file| !file.exists());
    let first = files.first().cloned().unwrap_or_default();
    let picture: Box<dyn Widget> = if is_image(&first) && Image::loaded(&first) {
        boxed(
            Rectangle::new()
                .width(Size::Fixed(40.0))
                .height(Size::Fixed(40.0))
                .radius(6.0)
                .fill(Image::cover(first.clone())),
        )
    } else {
        let glyph = if first.is_dir() {
            ICON_FOLDER_FILLED
        } else if is_image(&first) {
            ICON_IMAGE
        } else {
            ICON_DESCRIPTION
        };
        boxed(icon(glyph, 30.0, sys::color::accent::bg()))
    };
    let count = files.len();
    let mut layers: Vec<Box<dyn Widget>> = vec![boxed(
        Rectangle::new()
            .width(Size::Fixed(48.0))
            .height(Size::Fixed(48.0))
            .fill(Color::TRANSPARENT)
            .align_child(Center, Center)
            .child(Row::new(vec![picture])),
    )];
    if count > 1 {
        layers.push(boxed(
            Rectangle::new()
                .width(Size::Fixed(48.0))
                .height(Size::Fixed(18.0))
                .fill(Color::TRANSPARENT)
                .align_child(amane::End, Start)
                .child(
                    Rectangle::new()
                        .width(Size::Fixed(18.0))
                        .height(Size::Fixed(18.0))
                        .radius(9.0)
                        .fill(sys::color::accent::bg())
                        .align_child(Center, Center)
                        .child(
                            Text::new(count.to_string())
                                .style(sys::r#type::caption())
                                .color(Color::WHITE),
                        ),
                ),
        ));
    }
    let grid = grid.to_string();
    let dragged = files.clone();
    let chip = Rectangle::new()
        .width(Size::Fixed(48.0))
        .height(Size::Fixed(48.0))
        .fill(Color::TRANSPARENT)
        .drag_out(move || {
            DragData::Files(
                dragged
                    .iter()
                    .filter(|file| file.exists())
                    .cloned()
                    .collect(),
            )
        })
        .on_click(move |button| {
            if button == Button::Right {
                state::let_go(&grid, Some(index));
            }
        })
        .child(Stack::new(layers));
    if gone { chip.opacity(0.4) } else { chip }
}

fn is_image(file: &Path) -> bool {
    file.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp"
            )
        })
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
