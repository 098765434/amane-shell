//! The bar's workspaces, and moving between them with the wheel.

use super::*;

/// The workspace monitor `output` shows, and how many windows it has.
pub(super) fn shown_workspace(output: &str) -> Option<(i64, u32)> {
    Workspaces::read()
        .list()
        .iter()
        .find(|workspace| workspace.output() == Some(output) && workspace.active())
        .map(|workspace| (workspace.id(), workspace.windows()))
}

/// What the strip is painted with: the bar's tint over niri's frost, the
/// glass's own (none where niri draws it), black or white, or nothing.
pub(super) fn fill(style: BarStyle) -> Color {
    match style {
        BarStyle::Clear => Color::TRANSPARENT,
        BarStyle::Frosted => Material::Bar.tint(),
        BarStyle::Glass => Material::Control.fill(),
        BarStyle::Solid => match theme::mode() {
            Mode::Dark => Color::BLACK,
            Mode::Light => Color::WHITE,
        },
    }
}

/// The one shape that says which of monitor `output`'s workspaces shows
/// ([`workspace_shape`]); scrolling over it moves between them.
pub(super) fn workspaces(output: &str) -> Rectangle {
    let width = metrics().item_width;
    let indicator = {
        let workspaces = Workspaces::read();
        let here: Vec<&Workspace> = workspaces
            .list()
            .iter()
            .filter(|workspace| on_output(workspace, output))
            .collect();
        workspace_shape::indicator(output, &here, width)
    };
    let scrolled_on = output.to_string();

    Rectangle::new()
        .width(width)
        .height(Parent)
        .align_child(Start, Center)
        .on_scroll(move |scroll| {
            if status_bar().scroll_workspaces {
                scroll_workspaces(&scrolled_on, scroll);
            }
        })
        .child(indicator)
}

/// A wheel notch, or [`scroll_step`] lines of touchpad, focuses the next
/// workspace down or the previous one up.
pub(super) fn scroll_workspaces(output: &str, scroll: Scroll) {
    let step = SCROLLED.with(|scrolled| scroll_steps(scrolled, &scroll));
    if step == 0 {
        return;
    }

    let workspaces = Workspaces::read();
    let list: Vec<&Workspace> = workspaces
        .list()
        .iter()
        .filter(|workspace| on_output(workspace, output))
        .collect();
    // the one shown on this monitor, focused or not
    let current = list
        .iter()
        .position(|workspace| workspace.focused())
        .or_else(|| list.iter().position(|workspace| workspace.active()));
    if let Some(next) = stepped(current, list.len(), step) {
        let id = list[next].id();
        drop(workspaces);
        Workspaces::focus(id);
    }
}

/// One step along, back (-1) or on (1), for a wheel notch or for
/// [`scroll_step`] lines of touchpad, which gather in `scrolled` till then;
/// 0 while they're still gathering.
pub(super) fn scroll_steps(scrolled: &Cell<f32>, scroll: &Scroll) -> i32 {
    let delta = if scroll.y != 0.0 { scroll.y } else { scroll.x };
    match scroll.source {
        ScrollSource::Wheel | ScrollSource::Tilt => direction(delta),
        ScrollSource::Finger | ScrollSource::Continuous => {
            let total = if scroll.stop {
                0.0
            } else {
                scrolled.get() + delta
            };
            let step = if total.abs() >= scroll_step() {
                direction(total)
            } else {
                0
            };
            scrolled.set(if step == 0 { total } else { 0.0 });
            step
        }
    }
}

/// Whether `workspace` is on monitor `output`; one niri hasn't placed
/// shows on every bar.
pub(super) fn on_output(workspace: &Workspace, output: &str) -> bool {
    workspace.output().is_none_or(|on| on == output)
}

pub(super) fn direction(delta: f32) -> i32 {
    if delta > 0.0 {
        1
    } else if delta < 0.0 {
        -1
    } else {
        0
    }
}

/// The index `step` along from `current` among `len`, if that's another
/// one: it stops at either end.
pub(super) fn stepped(current: Option<usize>, len: usize, step: i32) -> Option<usize> {
    let current = current?;
    let next = current.checked_add_signed(step as isize)?;
    (next < len && next != current).then_some(next)
}
