//! What the tiles do: a click, files or text dropped on one, the phone
//! sends, and editing the grid (its tiles, the folders and apps offered).

use super::*;

pub(super) fn clicked(index: usize, tile: &DropTile, editing: bool) {
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
pub(super) fn open_settings() {
    amane::spawn("amane-settings --page drop");
    state::close();
}

/// The paired phone after `now` in the list, then back to asking.
pub(super) fn next_phone(now: Option<&str>) -> Option<String> {
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

pub(super) fn open_folder(folder: &Path) {
    amane_kit::launch::open(folder.to_string_lossy());
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

pub(super) fn dropped_on(index: usize, tile: &DropTile, files: Vec<PathBuf>) {
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

pub(super) fn text_dropped(index: usize, tile: &DropTile, text: String) {
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
pub(super) fn to_phone(index: usize, device: Option<String>, what: Payload) {
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
pub(super) fn with_folder(
    index: usize,
    folder: Option<PathBuf>,
    then: impl FnOnce(PathBuf) + Send + 'static,
) {
    match folder {
        Some(folder) => then(folder),
        None => choose_folder(index, then),
    }
}

/// Asks for the tile's folder and keeps it.
pub(super) fn choose_folder(index: usize, then: impl FnOnce(PathBuf) + Send + 'static) {
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

pub(super) fn edit_tile(index: usize, edit: impl FnOnce(&mut DropTile)) {
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

pub(super) fn remove_tile(index: usize) {
    let grid = state::ui().grid;
    change_drop_grids(|grids| {
        if let Some(grid) = grids.grids.get_mut(grid)
            && index < grid.tiles.len()
        {
            grid.tiles.remove(index);
        }
    });
}

pub(super) fn add_folder() {
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

pub(super) fn new_grid() {
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

pub(super) fn delete_grid(index: usize) {
    change_drop_grids(|grids| {
        if grids.grids.len() > 1 && index < grids.grids.len() {
            grids.grids.remove(index);
        }
    });
    state::change(|ui| ui.grid = ui.grid.saturating_sub(1));
    crate::refresh_item();
}

/// A button that adds a tile, while editing.
pub(super) struct Offer {
    title: &'static str,
    tile: DropTile,
}

/// The actions this grid hasn't got: the file ones, and the ways to share.
pub(super) fn offers(tiles: &[DropTile]) -> (Vec<Offer>, Vec<Offer>) {
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

pub(super) fn offer_button(offer: Offer) -> Rectangle {
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
pub(super) fn wrap(widths: &[f32], width: f32, gap: f32) -> Vec<usize> {
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
pub(super) fn edit_lines(tiles: &[DropTile], width: f32) -> Vec<Vec<Box<dyn Widget>>> {
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
        let widths: Vec<f32> = items
            .iter()
            .map(|item| amane_kit::layout::fixed_width(item.as_ref()))
            .collect();
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

pub(super) fn edit_row(tiles: &[DropTile], width: f32) -> Rectangle {
    let lines = edit_lines(tiles, width);
    let height = edit_height(lines.len());
    let rows: Vec<Box<dyn Widget>> = lines
        .into_iter()
        .map(|line| {
            boxed(
                frame(width, EDIT_ROW)
                    .align_child(Start, Center)
                    .child(Row::new(line).gap(4.0).align(Center)),
            )
        })
        .collect();
    frame(width, height).child(Column::new(rows).gap(4.0))
}

pub(super) fn same_kind(a: &DropTile, b: &DropTile) -> bool {
    match (a, b) {
        (DropTile::Upload { to: a }, DropTile::Upload { to: b }) => a == b,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}
