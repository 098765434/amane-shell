//! Quick Settings' IPC (`quick-settings …`), and what Shell Settings' page asks.

use super::*;

/// `amane-shell` IPC: `quick-settings [toggle [<page>] | open [<page>] | close]`, and what
/// Shell Settings asks.
pub fn run(command: Command) -> String {
    match command {
        Command::Toggle => panel::toggle(Panel::QuickSettings),
        Command::Open => open(Page::Grid),
        // a page by name opens on its own, as from its status icon
        Command::OpenPage(ref name) | Command::TogglePage(ref name) => match Page::named(name) {
            Some(page) if pages::available(page) => {
                if matches!(command, Command::OpenPage(_)) {
                    open_alone(page);
                } else {
                    toggle_alone(page);
                }
            }
            Some(_) => return format!("page {name} has nothing to show on this machine"),
            None => return format!("unknown page {name}; pages: {}", Page::NAMES.join(", ")),
        },
        Command::Close => {
            if is_open(Panel::QuickSettings) {
                close();
            }
        }
        // the Customize window opens with edit mode
        Command::Edit | Command::Customize => {
            open(Page::Grid);
            edit::enter();
        }
        // what Shell Settings' Quick Settings page lists: each control
        // this machine has, and whether it's on the canvas
        Command::Controls => {
            load_layout();
            return ControlId::ALL
                .into_iter()
                .filter(|id| controls::available(*id))
                .map(|id| {
                    let shown = layout::with(|layout| layout.rect_of(id).is_some());
                    format!(
                        "{}\t{}\n",
                        id.name(),
                        if shown { "shown" } else { "hidden" }
                    )
                })
                .collect();
        }
        Command::Show(ref name) | Command::Hide(ref name) => {
            let show = matches!(command, Command::Show(_));
            let Some(id) = ControlId::named(name) else {
                let names: Vec<&str> = ControlId::ALL.into_iter().map(ControlId::name).collect();
                return format!("unknown control {name}; controls: {}", names.join(", "));
            };
            load_layout();
            let placed = layout::with(|layout| {
                let placed = if show {
                    layout.show(id)
                } else {
                    layout.hide(id);
                    true
                };
                layout.save();
                placed
            });
            if !placed {
                QuickSettings::changed();
                return format!("no room for {name}: hide another control first");
            }
        }
        Command::ResetLayout => {
            let ctx = controls::Ctx::new(false);
            layout::replace(layout::Layout::default_for(grid::Canvas::tokens(), &|id| {
                controls::present(id, &ctx)
            }));
        }
    }
    QuickSettings::changed();
    String::from("ok")
}
