//! Where the dock's icons go: settling, arranging around one held or new, and laying
//! the row out along the dock.

use super::*;

/// The row for `sections`: each section's icons, a rule `rule` slots wide
/// between one section and the next.
pub(super) fn row(sections: &[Section], rule: f32) -> Vec<Entry> {
    let mut entries = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        let key = section.key();
        if index > 0 {
            entries.push(Entry {
                key: format!("rule:{key}"),
                kind: Kind::Rule,
                section: index,
                width: rule,
            });
        }
        if section.icons.is_empty() {
            entries.push(Entry {
                key: format!("empty:{key}"),
                kind: Kind::Empty,
                section: index,
                width: 1.0,
            });
        }
        for icon in &section.icons {
            entries.push(Entry {
                key: icon.key(&key),
                kind: Kind::Icon(icon.clone()),
                section: index,
                width: 1.0,
            });
        }
    }
    entries
}

/// `sections` with a window let go of where it went, until niri has it
/// there, and with the empty workspace after the last to drop on while a
/// window is held.
pub(super) fn settled(
    dock: &mut Dock,
    mut sections: Vec<Section>,
    spaces: &[Space],
    output: &str,
) -> Vec<Section> {
    if let Some(settle) = &dock.settle {
        if settle.done(&sections) {
            dock.settle = None;
        } else {
            sections = drag::with(&sections, &settle.window, &settle.drop);
        }
    }
    if dock
        .drag
        .as_ref()
        .is_some_and(|drag| drag.moved && drag.window().is_some())
    {
        sections = model::with_last(sections, spaces, output);
    }
    sections
}

/// `sections` as they'd be with the held window let go where the pointer
/// is, the others making way, and the held icon; forgets a drag whose
/// icon has gone.
pub(super) fn arrange(
    dock: &mut Dock,
    sections: Vec<Section>,
    at: Measures,
) -> (Vec<Section>, Option<Held>) {
    dock.target = None;
    let Some(drag) = dock.drag.as_mut().filter(|drag| drag.moved) else {
        return (sections, None);
    };
    // the icon as it is now (its window's title may have changed)
    let now = sections.iter().find_map(|section| {
        let key = section.key();
        section
            .icons
            .iter()
            .find(|icon| icon.key(&key) == drag.key)
            .cloned()
    });
    let Some(icon) = now else {
        dock.drag = None;
        return (sections, None);
    };
    drag.icon = icon.clone();
    let off = drag.pointer.1 < at.top - at.headroom;
    let held = Held {
        key: drag.key.clone(),
        icon,
        x: drag.pointer.0 - drag.grab.0,
        y: drag.pointer.1 - drag.grab.1,
        off,
    };
    let Some(window) = drag.window().cloned() else {
        return (sections, Some(held));
    };
    if off {
        return (sections, Some(held));
    }

    let base = drag::without(&sections, window.id);
    let slots: f32 = base
        .iter()
        .map(|s| s.icons.len().max(1) as f32)
        .sum::<f32>()
        + base.len().saturating_sub(1) as f32 * at.rule;
    let along = along(at, at.card_width(slots), drag.pointer.0);
    let Some(drop) = drag::drop_at(&base, at.rule, along) else {
        return (sections, Some(held));
    };
    let column = base
        .iter()
        .find(|section| section.workspace == Some(drop.workspace))
        .map_or(1, |section| drag::column_index(section, drop.place));
    let preview = drag::with(&base, &window, &drop);
    dock.target = Some((drop, column));
    (preview, Some(held))
}

/// The held icon let go at `y` on the surface: its window to where it was
/// going, or a pinned app pulled off the dock out of it.
pub(super) fn let_go(dock: &mut Dock, y: f32, at: Measures) {
    let Some(drag) = dock.drag.take() else {
        return;
    };
    if !drag.moved {
        return;
    }
    dock.dragged = true;
    match &drag.icon {
        Icon::Window(window) => {
            if let Some((drop, column)) = dock.target.take() {
                let elsewhere =
                    (window.workspace != Some(drop.workspace)).then_some(drop.workspace);
                Windows::move_to(window.id, elsewhere, column);
                dock.settle = Some(Settle::new(window.clone(), drop));
            }
        }
        Icon::Pin(pin) => {
            if y < at.top - at.headroom {
                pins::remove(pin);
            }
        }
    }
}

/// Moves each window that's new since the last frame to the workspace a
/// pin opened its app for, if it turned up elsewhere.
pub(super) fn place_new(windows: &[Win]) {
    SEEN.with_borrow_mut(|seen| {
        for window in windows {
            if seen.insert(window.id)
                && let Some(workspace) = pins::placed(&window.app_id)
                && window.workspace != Some(workspace)
            {
                Windows::move_to_workspace(window.id, workspace);
            }
        }
        seen.retain(|id| windows.iter().any(|window| window.id == *id));
    });
}

/// The row's places at rest, stepping their slides and growth, and the
/// card's width for them; forgets the places that have gone. While
/// `hidden` everything is simply where it goes: nothing on a hidden dock
/// is seen moving, and every frame it moved would be drawn for nothing.
pub(super) fn lay_out(
    dock: &mut Dock,
    entries: Vec<Entry>,
    at: Measures,
    motion: &DockMotion,
    hidden: bool,
) -> Laid {
    dock.items
        .retain(|key, _| entries.iter().any(|entry| entry.key == *key));
    // the first row it lays out is simply there
    let fresh = dock.width.is_none();

    let mut slot = 0.0;
    let mut rests = Vec::with_capacity(entries.len());
    let mut shown = Vec::with_capacity(entries.len());
    for entry in &entries {
        let item = dock.items.entry(entry.key.clone()).or_insert_with(|| Item {
            slot: anim(&motion.item, slot),
            presence: anim(&motion.item, if fresh { 1.0 } else { 0.0 }),
        });
        if hidden {
            item.slot = anim(&motion.item, slot);
            item.presence = anim(&motion.item, 1.0);
        } else {
            item.slot.to(slot);
            item.presence.to(1.0);
        }
        let presence = item.presence.value().clamp(0.0, 1.5);
        let rest = Rest {
            slot: item.slot.value(),
            // a rule only fades, it never swells
            presence: if entry.pointable() { presence } else { 0.0 },
        };
        rests.push(rest);
        shown.push(presence.min(1.0));
        slot += entry.width;
    }

    // where the row ends once everything has arrived, so the card's spring
    // has one place to go rather than a new one each frame of a slide
    let target = at.card_width(slot);
    let width = dock
        .width
        .get_or_insert_with(|| anim(&motion.resize, target));
    if hidden {
        *width = anim(&motion.resize, target);
    } else {
        width.to(target);
    }
    Laid {
        entries,
        rests,
        shown,
        width: width.value(),
    }
}

/// How many slots along the row of a card `width` wide the surface's `x`
/// is, the gap before each icon counting as its own.
pub(super) fn along(at: Measures, width: f32, x: f32) -> f32 {
    let start = at.card_left(width) + at.pad;
    (x - start + at.gap / 2.0) / at.pitch
}

/// `widget` with its top-left corner at (`x`, `y`) on the surface, at
/// `opacity`.
pub(super) fn place(widget: Rectangle, x: f32, y: f32, opacity: f32) -> Box<dyn Widget> {
    boxed(
        Rectangle::new()
            .width(Parent)
            .height(Parent)
            .fill(Color::TRANSPARENT)
            .align_child(Start, Start)
            .child(widget.translate(x, y).opacity(opacity.clamp(0.0, 1.0))),
    )
}
