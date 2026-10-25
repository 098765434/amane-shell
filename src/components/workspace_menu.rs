//! A submenu of workspaces to send something to: the dock moves a window
//! to one, the launcher opens an app on one. Each is named as niri names
//! it, or "Workspace N".

use amane::{Service, Windows, Workspace, Workspaces};

use crate::components::context_menu::{ContextMenu, MenuItem};

/// What a workspace is called in a menu.
fn label(workspace: &Workspace) -> String {
    workspace
        .name()
        .map(String::from)
        .unwrap_or_else(|| format!("Workspace {}", workspace.index()))
}

/// Every workspace, `pick` run with the id of the one chosen.
pub(crate) fn all(pick: impl Fn(i64) + Clone + 'static) -> ContextMenu {
    let workspaces = Workspaces::read();
    workspaces
        .list()
        .iter()
        .fold(ContextMenu::new(), |menu, workspace| {
            menu.item(item(label(workspace), workspace.id(), pick.clone()))
        })
}

/// The workspaces of the monitor in focus, in order. The empty one niri
/// keeps after the last is "New Workspace".
pub(crate) fn on_focused_output(pick: impl Fn(i64) + Clone + 'static) -> ContextMenu {
    let workspaces = Workspaces::read();
    let output = workspaces
        .list()
        .iter()
        .find(|workspace| workspace.focused())
        .and_then(Workspace::output)
        .map(String::from);
    let mut here: Vec<&Workspace> = workspaces
        .list()
        .iter()
        .filter(|workspace| output.is_none() || workspace.output() == output.as_deref())
        .collect();
    here.sort_by_key(|workspace| workspace.index());

    let windows = Windows::read();
    let empty = |workspace: &Workspace| {
        !windows
            .list()
            .iter()
            .any(|window| window.workspace() == Some(workspace.id()))
    };
    let last = here.len().saturating_sub(1);
    here.iter()
        .enumerate()
        .fold(ContextMenu::new(), |menu, (at, workspace)| {
            let name = if at == last && empty(workspace) && workspace.name().is_none() {
                String::from("New Workspace")
            } else {
                label(workspace)
            };
            menu.item(item(name, workspace.id(), pick.clone()))
        })
}

fn item(name: String, id: i64, pick: impl Fn(i64) + 'static) -> MenuItem {
    MenuItem::label(name).on_select(move || pick(id))
}
