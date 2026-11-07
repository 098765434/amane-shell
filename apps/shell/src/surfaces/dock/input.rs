//! The dock's pointer: the still layer that takes it, drags, dropped files, clicks.

use super::*;

/// The still layer over the whole surface that takes the pointer for every
/// icon: it never moves or changes, and comes first, so the hover it's
/// told of and the targets after it stay put while the icons swell (amane
/// finds a hovered target by its place in the frame).
pub(super) fn still_layer(
    output: &str,
    laid: &Laid,
    spaces: &[Space],
    at: Measures,
    holding: bool,
) -> Rectangle {
    let (hover_on, move_on, click_on) =
        (output.to_string(), output.to_string(), output.to_string());
    let (drag_on, release_on) = (output.to_string(), output.to_string());
    let (files_on, drop_on) = (output.to_string(), output.to_string());
    let pressed = laid.clone();
    let (held_over, dropped_on) = (laid.clone(), laid.clone());
    let drop_spaces = spaces.to_vec();
    // what a click needs of the row
    let laid = laid.clone();
    let spaces = spaces.to_vec();

    let layer = blank()
        .on_hover(move |over| {
            with_dock(&hover_on, |dock| {
                dock.reveal.hover(over);
                if !over {
                    dock.pointer = None;
                }
            });
        })
        .on_move(move |point: Point| {
            with_dock(&move_on, |dock| {
                dock.pointer = Some((point.x, point.y));
                // a pointer that arrived without an enter still counts
                dock.reveal.hover(true);
            });
        })
        .on_drag(move |point: Point| {
            with_dock(&drag_on, |dock| match &mut dock.drag {
                Some(drag) => drag.to((point.x, point.y)),
                None => dock.drag = take(&pressed, (point.x, point.y), at).filter(|_| dock.out),
            });
            // the dock under the pointer draws again for its input; no other shows the drag
        })
        .on_release(move |point: Point| {
            with_dock(&release_on, |dock| let_go(dock, point.y, at));
            Docks::changed();
        })
        .on_files_over(move |point, files| {
            let motion = motion::motion();
            with_dock(&files_on, |dock| {
                files_over(dock, &held_over, point, files, at, &motion)
            })
        })
        .on_drop(move |files| {
            with_dock(&drop_on, |dock| {
                let_files_go(dock, &dropped_on, &drop_spaces, &files, at)
            });
        })
        .on_click(move |button| {
            let pointer = with_dock(&click_on, |dock| {
                // the button let go after a drag isn't a click
                if std::mem::take(&mut dock.dragged) {
                    return None;
                }
                dock.pointer.filter(|_| dock.out)
            });
            let Some((x, y)) = pointer else {
                return;
            };
            if y < at.top - at.headroom || y > at.top + at.card {
                return;
            }
            let Some(index) = laid.under(along(at, laid.width, x)) else {
                return;
            };
            if let Kind::Icon(icon) = &laid.entries[index].kind {
                click(&click_on, icon, &spaces, button, (x, y));
            }
        });
    if holding {
        layer.cursor(Cursor::Grabbing)
    } else {
        layer
    }
}

/// The icon at `at` on the surface, and its key.
pub(super) fn icon_at(laid: &Laid, at: (f32, f32), measures: Measures) -> Option<(String, Icon)> {
    let (x, y) = at;
    if y < measures.top - measures.headroom || y > measures.top + measures.card {
        return None;
    }
    let index = laid.under(along(measures, laid.width, x))?;
    match &laid.entries[index].kind {
        Kind::Icon(icon) => Some((laid.entries[index].key.clone(), icon.clone())),
        _ => None,
    }
}

/// Whether `icon`'s app opens every one of `files`.
pub(super) fn opens(icon: &Icon, files: &[PathBuf]) -> bool {
    !files.is_empty()
        && app_icon::entry_for(icon.app_id())
            .is_some_and(|app| files.iter().all(|file| app.opens(file)))
}

/// Files from another program held at `point` (none: gone) over the
/// dock: they swell the icons as the pointer does, and can be let go on
/// an icon whose app opens them.
pub(super) fn files_over(
    dock: &mut Dock,
    laid: &Laid,
    point: Option<Point>,
    files: &[PathBuf],
    at: Measures,
    motion: &DockMotion,
) -> bool {
    let Some(point) = point else {
        dock.files = None;
        dock.pointer = None;
        return false;
    };
    let point = (point.x, point.y);
    dock.files_at = Some(point);
    dock.pointer = Some(point);
    let out = dock.out;
    let held = dock.files.get_or_insert_with(FilesOver::default);
    // still coming up from the edge
    let under = icon_at(laid, point, at).filter(|_| out);
    let Some((key, icon)) = under else {
        *held = FilesOver::default();
        return false;
    };
    if held.icon.as_ref().map(|(over, _)| over) != Some(&key) {
        held.icon = Some((key, Instant::now()));
        held.sprung = false;
        held.opens = opens(&icon, files);
        if let Some(delay) = motion.spring_load {
            redraw_after::<Docks>(delay + Duration::from_millis(10));
        }
    }
    held.opens
}

/// Files held over one icon long enough bring its window into view, so
/// they can be let go in it; a pinned app that isn't open opens.
pub(super) fn spring_load(dock: &mut Dock, laid: &Laid, spaces: &[Space], motion: &DockMotion) {
    let (Some(delay), Some(held)) = (motion.spring_load, dock.files.as_mut()) else {
        return;
    };
    let Some((key, since)) = &held.icon else {
        return;
    };
    if held.sprung || since.elapsed() < delay {
        return;
    }
    held.sprung = true;
    let icon = laid.entries.iter().find_map(|entry| match &entry.kind {
        Kind::Icon(icon) if entry.key == *key => Some(icon.clone()),
        _ => None,
    });
    match icon {
        Some(Icon::Window(window)) => Windows::focus(window.id),
        Some(Icon::Pin(pin)) => {
            let windows: Vec<Win> = Windows::read().list().iter().map(Win::from).collect();
            let recent = RECENT.with_borrow(Clone::clone);
            match model::latest(&pin.app_id, &windows, &recent) {
                Some(id) => Windows::focus(id),
                None => {
                    if let Some(workspace) = pin_workspace(&pin, spaces) {
                        pins::open(&pin.app_id, workspace);
                    }
                }
            }
        }
        None => {}
    }
}

/// `files` let go over the dock: opened with the app of the icon they
/// were on, on its workspace, if it opens them.
pub(super) fn let_files_go(
    dock: &mut Dock,
    laid: &Laid,
    spaces: &[Space],
    files: &[PathBuf],
    at: Measures,
) {
    let Some(point) = dock.files_at.take() else {
        return;
    };
    let Some((_, icon)) = icon_at(laid, point, at) else {
        return;
    };
    if !opens(&icon, files) {
        return;
    }
    let workspace = match &icon {
        Icon::Window(window) => window.workspace,
        Icon::Pin(pin) => pin_workspace(pin, spaces),
    };
    if let Some(workspace) = workspace {
        pins::open_with(icon.app_id(), workspace, files);
    }
}

/// The icon the button went down on at `at`, held from where the pointer
/// took it, if the press was on one.
pub(super) fn take(laid: &Laid, at: (f32, f32), measures: Measures) -> Option<Drag> {
    let (x, y) = at;
    if y < measures.top - measures.headroom || y > measures.top + measures.card {
        return None;
    }
    let index = laid.under(along(measures, laid.width, x))?;
    let Kind::Icon(icon) = &laid.entries[index].kind else {
        return None;
    };
    let left =
        measures.card_left(laid.width) + measures.pad + laid.rests[index].slot * measures.pitch;
    let top = measures.top + measures.pad;
    let side = measures.side;
    Some(Drag {
        key: laid.entries[index].key.clone(),
        icon: icon.clone(),
        grab: ((x - left).clamp(0.0, side), (y - top).clamp(0.0, side)),
        pressed: at,
        pointer: at,
        moved: false,
    })
}

/// A click on `icon` at `at` on monitor `output`'s dock.
pub(super) fn click(output: &str, icon: &Icon, spaces: &[Space], button: Button, at: (f32, f32)) {
    let at = (at.0 as i32, at.1 as i32);
    match (icon, button) {
        (Icon::Window(window), Button::Left) => Windows::focus(window.id),
        (Icon::Window(window), Button::Right) => {
            MENU_ON.set(Some(output.to_string()));
            open_window_menu(window, spaces, DOCK_NS, at);
        }
        (Icon::Pin(pin), Button::Left) => {
            let windows: Vec<Win> = Windows::read().list().iter().map(Win::from).collect();
            let recent = RECENT.with_borrow(Clone::clone);
            match model::latest(&pin.app_id, &windows, &recent) {
                Some(id) => Windows::focus(id),
                None => {
                    if let Some(workspace) = pin_workspace(pin, spaces) {
                        pins::open(&pin.app_id, workspace);
                    }
                }
            }
        }
        (Icon::Pin(pin), Button::Right) => {
            if let Some(workspace) = pin_workspace(pin, spaces) {
                MENU_ON.set(Some(output.to_string()));
                open_pin_menu(pin, workspace, DOCK_NS, at);
            }
        }
        _ => {}
    }
}

/// The workspace `pin` opens its app on: its own, or, while that isn't
/// open, the empty one niri keeps after the last on its monitor.
pub(super) fn pin_workspace(pin: &pins::Pin, spaces: &[Space]) -> Option<i64> {
    model::resolve(pin, spaces)
        .or_else(|| {
            spaces
                .iter()
                .filter(|space| space.output == pin.output)
                .max_by_key(|space| space.index)
        })
        .map(|space| space.id)
}
