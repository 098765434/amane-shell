//! The controls and their pages, one file per setting. Each file has the
//! toggle or cell for it (`None` when this machine hasn't got it) and the
//! page it opens. [`grid`] draws them on the canvas where the layout puts
//! them, in the default order unless moved.

pub mod airplane;
pub mod appearance;
pub mod background;
pub mod bluetooth;
pub mod bluetooth_text;
pub mod failure;
pub mod keep_awake;
pub mod keyboard;
pub mod media;
pub mod network;
pub mod notifications;
pub mod phone;
pub mod phone_screen;
pub mod power;
pub mod session;
pub mod software_update;
pub mod sound;
pub mod tailscale;

use std::collections::HashMap;
use std::time::Instant;

use amane::{Blend, BlurShape, Button, Center, Column, Parent, Stack, Start, Text, Widget};

use super::controls::{self, ControlId, Ctx};
use super::detail::DetailPage;
use super::grid::{Metrics, Occupancy, Rect, Slot, Span};
use super::tile::{capsule_button, capsule_width, module};
use super::{edit, layout};
use crate::chrome::panel::close;
use crate::services::installed;
use crate::surfaces::quick_settings::grid::FromTokens as _;
use crate::tokens::{Styled, quick, sys};
use emaki_kit::glass::{self, Rim};
use emaki_kit::glass_region;
use emaki_kit::layout::{boxed, frame};
use emaki_kit::material::{self, Level, Material};
use emaki_kit::motion::{self, Arrive, Item, Pose};

/// Every control's motion is kept under this scope.
pub(super) const SCOPE: &str = "emaki.qs.grid";

// The cascade (`quick.open.cascade*`): how much later, in milliseconds,
// the module farthest from the bar's corner sets off than the nearest as
// the card opens, and how small a module is as it comes in, and how far
// below.

/// How the grid moves this frame, beyond its controls' own motion.
#[derive(Clone, Copy, Default)]
pub(super) struct Motion {
    /// When the card began to open, for the modules to arrive in a wave
    /// out from the bar's corner.
    pub opened: Option<Instant>,
    /// The controls a page growing over the grid comes out of (the first
    /// on the grid), and how much of that one still shows: its frost has
    /// gone to the page's sheet.
    pub focus: Option<(&'static [ControlId], f32)>,
}

impl Motion {
    /// How far a module whose centre is `distance` (0 … 1 of the canvas's
    /// diagonal) from the bar's corner has arrived: 0 … 1, a little past on
    /// the spring.
    fn arrival(self, distance: f32) -> f32 {
        let Some(opened) = self.opened else {
            return 1.0;
        };
        let elapsed = opened.elapsed().as_secs_f32() - quick::open::cascade() / 1000.0 * distance;
        let progress = elapsed / motion::settle().as_secs_f32();
        if progress < 1.0 {
            amane::request_frame();
        }
        motion::spring_curve(progress)
    }
}

/// `pose` as the cascade has it, `arrived` of the way in.
fn arriving(pose: Pose, arrived: f32) -> Pose {
    Pose {
        y: pose.y + (1.0 - arrived) * motion::travel(quick::open::cascade_rise()),
        scale: pose.scale * f32::blend(motion::scale(quick::open::cascade_scale()), 1.0, arrived),
        opacity: pose.opacity * arrived.clamp(0.0, 1.0),
        ..pose
    }
}

/// The grid as drawn this frame: its widget, how tall it is, and where the
/// compositor should frost behind it (in the grid's own coordinates).
pub(super) struct Grid {
    pub widget: Box<dyn Widget>,
    /// The canvas's width.
    pub width: f32,
    pub height: f32,
    pub shapes: Vec<BlurShape>,
    /// Each control where it rests, with its corners' radius, in the
    /// grid's own coordinates: where its page grows from.
    pub rests: Vec<(ControlId, Slot, f32)>,
}

impl Grid {
    /// Where the first of `ids` on the grid rests, and its corners' radius.
    pub fn rest(&self, ids: &[ControlId]) -> Option<(Slot, f32)> {
        ids.iter().find_map(|id| {
            self.rests
                .iter()
                .find(|(placed, _, _)| placed == id)
                .map(|(_, slot, radius)| (*slot, *radius))
        })
    }
}

/// The canvas, and the Edit Controls button under it.
pub(super) fn grid(look: Motion) -> Grid {
    let editing = edit::editing();
    let ctx = Ctx::new(editing);
    layout::load_once(&|id| controls::present(id, &ctx));
    let px = Metrics::tokens();

    // the layout as it is, or as the drag in flight previews it
    let shown_layout = edit::shown_layout();
    let canvas = shown_layout.canvas;
    let placed: Vec<(ControlId, Rect)> = shown_layout.placed().collect();

    // the modules are one panel of glass: alike, in the theme's tint, their
    // labels flipped and tint thinned by the backdrop behind all of them
    let panel = panel_id();
    let members: Vec<u32> = placed.iter().map(|(id, _)| glass_id(*id)).collect();

    let mut widgets: HashMap<ControlId, Box<dyn Widget>> = HashMap::new();
    let mut slots: Vec<(ControlId, Rect, Slot)> = Vec::new();
    for (id, rect) in &placed {
        let built = Material::Control
            .adapted_group(panel, &members, || controls::build(*id, rect.span, &ctx));
        let widget = match built {
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
                scale: motion::scale(motion::enter_scale()),
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

    let gap = sys::space::gap::l();
    let mut shapes = Vec::new();
    // every module's glass edge, lit in one pass over the whole grid
    let mut rims: Vec<Rim> = Vec::new();
    let mut layers: Vec<Box<dyn Widget>> = Vec::new();
    // in edit mode the places a control can go show through.
    // The layer every drag is measured on is the surface's own, under the
    // card and the Customize window alike (`customize::drag_layer`).
    if editing {
        layers.push(edit::ghost_grid(&px, &canvas, &shown_layout.occupancy()));
        layers.push(edit::drop_outline(&px, &shown_layout));
    }
    // the control in hand draws last, over the rest
    let held = edit::held();
    shown.sort_by_key(|item| Some(item.value.0) == held);
    let mut dressings: Vec<Box<dyn Widget>> = Vec::new();
    let mut rests = Vec::new();
    let focus = look.focus.and_then(|(ids, shows)| {
        ids.iter()
            .find(|id| slots.iter().any(|(placed, _, _)| placed == *id))
            .map(|id| (*id, shows))
    });
    let width = px.extent(canvas.cols);
    // the cascade spreads out from the canvas's bottom-right corner, by the bar
    let diagonal = width.hypot(grid_h).max(1.0);
    for item in shown {
        let (id, rect, slot) = item.value;
        // a control on its way out is drawn as it was until it has gone
        let widget = match widgets.remove(&id) {
            Some(widget) => widget,
            None if item.leaving => {
                match Material::Control
                    .adapted_group(panel, &members, || controls::build(id, rect.span, &ctx))
                {
                    Some(widget) => widget,
                    None => continue,
                }
            }
            None => continue,
        };
        let distance = (width - (slot.x + slot.width / 2.0))
            .hypot(grid_h - (slot.y + slot.height / 2.0))
            / diagonal;
        let mut pose = arriving(
            edit::held_pose(id).unwrap_or(item.pose),
            look.arrival(distance),
        );
        // the control whose page grows out of it gives its frost to the page
        let focused = focus.filter(|(focus, _)| *focus == id);
        if let Some((_, shows)) = focused {
            pose.opacity *= shows;
        }
        // the control in hand is drawn by the surface over the card and
        // the Customize window both (`customize::ghost`), frost and all; its
        // place here stays, clear, so nothing else in the frame moves
        let in_hand = held == Some(id);
        if in_hand {
            pose.opacity = 0.0;
        }
        let widget = if editing && !item.leaving {
            dressings.push(edit::dressing(id, slot, pose));
            edit::body(widget, slot)
        } else {
            widget
        };
        if !item.leaving {
            rests.push((id, slot, radius_of(id, rect.span, slot)));
        }
        let radius = radius_of(id, rect.span, slot) * pose.scale;
        // frosted and lit as drawn: scaled about its centre, fading with it
        let drawn = Rim {
            x: pose.x + slot.width * (1.0 - pose.scale) / 2.0,
            y: pose.y + slot.height * (1.0 - pose.scale) / 2.0,
            width: slot.width * pose.scale,
            height: slot.height * pose.scale,
            radius,
            opacity: pose.opacity.clamp(0.0, 1.0),
        };
        if focused.is_none() && !in_hand {
            let glass = glass_id(id);
            shapes.push(
                Material::Control
                    .adapted_group(panel, &members, || frost(&drawn))
                    .id(glass),
            );
        }
        if !in_hand {
            rims.push(drawn);
        }
        layers.push(boxed(
            frame(slot.width, slot.height)
                .translate(pose.x, pose.y)
                .scale(pose.scale)
                .opacity(pose.opacity.clamp(0.0, 1.0))
                .child(Stack::new(vec![widget])),
        ));
    }
    // the badges and handles sit over everything, the held control's last
    layers.extend(dressings);

    let cells = frame(width, grid_h).child(Stack::new(layers).width(Parent).height(Parent));
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
    // action of the moment, glass tinted with the accent
    let button_h = sys::size::control::height();
    let label = if editing { "Done" } else { "Edit Controls" };
    let button_w = capsule_width(label, false);
    let button_x = (width - button_w) / 2.0;
    // it arrives last, after the controls above it
    let pose = arriving(Pose::at(0.0), look.arrival(1.0));
    let capsule = Rim {
        x: button_x + button_w * (1.0 - pose.scale) / 2.0,
        y: grid_h + gap + pose.y + button_h * (1.0 - pose.scale) / 2.0,
        width: button_w * pose.scale,
        height: button_h * pose.scale,
        radius: button_h * pose.scale / 2.0,
        opacity: pose.opacity,
    };
    let capsule_glass = if editing {
        Material::Prominent
    } else {
        Material::Control
    };
    shapes.push(frost_in(&capsule, capsule_glass));
    rims.push(capsule);
    let button = frame(width, button_h).align_child(Center, Center).child(
        frame(button_w, button_h)
            .translate(0.0, pose.y)
            .scale(pose.scale)
            .opacity(pose.opacity)
            .child(capsule_button(
                "emaki.qs.edit",
                None,
                label,
                editing,
                edit::toggle,
            )),
    );

    let height = grid_h + gap + button_h;
    let column = Column::new(vec![boxed(cells), boxed(button)])
        .gap(gap)
        .width(width);

    let widget: Box<dyn Widget> = boxed(column);

    // the glass over everything: one layer lights every module's edge
    let total_w = width;
    let mut lit: Vec<Box<dyn Widget>> = vec![widget];
    if !Material::Control.compositor_draws() {
        lit.extend(glass::rim_layers(&rims).into_iter().map(boxed));
    }
    let widget = boxed(
        frame(total_w, height)
            .align_child(Start, Start)
            .child(Stack::new(lit).width(Parent).height(Parent)),
    );

    Grid {
        widget,
        width: total_w,
        height,
        shapes,
        rests,
    }
}

/// The id the panel's labels are decided under.
fn panel_id() -> u32 {
    glass_region::shape_id("emaki.quick-settings")
}

/// The id of control `id`'s glass, for niri's readings of its backdrop.
fn glass_id(id: ControlId) -> u32 {
    glass_region::shape_id(&format!("emaki.quick-settings.{id:?}"))
}

/// The frost behind a module, where it's drawn.
fn frost(drawn: &Rim) -> BlurShape {
    frost_in(drawn, Material::Control)
}

/// The frost behind a piece of `material`, where it's drawn.
fn frost_in(drawn: &Rim, material: Material) -> BlurShape {
    material.shape(
        drawn.x,
        drawn.y,
        drawn.width,
        drawn.height,
        drawn.radius,
        drawn.opacity,
    )
}

/// What a control waiting for its hardware shows in edit mode and in the
/// Customize window: its module's outline and name, dimmed, so it can be
/// moved or removed.
pub(super) fn placeholder(id: ControlId, slot: Slot) -> Box<dyn Widget> {
    let span = Span {
        cols: 1,
        rows: if slot.height > Metrics::tokens().cell * 1.5 {
            2
        } else {
            1
        },
    };
    let inner = frame(slot.width, slot.height)
        .align_child(Center, Center)
        .child(
            Text::new(id.title())
                .style(quick::r#type::caption())
                .color(material::label(Level::Secondary)),
        );
    boxed(
        module(slot.width, slot.height, radius_of(id, span, slot), inner)
            .opacity(quick::placeholder::opacity()),
    )
}

/// How round a control's module is: one cell tall it's a capsule, taller
/// it has the card's corners, and the slider cards (Sound, Display) keep
/// the card's corners at any height. The frost and the glass edge follow
/// this, so it must match what each control draws (`slider_card::card`).
pub(super) fn radius_of(id: ControlId, span: Span, slot: Slot) -> f32 {
    if span.rows > 1 || matches!(id, ControlId::Sound | ControlId::Display) {
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
        crate::services::launch(&[program]);
    })
}

/// `page` with a last row opening Emaki Settings on its page `place`, or
/// `fallback` (the app with the rest of its settings) without Emaki Settings.
pub(super) fn emaki_settings_link(
    page: DetailPage,
    label: &'static str,
    place: &'static str,
    fallback: &'static str,
) -> DetailPage {
    let settings = emaki_kit::app_ids::SETTINGS;
    if !installed(settings) {
        return settings_link(page, label, fallback);
    }
    page.footer(label, move || {
        close();
        amane::Launch::argv(&[settings, "--page", place]).activated();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frost_takes_each_controls_own_corners() {
        let px = Metrics::tokens();
        let wide = px.slot(Rect {
            at: super::super::grid::Place { col: 0, row: 0 },
            span: Span { cols: 4, rows: 1 },
        });
        // the slider cards keep the card's corners, a toggle one cell tall
        // is a capsule
        assert_eq!(
            radius_of(ControlId::Sound, Span { cols: 4, rows: 1 }, wide),
            quick::card::radius()
        );
        assert_eq!(
            radius_of(ControlId::Focus, Span { cols: 4, rows: 1 }, wide),
            wide.height / 2.0
        );
    }
}
