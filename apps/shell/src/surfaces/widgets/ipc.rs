//! The widgets' IPC (`widgets …`).

use super::*;

/// `widgets list [--json]`, `kinds`, `add <kind> [size] [output]`,
/// `remove <id>`, `set <id> <key> [value]`, `resize <id> <size>`,
/// `move <id> <col> <row>`, `undo`, `reset-layout [output]`, `edit [output]`
/// and `done`, one argument a line:
/// `printf 'widgets\nadd\ncpu\nmedium\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock`.
pub fn run(command: Command) -> String {
    match command {
        Command::List => store::with(|store| {
            store
                .all()
                .map(|(output, entry)| {
                    let at = entry.at.map_or_else(
                        || String::from("hidden"),
                        |at| format!("{},{}", at.col, at.row),
                    );
                    // then its settings, as `key=value`
                    let settings: String = store
                        .settings(entry.id.id)
                        .iter()
                        .map(|(key, value)| format!(" {key}={value}"))
                        .collect();
                    format!(
                        "{output} {} {} {} {at}{settings}",
                        entry.id.id,
                        entry.id.kind.name(),
                        WidgetSize::of(entry.span).name()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }),
        Command::Add { kind, size, output } => add_command(&kind, size, output),
        Command::ListJson => list_json(),
        Command::Kinds => kinds_json(),
        Command::Set { id, key, value } => set(id, &key, value),
        Command::Resize { id, size } => resize_command(id, &size),
        Command::Move { id, col, row } => move_command(id, col, row),
        Command::Undo => {
            if edit::editing_any().is_some() && edit::can_undo() {
                edit::undo();
                String::from("ok")
            } else {
                String::from("error: nothing to undo (Undo Remove is Edit Widgets')")
            }
        }
        Command::Remove(id) => {
            let found = store::with(|store| {
                store
                    .all()
                    .find(|(_, entry)| entry.id.id == id)
                    .map(|(output, entry)| (output.to_string(), entry.id))
            });
            match found {
                Some((output, key)) => {
                    remove(&output, key);
                    String::from("ok")
                }
                None => format!("error: no widget {id}"),
            }
        }
        Command::ResetLayout(output) => match output_for(output.as_ref()) {
            Ok(output) => {
                store::with(|store| store.clear(&output));
                changed();
                String::from("ok")
            }
            Err(error) => error,
        },
        Command::Edit(output) => match output_for(output.as_ref()) {
            Ok(_) if !desktop().show_widgets => String::from(
                "error: widgets are turned off (Emaki Settings, Desktop: Show widgets on the desktop)",
            ),
            Ok(output) => {
                edit::enter(&output);
                String::from("ok")
            }
            Err(error) => error,
        },
        Command::Done => {
            edit::leave_all();
            String::from("ok")
        }
    }
}

/// The monitor widget `id` is on, and its key.
fn find(id: u32) -> Option<(String, WidgetKey)> {
    store::with(|store| {
        store
            .all()
            .find(|(_, entry)| entry.id.id == id)
            .map(|(output, entry)| (output.to_string(), entry.id))
    })
}

/// `set <id> <key> [value]`: a setting its kind has, to a value it takes.
fn set(id: u32, key: &str, value: Option<String>) -> String {
    let Some((_, widget)) = find(id) else {
        return format!("error: no widget {id}");
    };
    let Some(setting) = widget.kind.setting(key) else {
        let keys: Vec<&str> = widget.kind.spec().settings.iter().map(|s| s.key).collect();
        return if keys.is_empty() {
            format!("error: {} has no settings", widget.kind.name())
        } else {
            format!("error: {} has {}", widget.kind.name(), keys.join(", "))
        };
    };
    if !setting.accepts(value.as_deref()) {
        return match &setting.kind {
            SettingKind::Toggle { value: on, .. } => format!("error: {key} is {on} or nothing"),
            SettingKind::Choice { .. } => format!("error: {key} can't be that"),
        };
    }
    store::with(|store| {
        let next = store.settings(id).with(key, value);
        store.set_settings(id, next);
    });
    changed();
    String::from("ok")
}

/// `list --json`: every widget, where it's kept and what it's set to.
fn list_json() -> String {
    let widgets: Vec<serde_json::Value> = store::with(|store| {
        store
            .all()
            .map(|(output, entry)| {
                let settings: serde_json::Map<String, serde_json::Value> = store
                    .settings(entry.id.id)
                    .iter()
                    .map(|(k, v)| (k.to_string(), serde_json::Value::from(v)))
                    .collect();
                serde_json::json!({
                    "output": output,
                    "id": entry.id.id,
                    "kind": entry.id.kind.name(),
                    "title": entry.id.kind.title(),
                    "glyph": entry.id.kind.glyph().to_string(),
                    "category": entry.id.kind.category().name(),
                    "size": WidgetSize::of(entry.span).name(),
                    "at": entry.at.map(|at| [at.col, at.row]),
                    "settings": settings,
                })
            })
            .collect()
    });
    serde_json::Value::from(widgets).to_string()
}

/// `kinds`: every kind this machine has something for, its sizes and what
/// it can be set to (a choice's options as they are now).
fn kinds_json() -> String {
    let kinds: Vec<serde_json::Value> = WidgetKind::ALL
        .into_iter()
        .filter(|kind| kind.available())
        .map(|kind| {
            let settings: Vec<serde_json::Value> = kind
                .spec()
                .settings
                .iter()
                .map(|setting| match &setting.kind {
                    SettingKind::Choice { automatic, options } => serde_json::json!({
                        "key": setting.key,
                        "title": setting.title,
                        "automatic": automatic,
                        "choices": options()
                            .into_iter()
                            .map(|(value, name)| serde_json::json!({"value": value, "name": name}))
                            .collect::<Vec<_>>(),
                    }),
                    SettingKind::Toggle { value, inverted } => serde_json::json!({
                        "key": setting.key,
                        "title": setting.title,
                        "toggle": value,
                        "inverted": inverted,
                    }),
                })
                .collect();
            serde_json::json!({
                "kind": kind.name(),
                "title": kind.title(),
                "blurb": kind.blurb(),
                "sizes": kind.sizes().map(WidgetSize::name).collect::<Vec<_>>(),
                "default": kind.default_size().name(),
                "settings": settings,
            })
        })
        .collect();
    serde_json::Value::from(kinds).to_string()
}

/// `add <kind> [size] [output]`.
fn add_command(kind: &str, size: Option<String>, output: Option<String>) -> String {
    let Some(kind) = WidgetKind::named(kind) else {
        let kinds: Vec<&str> = WidgetKind::ALL.iter().map(|k| k.name()).collect();
        return format!("error: widgets add <{}> [size] [output]", kinds.join("|"));
    };
    // a word that's no size is the monitor, as `widgets add cpu eDP-1`
    let (size, output) = match (size.as_deref().map(WidgetSize::named), output) {
        (Some(Some(size)), output) => (size, output),
        (Some(None), Some(_)) => {
            let sizes: Vec<&str> = kind.sizes().map(WidgetSize::name).collect();
            return format!("error: {} comes in {}", kind.name(), sizes.join(", "));
        }
        (Some(None), None) => (kind.default_size(), size),
        (None, output) => (kind.default_size(), output),
    };
    let output = match output_for(output.as_ref()) {
        Ok(output) => output,
        Err(error) => return error,
    };
    if !desktop().show_widgets || current(&output).is_none() {
        return format!("error: widgets aren't showing on {output}");
    }
    if add(&output, kind, size, None) {
        String::from("ok")
    } else {
        String::from("error: no room for it")
    }
}

/// `resize <id> <size>`.
fn resize_command(id: u32, size: &str) -> String {
    let Some((output, key)) = find(id) else {
        return format!("error: no widget {id}");
    };
    let Some(size) = WidgetSize::named(size).filter(|s| key.kind.sizes().any(|k| k == *s)) else {
        let sizes: Vec<&str> = key.kind.sizes().map(WidgetSize::name).collect();
        return format!("error: {} comes in {}", key.kind.name(), sizes.join(", "));
    };
    match current(&output).and_then(|layout| layout.resized(key, size.span())) {
        Some(next) => {
            commit(&output, &next);
            String::from("ok")
        }
        None => String::from("error: no room for it"),
    }
}

/// `move <id> <col> <row>`: its top-left to that cell, the others making way.
fn move_command(id: u32, col: u8, row: u8) -> String {
    let Some((output, key)) = find(id) else {
        return format!("error: no widget {id}");
    };
    let Some(layout) = current(&output) else {
        return format!("error: widgets aren't showing on {output}");
    };
    let span = layout
        .span_of(key)
        .unwrap_or_else(|| key.kind.default_size().span());
    let at = Place::new(col, row);
    if !layout.canvas.fits(&Rect::new(at, span)) {
        return format!(
            "error: {output} is {} by {} cells",
            layout.canvas.cols, layout.canvas.rows
        );
    }
    match layout.placed_at(key, span, at) {
        Some(next) => {
            commit(&output, &next);
            String::from("ok")
        }
        None => String::from("error: no room there"),
    }
}
