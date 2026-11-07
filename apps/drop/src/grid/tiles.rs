//! The tiles: what each shows (a folder, an app, an action, a phone) and
//! the ring of a job under way.

use super::*;

pub(super) fn tiles(tiles: &[DropTile], columns: usize, editing: bool) -> Rectangle {
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
        .height(rows_count * TILE_H + (rows_count - 1.0) * GAP)
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
pub(super) fn becomes_tile(file: &Path) -> bool {
    file.is_dir()
        || file
            .extension()
            .is_some_and(|extension| extension == "desktop")
}

pub(super) fn add_tiles(files: Vec<PathBuf>) {
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
pub(super) fn look(tile: &DropTile) -> (Picture, String) {
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

pub(super) fn upload_glyph(to: UploadTo) -> char {
    match to {
        UploadTo::ZeroX0 | UploadTo::S3 => ICON_CLOUD_UPLOAD,
        UploadTo::Imgur => ICON_IMAGE,
        UploadTo::Sftp => ICON_DNS,
    }
}

/// Whether a tile takes text let go on it.
pub(super) fn takes_text(tile: &DropTile) -> bool {
    matches!(
        tile,
        DropTile::SaveText { .. } | DropTile::Phone { .. } | DropTile::Upload { .. }
    )
}

pub(super) enum Picture {
    Glyph(char),
    File(PathBuf),
}

pub(super) fn tile_cell(index: usize, tile: &DropTile, editing: bool) -> Rectangle {
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
        .width(TILE_W)
        .height(TILE_H)
        .radius(12.0)
        .fill(if hovered {
            amane_kit::color::with_alpha(accent, 0.25)
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
    frame(TILE_W, TILE_H).child(Stack::new(vec![
        boxed(cell),
        boxed(
            Rectangle::new()
                .width(TILE_W)
                .height(Size::Fixed(24.0))
                .align_child(amane::End, Start)
                .child(remove),
        ),
    ]))
}

/// A ring `diameter` across, filled `done` of the way round from the top.
pub(super) fn ring(done: f32, diameter: f32, color: Color) -> Canvas {
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
        .width(diameter)
        .height(diameter)
        .shapes(shapes)
}

/// `text` cut with an ellipsis to fit `width`.
pub(super) fn clip(text: &str, style: amane_kit::tokens::TextStyle, width: f32) -> String {
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
