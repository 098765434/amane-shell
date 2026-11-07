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
    let choices = choices(key);
    if !choices.is_empty() {
        menu = menu.separator();
    }
    for (title, submenu) in choices {
        menu = menu.item(MenuItem::label(title).submenu(submenu));
    }
    if key.kind == WidgetKind::Clock {
        menu = menu
            .item(toggle(key, "Show Seconds", |s| &mut s.seconds, "on", false))
            .item(toggle(key, "24-Hour Time", |s| &mut s.hours, "12", true));
    }
    let (on_edit, on_remove) = (output.to_string(), output.to_string());
    menu = menu
        .separator()
        .item(MenuItem::label("Edit Widgets…").on_select(move || edit::enter(&on_edit)))
        .separator()
        .item(
            MenuItem::label("Remove Widget")
                .destructive()
                .on_select(move || remove(&on_remove, key)),
        );
    context_menu::show_under(WIDGET_MENU_KEY, menu, namespace(output), at);
}

/// What a card can be set to, each a submenu with its choice checked:
/// what it shows, or for the clock its colour and line; none for a kind
/// with nothing to pick.
pub(super) fn choices(key: WidgetKey) -> Vec<(&'static str, ContextMenu)> {
    let named = |options: &[(&str, &str)]| -> Vec<(String, String)> {
        options
            .iter()
            .map(|(id, name)| (id.to_string(), name.to_string()))
            .collect()
    };
    let (options, field, automatic): (Vec<(String, String)>, Field, &str) = match key.kind {
        WidgetKind::Battery => (
            catalog::devices()
                .into_iter()
                .map(|d| (d.id, d.name))
                .collect(),
            |s| &mut s.device,
            "First Device",
        ),
        WidgetKind::Network => (
            SystemStats::read()
                .interfaces()
                .into_iter()
                .map(|n| (n.to_string(), n.to_string()))
                .collect(),
            |s| &mut s.interface,
            "Default Route",
        ),
        WidgetKind::Disk => (
            SystemStats::read()
                .disks
                .iter()
                .map(|d| (d.mount.clone(), d.mount.clone()))
                .collect(),
            |s| &mut s.mount,
            "Main Disk",
        ),
        WidgetKind::Temperature => (
            SystemStats::read()
                .sensors
                .iter()
                .map(|s| (s.label.clone(), s.label.clone()))
                .collect(),
            |s| &mut s.sensor,
            "Processor",
        ),
        WidgetKind::Clock => {
            let colors = ACCENTS
                .iter()
                .filter_map(|(name, hex)| Some((hex.as_ref()?.to_string(), name.to_string())))
                .collect();
            return vec![
                ("Color", pick(key, colors, |s| &mut s.color, "Accent")),
                (
                    "Line",
                    pick(
                        key,
                        named(&[("thin", "Thin"), ("bold", "Bold")]),
                        |s| &mut s.line,
                        "Regular",
                    ),
                ),
            ];
        }
        _ => return Vec::new(),
    };
    vec![("Show", pick(key, options, field, automatic))]
}

/// A submenu setting `field` of `key`'s settings: `automatic` for none,
/// then each of `options` (its value, its name), the one set checked.
pub(super) fn pick(
    key: WidgetKey,
    options: Vec<(String, String)>,
    field: Field,
    automatic: &str,
) -> ContextMenu {
    let settings = store::with(|store| store.settings(key.id));
    let now = field(&mut settings.clone()).clone();
    let set = |value: Option<String>| {
        let mut next = settings.clone();
        *field(&mut next) = value;
        move || {
            store::with(|store| store.set_settings(key.id, next.clone()));
            changed();
        }
    };
    let mut menu = ContextMenu::new()
        .item(checked(MenuItem::label(automatic), now.is_none()).on_select(set(None)));
    if !options.is_empty() {
        menu = menu.separator();
    }
    for (id, name) in options {
        let on = now.as_deref() == Some(id.as_str());
        menu = menu.item(checked(MenuItem::label(name), on).on_select(set(Some(id))));
    }
    menu
}

/// A menu item that turns `field` of `key`'s settings between `value`
/// and none, checked while it's `value`, or while it's none when
/// `inverted` (12-hour time under "24-Hour Time").
pub(super) fn toggle(
    key: WidgetKey,
    title: &str,
    field: Field,
    value: &'static str,
    inverted: bool,
) -> MenuItem {
    let settings = store::with(|store| store.settings(key.id));
    let set = field(&mut settings.clone()).as_deref() == Some(value);
    checked(MenuItem::label(title), set != inverted).on_select(move || {
        let mut next = settings.clone();
        *field(&mut next) = (!set).then(|| value.to_string());
        store::with(|store| store.set_settings(key.id, next));
        changed();
    })
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

/// The monitor a command means: the one named, else the focused
/// workspace's, else the first with widgets.
pub(super) fn output_for(name: Option<&String>) -> Option<String> {
    if let Some(name) = name {
        return Some(name.clone());
    }
    let focused = Workspaces::read()
        .list()
        .iter()
        .find(|ws| ws.focused())
        .and_then(|ws| ws.output().map(String::from));
    focused.or_else(|| DESKS.with(|desks| desks.all(|desks| desks.keys().next().cloned())))
}
