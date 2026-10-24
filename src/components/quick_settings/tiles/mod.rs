//! The controls and their pages, one file per setting. Each file has the
//! toggle or cell for it (`None` when this machine hasn't got it) and the
//! page it opens. [`grid`] draws them on the canvas where the layout puts
//! them, in Control Center's order by default.

pub mod airplane;
pub mod appearance;
pub mod background;
pub mod bluetooth;
pub mod keyboard;
pub mod media;
pub mod network;
pub mod notifications;
pub mod power;
pub mod session;
pub mod sound;

use std::collections::HashMap;

use amane::{
    Button, Center, Color, Column, End, InputArea, Parent, Rectangle, Row, ScrollArea, Size, Stack,
    Start, Text, Widget,
};

use super::controls::{self, ControlId, Ctx};
use super::detail::DetailPage;
use super::glass_region::{self, STEPS};
use super::grid::{Metrics, Occupancy, Rect, Slot, Span};
use super::tile::{capsule_button, capsule_width, module};
use super::{edit, layout};
use crate::components::glass::{self, Rim};
use crate::components::icons::ICON_ADD;
use crate::components::layout::{boxed, icon, pad, text_height};
use crate::components::list::{self, List, ListItem};
use crate::components::motion::{self, Arrive, Item, Pose};
use crate::components::taskbar::close;
use crate::services::installed;
use crate::tokens::{Styled, quick, sys};

/// How many cells wide the Add list beside the canvas is in edit mode.
const ADD_LIST_CELLS: u8 = 3;

/// Every control's motion is kept under this scope.
pub(super) const SCOPE: &str = "amane.qs.grid";

/// The grid as drawn this frame: its widget, how tall it is, and where the
/// compositor should frost behind it (in the grid's own coordinates).
pub(super) struct Grid {
    pub widget: Box<dyn Widget>,
    /// The canvas's width, and the Add list's beside it in edit mode.
    pub width: f32,
    pub height: f32,
    pub regions: Vec<InputArea>,
}

/// The canvas, and the Edit Controls button under it.
pub(super) fn grid() -> Grid {
    let editing = edit::editing();
    let ctx = Ctx::new(editing);
    layout::load_once(&|id| controls::present(id, &ctx));
    let px = Metrics::tokens();

    // the layout as it is, or as the drag in flight previews it
    let shown_layout = edit::shown_layout();
    let canvas = shown_layout.canvas;
    let placed: Vec<(ControlId, Rect)> = shown_layout.placed().collect();

    let mut widgets: HashMap<ControlId, Box<dyn Widget>> = HashMap::new();
    let mut slots: Vec<(ControlId, Rect, Slot)> = Vec::new();
    for (id, rect) in &placed {
        let widget = match controls::build(*id, rect.span, &ctx) {
            Some(widget) => widget,
            // a control waiting for its hardware takes no room, except in
            // edit mode, where it shows as a placeholder to move or remove
            None if editing => placeholder(*id, px.slot(*rect)),
            None => continue,
        };
        widgets.insert(*id, widget);
        slots.push((*id, *rect, px.slot(*rect)));
    }
    edit::placed(slots.iter().map(|(id, _, slot)| (*id, *slot)).collect());

    let items: Vec<Item<(ControlId, Rect, Slot)>> = slots
        .iter()
        .map(|(id, rect, slot)| {
            let pose = Pose::at(slot.y).moved(slot.x);
            // a control added grows into its cells; one removed shrinks away
            let away = Pose {
                scale: motion::ENTER_SCALE,
                ..pose.faded()
            };
            Item {
                key: id.key(),
                value: (*id, *rect, *slot),
                height: slot.height,
                pose,
                enter: away,
                exit: away,
            }
        })
        .collect();
    let mut shown = motion::animate(SCOPE, items, Arrive::Rest);

    // how many rows show: all of them while editing, else those the
    // controls that built are in
    let rows = if editing {
        canvas.rows
    } else {
        Occupancy::of(canvas, slots.iter().map(|(id, rect, _)| (*id, *rect)))
            .rows_used()
            .max(1)
    };
    let grid_h = px.extent(rows);

    // in edit mode the Add list sits to the left of the canvas, which
    // moves right by its width
    let gap = sys::space::gap::l();
    let canvas_x = if editing {
        px.extent(ADD_LIST_CELLS) + gap
    } else {
        0.0
    };
    let mut regions = Vec::new();
    // every module's glass edge, lit in one pass over the whole grid
    let mut rims: Vec<Rim> = Vec::new();
    let mut layers: Vec<Box<dyn Widget>> = Vec::new();
    // in edit mode the places a control can go show through, as on iOS,
    // and the layer every drag is measured on comes before the controls:
    // amane keeps the target being dragged by its place in the frame's hit
    // areas, so nothing that changes shape under a drag may come before it
    if editing {
        layers.push(edit::ghost_grid(&px, &canvas, &shown_layout.occupancy()));
        layers.push(edit::drop_outline(&px, &shown_layout));
        layers.push(edit::drag_layer());
    }
    // the control in hand draws last, over the rest
    let held = edit::held();
    shown.sort_by_key(|item| Some(item.value.0) == held);
    let mut dressings: Vec<Box<dyn Widget>> = Vec::new();
    for item in shown {
        let (id, rect, slot) = item.value;
        // a control on its way out is drawn as it was until it has gone
        let widget = match widgets.remove(&id) {
            Some(widget) => widget,
            None if item.leaving => match controls::build(id, rect.span, &ctx) {
                Some(widget) => widget,
                None => continue,
            },
            None => continue,
        };
        let pose = edit::held_pose(id).unwrap_or(item.pose);
        let widget = if editing && !item.leaving {
            dressings.push(edit::dressing(id, slot, pose));
            edit::body(widget, slot)
        } else {
            widget
        };
        let frost = edit::frosted(pose, slot);
        let radius = radius_of(rect.span, slot) * pose.scale;
        // a control fading in or out has no frost under it until it's
        // mostly there (niri can't fade frost)
        if pose.opacity >= glass_region::FROST_FROM {
            regions.extend(glass_region::rounded(
                canvas_x + frost.x,
                frost.y,
                frost.width,
                frost.height,
                radius,
                STEPS,
            ));
        }
        // lit as drawn: scaled about its centre, not rounded to pixels
        rims.push(Rim {
            x: canvas_x + pose.x + slot.width * (1.0 - pose.scale) / 2.0,
            y: pose.y + slot.height * (1.0 - pose.scale) / 2.0,
            width: slot.width * pose.scale,
            height: slot.height * pose.scale,
            radius,
            opacity: pose.opacity,
        });
        layers.push(boxed(
            Rectangle::new()
                .width(Size::Fixed(slot.width))
                .height(Size::Fixed(slot.height))
                .fill(Color::TRANSPARENT)
                .translate(pose.x, pose.y)
                .scale(pose.scale)
                .opacity(pose.opacity.clamp(0.0, 1.0))
                .child(Stack::new(vec![widget])),
        ));
    }
    // the badges and handles sit over everything, the held control's last
    layers.extend(dressings);

    let width = px.extent(canvas.cols);
    let cells = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(grid_h))
        .fill(Color::TRANSPARENT)
        .child(Stack::new(layers).width(Parent).height(Parent));
    // a right click on the canvas's empty space edits it
    let cells = if editing {
        cells
    } else {
        cells.on_click(|button| {
            if button == Button::Right {
                edit::enter();
            }
        })
    };

    // the Edit Controls capsule, centred under the canvas; Done is the one
    // action of the moment, in the accent
    let button_h = sys::size::control::height();
    let label = if editing { "Done" } else { "Edit Controls" };
    let button_w = capsule_width(label, false);
    let button_x = canvas_x + (width - button_w) / 2.0;
    regions.extend(glass_region::rounded(
        button_x,
        grid_h + gap,
        button_w,
        button_h,
        button_h / 2.0,
        STEPS,
    ));
    rims.push(Rim {
        x: button_x,
        y: grid_h + gap,
        width: button_w,
        height: button_h,
        radius: button_h / 2.0,
        opacity: 1.0,
    });
    let button = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(button_h))
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .child(capsule_button(
            "amane.qs.edit",
            None,
            label,
            editing,
            edit::toggle,
        ));

    let height = grid_h + gap + button_h;
    let column = Column::new(vec![boxed(cells), boxed(button)])
        .gap(gap)
        .width(Size::Fixed(width));

    // what's been removed, to add back, and the layout's own actions, on a
    // sheet beside the canvas, its bottom in line with the canvas's
    let widget: Box<dyn Widget> = if editing {
        let hidden: Vec<ControlId> = layout::with(|layout| layout.hidden().collect());
        let list_w = px.extent(ADD_LIST_CELLS);
        let (list, list_h) = add_list(hidden, list_w, grid_h);
        regions.extend(glass_region::rounded(
            0.0,
            grid_h - list_h,
            list_w,
            list_h,
            quick::card::radius(),
            STEPS,
        ));
        rims.push(Rim {
            x: 0.0,
            y: grid_h - list_h,
            width: list_w,
            height: list_h,
            radius: quick::card::radius(),
            opacity: 1.0,
        });
        let beside = Rectangle::new()
            .width(Size::Fixed(list_w))
            .height(Size::Fixed(grid_h))
            .fill(Color::TRANSPARENT)
            .align_child(Start, End)
            .child(Stack::new(vec![list]));
        boxed(
            Row::new(vec![boxed(beside), boxed(column)])
                .gap(gap)
                .align(Start),
        )
    } else {
        boxed(column)
    };

    // the glass over everything: one layer lights every module's edge
    let total_w = canvas_x + width;
    let mut lit: Vec<Box<dyn Widget>> = vec![widget];
    lit.extend(glass::rim_layers(&rims).into_iter().map(boxed));
    let widget = boxed(
        Rectangle::new()
            .width(Size::Fixed(total_w))
            .height(Size::Fixed(height))
            .fill(Color::TRANSPARENT)
            .align_child(Start, Start)
            .child(Stack::new(lit).width(Parent).height(Parent)),
    );

    Grid {
        widget,
        width: total_w,
        height,
        regions,
    }
}

/// The controls hidden from the grid, each a row that adds it back (one
/// whose hardware is missing waits on the canvas for it), then Undo
/// Remove and Reset Layout, on a glass sheet `width` wide and no taller
/// than `max_height`; and how tall it is. A long list scrolls.
fn add_list(hidden: Vec<ControlId>, width: f32, max_height: f32) -> (Box<dyn Widget>, f32) {
    let inset = sys::space::gap::s();
    let row_h = sys::size::row::height();
    let row_gap = sys::space::gap::xs();
    let header_h = text_height(sys::r#type::caption());
    let list_rows = hidden.len().max(1) as f32;
    let actions_h = row_h * 2.0 + row_gap;
    let frame_h = sys::space::gap::l() + actions_h + inset * 2.0;
    let wanted = header_h + row_gap + list_rows * row_h + (list_rows - 1.0) * row_gap;
    let list_h = wanted.min((max_height - frame_h).max(row_h));
    let height = list_h + frame_h;

    let (list, actions) = list::with_row_width(width - inset * 2.0, || {
        let items: Vec<ListItem> = hidden
            .into_iter()
            .map(|id| {
                let item = ListItem::new(format!("amane.qs.add.{}", id.name()), id.title());
                // one without its hardware says so in place of the plus;
                // it still goes on the canvas, to wait there
                let item = if controls::available(id) {
                    item.trailing(icon(
                        ICON_ADD,
                        sys::size::icon::m(),
                        sys::color::accent::bg(),
                    ))
                } else {
                    item.trailing(
                        Text::new("Unavailable")
                            .style(sys::r#type::caption())
                            .color(sys::color::content::secondary()),
                    )
                };
                item.on_press(move || edit::add(id))
            })
            .collect();
        let list = List::new(items)
            .header("Add a Control")
            .empty("Every control is on the canvas")
            .build();
        let list = ScrollArea::new("amane.qs.add", list).height(Size::Fixed(list_h));

        let mut undo = ListItem::new("amane.qs.edit.undo", "Undo Remove");
        if edit::can_undo() {
            undo = undo
                .color(sys::color::surface::panel::fg())
                .on_press(edit::undo);
        }
        let reset = ListItem::new("amane.qs.edit.reset", "Reset Layout")
            .color(sys::color::surface::panel::fg())
            .on_press(edit::reset);
        (list, List::new(vec![undo, reset]).build())
    });

    let inner = Rectangle::new()
        .width(Parent)
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .padding(pad(inset, inset))
        .align_child(Start, Start)
        .child(
            Column::new(vec![boxed(list), boxed(actions)])
                .gap(sys::space::gap::l())
                .width(Parent),
        );
    (
        boxed(module(width, height, quick::card::radius(), inner)),
        height,
    )
}

/// What a control waiting for its hardware shows in edit mode: its
/// module's outline and name, dimmed, so it can be moved or removed.
fn placeholder(id: ControlId, slot: Slot) -> Box<dyn Widget> {
    let span = Span {
        cols: 1,
        rows: if slot.height > Metrics::tokens().cell * 1.5 {
            2
        } else {
            1
        },
    };
    let inner = Rectangle::new()
        .width(Size::Fixed(slot.width))
        .height(Size::Fixed(slot.height))
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .child(
            Text::new(id.title())
                .style(quick::r#type::caption())
                .color(sys::color::content::secondary()),
        );
    boxed(module(slot.width, slot.height, radius_of(span, slot), inner).opacity(DIMMED))
}

// TODO(token): `sys.opacity.disabled` once Penpot has it (manual §1.1);
// `buttons/mod.rs` holds the same value.
/// How faint a placeholder is.
const DIMMED: f32 = 0.38;

/// How round a control's module is: one cell tall it's a capsule, taller
/// it has the card's corners.
pub(super) fn radius_of(span: Span, slot: Slot) -> f32 {
    if span.rows > 1 {
        quick::card::radius()
    } else {
        slot.height / 2.0
    }
}

/// `page` with a last row opening `program`, the app that has the rest of
/// its settings, when that's installed.
pub(super) fn settings_link(
    page: DetailPage,
    label: &'static str,
    program: &'static str,
) -> DetailPage {
    if !installed(program) {
        return page;
    }
    page.footer(label, move || {
        close();
        amane::spawn(program);
    })
}

/// "On" or "Off", the subtitle of a tile with nothing more to say.
pub(super) fn on_off(on: bool) -> &'static str {
    if on { "On" } else { "Off" }
}

/// "3 Connected": how many devices or tunnels are, when it's more than one.
pub(super) fn connected_count(n: usize) -> String {
    format!("{n} Connected")
}
