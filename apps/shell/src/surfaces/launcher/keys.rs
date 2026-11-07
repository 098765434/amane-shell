//! The launcher's keyboard: what each key does in each mode.

use super::*;

/// Every key: nothing else on the panel takes the focus.
pub(super) fn on_key(key: Key) {
    if with(|state| state.capture.is_some()) {
        capture_key(key);
        return;
    }
    let apps = Apps::read();
    let view = View::new(apps.list());
    let held = modifiers();
    let typed = !view.field.text().is_empty();
    let listing = !view.grid();

    // a chord isn't text: the editing ones edit, Ctrl+1 … 4 pick a mode,
    // and the rest go nowhere
    if held.ctrl
        && let Key::Character(letter) = key
    {
        match letter.to_ascii_lowercase() {
            'a' if typed => with(|state| state.field.select_all()),
            'c' if typed => text_edit::copy(&view.field.selected()),
            'x' if typed => {
                text_edit::copy(&view.field.selected());
                type_into(Field::delete_selection);
            }
            'v' => text_edit::paste(|text| {
                type_into(|field| {
                    field.insert(&text);
                    true
                });
                Launcher::changed();
            }),
            // the keyboard's way to set the picked action's quick keys
            'k' => {
                if let Some(entry) = view.entry(view.selected)
                    && let Some(title) = &entry.action_title
                    && !matches!(entry.action, entries::Action::Hint)
                {
                    start_capture(&entry.key, title, entry.quick_keys.as_deref().unwrap_or(""));
                }
            }
            digit => {
                if let Some(mode) = Mode::from_digit(digit) {
                    switch_mode(mode);
                }
            }
        }
        Launcher::changed();
        return;
    }
    if held.command() && matches!(key, Key::Character(_) | Key::Space) {
        return;
    }

    let layout = view.layout();
    let count = view.count();
    let selected = view.selected;
    let height = view.list_height();
    let set_focus = |zone| with(|state| state.focus = zone);
    let history = with(|state| state.history_at.is_some());
    let fresh = view.mode == Mode::Home && view.scope.is_none() && (!typed || history);

    match (key, view.focus) {
        (Key::Character(letter), _) => type_into(|field| {
            field.insert(&letter.to_string());
            true
        }),
        // a search doesn't start with a space
        (Key::Space, _) if typed => type_into(|field| {
            field.insert(" ");
            true
        }),
        // Backspace in an empty field leaves the scope
        (Key::Backspace, _) if !typed && view.scope.is_some() => back(),
        (Key::Backspace, _) => type_into(Field::backspace),
        // Tab narrows to the row picked, when it narrows to anything
        (Key::Tab, _) if !held.shift && listing => {
            match view.entry(selected).and_then(|entry| entry.scope.clone()) {
                Some(scope) => {
                    drop(view);
                    drop(apps);
                    enter_scope(scope);
                }
                None => set_focus(view.focus.next(false, view.pills())),
            }
        }
        (Key::Tab, focus) => set_focus(focus.next(held.shift, view.pills())),
        (Key::Escape, _) => {
            drop(view);
            drop(apps);
            back();
        }
        // Ctrl+Return shows a file in the file manager
        (Key::Enter, _) if held.ctrl => {
            if let Some(entries::Action::OpenFile(path)) =
                view.entry(selected).map(|entry| entry.action.clone())
            {
                close();
                file_search::reveal(&path);
            }
        }
        (Key::Enter, Zone::Pills) => set_focus(Zone::Body),
        // Shift+Return puts a copy back on the clipboard, without pasting it
        (Key::Enter, _) if held.shift && view.mode == Mode::Clipboard => {
            if let Some(entries::Action::Paste(clip)) =
                view.entry(selected).map(|entry| entry.action.clone())
            {
                close();
                clipboard_history::put_back(&clip, false);
            }
        }
        (Key::Enter, _) => {
            drop(view);
            drop(apps);
            run_selected();
        }
        (Key::Delete, _) if view.mode == Mode::Clipboard => {
            if let Some(entries::Action::Paste(clip)) =
                view.entry(selected).map(|entry| entry.action.clone())
            {
                clipboard_history::delete(clip.id);
            }
        }

        // the searches run before, from an empty field
        (Key::Up, Zone::Search) if fresh && walk_history(true) => {}
        (Key::Down, Zone::Search) if fresh && history => {
            walk_history(false);
        }

        // while listing, Left and Right are the caret's and Up and Down
        // the list's, wherever the focus is
        (Key::Left, _) if listing && typed => with(|state| state.field.left(held.shift)),
        (Key::Right, _) if listing && typed => with(|state| state.field.right(held.shift)),
        (Key::Home, _) if listing && typed => with(|state| state.field.move_to(0, held.shift)),
        (Key::End, _) if listing && typed => {
            with(|state| state.field.move_to(usize::MAX, held.shift))
        }

        (Key::Down, Zone::Search) if view.pills() => set_focus(Zone::Pills),
        (Key::Down, Zone::Search) if count > 0 => {
            set_focus(Zone::Body);
            select((selected + 1).min(count - 1), &layout, height);
        }

        (Key::Left, Zone::Pills) => turn_to(view.page.saturating_sub(1)),
        (Key::Right, Zone::Pills) => {
            turn_to((view.page + 1).min(view.buckets.len().saturating_sub(1)))
        }
        (Key::Home, Zone::Pills) => turn_to(0),
        (Key::End, Zone::Pills) => turn_to(view.buckets.len().saturating_sub(1)),
        (Key::Up, Zone::Pills) => set_focus(Zone::Search),
        (Key::Down, Zone::Pills) => set_focus(Zone::Body),

        // off the top of the apps or rows, up to what's over them
        (Key::Up, Zone::Body) if selected < layout.row() => {
            if selected > 0 {
                select(0, &layout, height);
            }
            set_focus(if view.pills() {
                Zone::Pills
            } else {
                Zone::Search
            })
        }
        (Key::Left, Zone::Body) if count > 0 => select(selected.saturating_sub(1), &layout, height),
        (Key::Right, Zone::Body) if count > 0 => {
            select((selected + 1).min(count - 1), &layout, height)
        }
        (Key::Up, Zone::Body) if count > 0 => {
            select(selected.saturating_sub(layout.row()), &layout, height)
        }
        (Key::Down, Zone::Body) if count > 0 => {
            select((selected + layout.row()).min(count - 1), &layout, height)
        }
        (Key::Home, Zone::Body) if count > 0 => select(0, &layout, height),
        (Key::End, Zone::Body) if count > 0 => select(count - 1, &layout, height),
        (Key::Up, Zone::Search) if listing && count > 0 => {
            select(selected.saturating_sub(1), &layout, height)
        }
        _ => return,
    }
    Launcher::changed();
}
