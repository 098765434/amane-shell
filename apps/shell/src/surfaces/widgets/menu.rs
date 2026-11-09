//! A card's menu and the desktop's: the sizes, what it shows, and the commands they run.

use super::*;

pub(super) fn checked(item: MenuItem, on: bool) -> MenuItem {
    item.checked(on)
}

/// A card's menu: the sizes it comes in, what it shows, Edit Widgets… and
/// Remove Widget.
pub(super) fn widget_menu(output: &str, key: WidgetKey, at: (i32, i32)) {
    let Some(layout) = current(output) else {
        return;
    };
    let span = layout
        .span_of(key)
        .unwrap_or(key.kind.default_size().span());
    let mut menu = ContextMenu::new();
    for size in key.kind.sizes() {
        let item = checked(MenuItem::label(size.title()), size.span() == span);
        let fits = size.span() == span || layout.resized(key, size.span()).is_some();
        let on = output.to_string();
        menu = menu.item(if fits {
            item.on_select(move || resize(&on, key, size.span()))
        } else {
            item.disabled()
        });
    }
    let settings = key.kind.spec().settings;
    if !settings.is_empty() {
        menu = menu.separator();
    }
    for setting in settings {
        menu = menu.item(setting_item(key, setting));
    }
    let editing = edit::editing(output);
    let (on_edit, on_remove) = (output.to_string(), output.to_string());
    if !editing {
        menu = menu
            .separator()
            .item(MenuItem::label("Edit Widgets…").on_select(move || edit::enter(&on_edit)));
    }
    menu = menu
        .separator()
        .item(
            MenuItem::label("Remove Widget")
                .destructive()
                .on_select(move || {
                    // while editing, Undo Remove puts it back
                    if edit::editing(&on_remove) {
                        edit::remove(&on_remove, key);
                    } else {
                        remove(&on_remove, key);
                    }
                }),
        );
    // while editing, the menu hangs under the edit surface, over the windows
    let surface = if editing {
        namespace(&format!("edit.{output}"))
    } else {
        namespace(output)
    };
    context_menu::show_under(WIDGET_MENU_KEY, menu, surface, at);
}

/// `setting`'s item in `key`'s menu: a submenu of its choices with the
/// one set checked, or a switch.
pub(super) fn setting_item(key: WidgetKey, setting: &'static SettingSpec) -> MenuItem {
    let settings = store::with(|store| store.settings(key.id));
    let now = settings.get(setting.key).map(String::from);
    let set = move |value: Option<String>| {
        let next = settings.clone().with(setting.key, value);
        move || {
            store::with(|store| store.set_settings(key.id, next.clone()));
            changed();
        }
    };
    match &setting.kind {
        SettingKind::Choice { automatic, options } => {
            let mut menu = ContextMenu::new()
                .item(checked(MenuItem::label(*automatic), now.is_none()).on_select(set(None)));
            let options = options();
            if !options.is_empty() {
                menu = menu.separator();
            }
            for (id, name) in options {
                let on = now.as_deref() == Some(id.as_str());
                menu = menu.item(checked(MenuItem::label(name), on).on_select(set(Some(id))));
            }
            MenuItem::label(setting.title).submenu(menu)
        }
        SettingKind::Toggle { value, inverted } => {
            let on = now.as_deref() == Some(*value);
            checked(MenuItem::label(setting.title), on != *inverted)
                .on_select(set((!on).then(|| (*value).to_string())))
        }
    }
}

/// The desktop's menu: Edit Widgets….
pub(super) fn desktop_menu(output: &str, at: (i32, i32)) {
    let on = output.to_string();
    let menu = ContextMenu::new()
        .item(MenuItem::label("Edit Widgets…").on_select(move || edit::enter(&on)));
    context_menu::show_under(DESKTOP_MENU_KEY, menu, namespace(output), at);
}

pub(super) fn resize(output: &str, key: WidgetKey, span: Span) {
    if let Some(next) = current(output).and_then(|layout| layout.resized(key, span)) {
        commit(output, &next);
    }
}

pub(super) fn remove(output: &str, key: WidgetKey) {
    if let Some(mut layout) = current(output) {
        layout.remove(key);
        commit(output, &layout);
    }
}

/// Adds a `kind` widget to `output` at `size`, in the first free cells or
/// at `at`; whether there was room.
pub(super) fn add(output: &str, kind: WidgetKind, size: WidgetSize, at: Option<Place>) -> bool {
    let Some(layout) = current(output) else {
        return false;
    };
    let added = store::with(|store| store.add(output, &layout, kind, size, at)).is_some();
    changed();
    added
}

/// The monitor a command means: the one named (when it's connected), else
/// the focused workspace's, else the first with widgets; or what's wrong.
pub(super) fn output_for(name: Option<&String>) -> Result<String, String> {
    if let Some(name) = name {
        return if crate::chrome::per_output::connected(name) {
            Ok(name.clone())
        } else {
            Err(format!("error: no monitor {name}"))
        };
    }
    let focused = Workspaces::read()
        .list()
        .iter()
        .find(|ws| ws.focused())
        .and_then(|ws| ws.output().map(String::from));
    focused
        .or_else(|| DESKS.with(|desks| desks.all(|desks| desks.keys().next().cloned())))
        .ok_or_else(|| String::from("error: no monitor"))
}
