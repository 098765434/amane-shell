//! The widgets' IPC (`widgets …`).

use super::*;

/// `widgets list`, `add <kind> [size] [output]`, `remove <id>`,
/// `reset-layout [output]`, `edit [output]` and `done`, one argument a line:
/// `printf 'widgets\nadd\ncpu\nmedium\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`.
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
                    format!(
                        "{output} {} {} {} {at}",
                        entry.id.id,
                        entry.id.kind.name(),
                        WidgetSize::of(entry.span).name()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }),
        Command::Add { kind, size, output } => {
            let Some(kind) = WidgetKind::named(&kind) else {
                let kinds: Vec<&str> = WidgetKind::ALL.iter().map(|k| k.name()).collect();
                return format!("error: widgets add <{}> [size] [output]", kinds.join("|"));
            };
            // a word that's no size is the monitor, as `widgets add cpu eDP-1`
            let (size, output) = match size.as_deref().and_then(WidgetSize::named) {
                Some(size) => (size, output),
                None => (kind.default_size(), size.or(output)),
            };
            match output_for(output.as_ref()) {
                Some(output) if add(&output, kind, size, None) => String::from("ok"),
                Some(_) => String::from("error: no room for it"),
                None => String::from("error: no monitor"),
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
            Some(output) => {
                store::with(|store| store.clear(&output));
                changed();
                String::from("ok")
            }
            None => String::from("error: no monitor"),
        },
        Command::Edit(output) => match output_for(output.as_ref()) {
            Some(output) => {
                edit::enter(&output);
                String::from("ok")
            }
            None => String::from("error: no monitor"),
        },
        Command::Done => {
            edit::leave_all();
            String::from("ok")
        }
    }
}
