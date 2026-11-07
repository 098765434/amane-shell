//! How a desktop draws: the dimming and the switch between workspaces, the cards and their
//! shadows, the one held.

use super::*;

/// Whether the cards fade back now, as the widget style says: by default
/// while a window is open on the monitor's workspace, and part way through
/// a switch between one with windows and one without, as far as the switch
/// has gone; never while editing. `settled` lands the fade where it's going
/// at once. 0 … 1, faded.
pub(super) fn dimming(output: &str, editing: bool, settled: bool) -> f32 {
    let style = desktop().widget_style;
    let automatic = !editing && style == WidgetStyle::Automatic;
    let busy = !editing
        && match style {
            WidgetStyle::Full => false,
            WidgetStyle::Dim => true,
            WidgetStyle::Automatic => Workspaces::read()
                .list()
                .iter()
                .any(|ws| ws.output() == Some(output) && ws.active() && ws.windows() > 0),
        };
    let target = if busy { 1.0 } else { 0.0 };
    let switching = automatic.then(|| switching(output)).flatten();
    with_desk(output, |desk| {
        // the switch moves the fade itself, which carries on from there once it lands
        if let Some(dim) = switching {
            desk.dim = motion::fade(dim, motion::FADE);
            return dim;
        }
        if settled {
            desk.dim = motion::fade(target, motion::FADE);
        }
        desk.dim.to(target);
        desk.dim.value().clamp(0.0, 1.0)
    })
}

/// The fade part way through a workspace switch on `output`: between the
/// two workspaces it's between, each faded if it has windows, by how far
/// the switch has gone. None while no switch is under way there.
pub(super) fn switching(output: &str) -> Option<f32> {
    let switch = WorkspaceSwitch::read();
    if switch.output() != output {
        return None;
    }
    let workspaces = Workspaces::read();
    let here: Vec<_> = workspaces
        .list()
        .iter()
        .filter(|ws| ws.output() == Some(output))
        .collect();
    // kept to the ends and settled on a landing, as the bar's shape is
    let at = f64::from(status_bar::switch_position(&switch, output, here.len())?);
    if at == at.round() {
        return None;
    }
    // the switch counts from 0, niri's workspace indices from 1
    let busy = |position: f64| {
        let index = position as u32 + 1;
        let busy = here
            .iter()
            .any(|ws| ws.index() == index && ws.windows() > 0);
        if busy { 1.0 } else { 0.0 }
    };
    let below = at.floor();
    let along = (at - below) as f32;
    Some(busy(below) * (1.0 - along) + busy(below + 1.0) * along)
}

/// The soft shadow under every card (Quick Settings' `quick.shadow`).
pub(super) fn shadow() -> Shadow {
    Shadow::drop(Color::rgba(0, 0, 0, 0x29))
        .blur(24.0)
        .offset(0.0, 8.0)
}

/// A card's glass, `width` by `height`, holding `content`.
pub(super) fn card(width: f32, height: f32, content: Box<dyn Widget>) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(height)
        .radius(RADIUS)
        .fill(Material::Control.fill())
        .shadow(shadow())
        .clip()
        .child(Stack::new(vec![content]))
}

pub(super) fn view(
    output: &str,
    geometry: &Geometry,
    layout: &Layout<WidgetKey>,
    dim: f32,
    editing: bool,
) -> View {
    let px = metrics();
    let (ox, oy) = geometry.origin;
    let placed: Vec<(WidgetKey, Rect)> = layout.placed().collect();
    let slots: Vec<(WidgetKey, Slot)> = placed.iter().map(|(k, r)| (*k, px.slot(*r))).collect();
    with_desk(output, |desk| desk.slots = slots.clone());
    let held = held(output);

    let items: Vec<Item<(WidgetKey, Rect, Slot)>> = placed
        .iter()
        .zip(&slots)
        .map(|((key, rect), (_, slot))| {
            let pose = Pose::at(slot.y).moved(slot.x);
            let away = Pose {
                scale: motion::scale(motion::ENTER_SCALE),
                ..pose.faded()
            };
            Item {
                key: u64::from(key.id),
                value: (*key, *rect, *slot),
                height: slot.height,
                pose,
                enter: away,
                exit: away,
            }
        })
        .collect();
    let mut shown = motion::animate(&scope(output), items, Arrive::Rest);
    // the card in hand draws over the rest
    shown.sort_by_key(|item| held.map(|(k, _)| k) == Some(item.value.0));

    let mut layers: Vec<Box<dyn Widget>> =
        vec![boxed(still_layer(output, *geometry, held.is_some()))];
    if editing {
        layers.push(edit::drop_outline(output, &px, geometry, layout));
    }
    let mut frost = Vec::new();
    let mut rims: Vec<Rim> = Vec::new();
    let mut dressings = Vec::new();
    // the card in hand, over everything, the gallery too
    let mut over: Vec<Box<dyn Widget>> = Vec::new();
    let fade = 1.0 - DIM * dim;
    for item in shown {
        let (key, rect, slot) = item.value;
        let mut pose = item.pose;
        if let Some((held_key, at)) = held
            && held_key == key
            && !item.leaving
        {
            pose = Pose {
                scale: DRAG_SCALE,
                ..Pose::at(at.1).moved(at.0)
            };
        }
        let opacity = (pose.opacity * fade).clamp(0.0, 1.0);
        let size = WidgetSize::of(rect.span);
        let settings = store::with(|store| store.settings(key.id));
        let glass = glass_id(output, key);
        let built = Material::Control.adapted(glass, || {
            card(
                slot.width,
                slot.height,
                catalog::build(key, size, slot.width, slot.height, &settings),
            )
        });
        let x = ox + pose.x;
        let y = oy + pose.y;
        let drawn = Rim {
            x: x + slot.width * (1.0 - pose.scale) / 2.0,
            y: y + slot.height * (1.0 - pose.scale) / 2.0,
            width: slot.width * pose.scale,
            height: slot.height * pose.scale,
            radius: RADIUS * pose.scale,
            opacity,
        };
        frost.push(
            Material::Control
                .adapted(glass, || {
                    Material::Control.shape(
                        drawn.x,
                        drawn.y,
                        drawn.width,
                        drawn.height,
                        drawn.radius,
                        if amane::blur_fades() {
                            opacity
                        } else {
                            pose.opacity
                        },
                    )
                })
                .id(glass),
        );
        let in_hand = held.is_some_and(|(k, _)| k == key) && !item.leaving;
        let drawn_card = boxed(
            Rectangle::new()
                .width(slot.width)
                .height(slot.height)
                .fill(Color::TRANSPARENT)
                .translate(x, y)
                .scale(pose.scale)
                .opacity(opacity)
                .child(built),
        );
        if in_hand {
            over.push(drawn_card);
        } else {
            rims.push(drawn);
            layers.push(drawn_card);
        }
        if editing && !item.leaving {
            dressings.push(edit::dressing(output, key, slot, Pose { x, y, ..pose }));
        }
    }
    if !Material::Control.compositor_draws() {
        layers.extend(glass::rim_layers(&rims).into_iter().map(boxed));
    }
    layers.extend(dressings);
    if editing {
        let (gallery, gallery_frost) = gallery::sheet(output, geometry);
        layers.push(gallery);
        frost.extend(gallery_frost);
        if let Some((ghost, ghost_frost)) = gallery::ghost(output, geometry) {
            layers.push(ghost);
            frost.extend(ghost_frost);
        }
    }
    layers.extend(over);

    View {
        root: Stack::new(layers),
        frost,
    }
}

/// The card in hand on `output`, and where its top-left is in the
/// canvas's pixels.
pub(super) fn held(output: &str) -> Option<(WidgetKey, (f32, f32))> {
    with_desk(output, |desk| match desk.session.as_ref().map(|s| s.drag) {
        Some(Drag::Move { id, at, .. }) => Some((id, at)),
        _ => None,
    })
}

/// The box round every card in `layout`, in the canvas's pixels: x, y,
/// width, height. Nothing when there are no cards.
pub(super) fn cards_extent(layout: &Layout<WidgetKey>) -> (f32, f32, f32, f32) {
    let px = metrics();
    let mut bounds: Option<(f32, f32, f32, f32)> = None;
    for (_, rect) in layout.placed() {
        let slot = px.slot(rect);
        let (left, top, right, bottom) =
            (slot.x, slot.y, slot.x + slot.width, slot.y + slot.height);
        bounds = Some(match bounds {
            None => (left, top, right, bottom),
            Some((l, t, r, b)) => (l.min(left), t.min(top), r.max(right), b.max(bottom)),
        });
    }
    match bounds {
        Some((l, t, r, b)) => (l, t, r - l, b - t),
        None => (0.0, 0.0, 0.0, 0.0),
    }
}

/// The drag in flight on `output`.
pub(super) fn drag(output: &str) -> Option<Drag<WidgetKey>> {
    with_desk(output, |desk| desk.session.as_ref().map(|s| s.drag))
}
