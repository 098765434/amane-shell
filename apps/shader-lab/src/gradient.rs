//! The gradient stop editor: the effect's gradient as a bar with a handle
//! per stop. Drag a handle to move its stop, click one to pick its colour,
//! double-click the bar to add a stop there; − or Delete removes the one
//! selected. Editing turns a preset into the look's own stops.

use std::cell::Cell;

use amane::{Color, Column, Gradient, Point, Radius, Rectangle, Row, Service, Size, Stack, Widget};
use amane_kit::buttons::PushButton;
use amane_kit::color_picker::{ColorPicker, to_color};
use amane_kit::dropdown::Dropdown;
use amane_kit::lab::config::Config;
use amane_kit::lab::manifest::Manifest;
use amane_kit::layout::boxed;
use amane_kit::material;
use amane_kit::tokens::sys;

use crate::effects::Effects;
use crate::{Lab, change, panel, preview};

const BAR: f32 = 26.0;
const HANDLE: f32 = 12.0;

/// How near a press has to be to a stop to take it, in pixels.
const REACH: f32 = 8.0;

/// The most stops the shader's rows hold, and the fewest a gradient has.
const MOST: usize = 8;
const FEWEST: usize = 2;

thread_local! {
    // the stop a press took, while it's held
    static HELD: Cell<Option<usize>> = const { Cell::new(None) };
}

/// The editor and how tall it is, for effects with a gradient.
pub fn editor(manifest: &Manifest, lab: &Lab) -> Option<(Column, f32)> {
    let gradient = manifest.gradient.as_ref()?;
    let stops = lab.state.stops(manifest)?;
    let width = panel::WIDTH;

    let custom = lab.state.gradient.stops.is_some();

    let names: Vec<String> = std::iter::once("Custom".to_owned())
        .chain(gradient.presets.iter().map(|preset| preset.name.clone()))
        .collect();

    let chosen = if custom {
        0
    } else {
        lab.state.gradient.preset + 1
    };

    let presets = Dropdown::new("lab.gradient-preset", names, chosen)
        .width(width)
        .on_select(|index| {
            let Some(preset) = index.checked_sub(1) else {
                return;
            };

            change("gradient", |state| {
                state.gradient.preset = preset;
                state.gradient.stops = None;
            });

            Lab::write().stop = 0;
        })
        .build();

    let selected = lab.stop.min(stops.len() - 1);

    let mut rows: Vec<Box<dyn Widget>> = vec![
        boxed(panel::caption(&gradient.label)),
        boxed(presets),
        boxed(bar(&stops, selected, width)),
        boxed(stop_row(&stops, selected)),
    ];

    let control = sys::size::control::height();
    let mut height =
        panel::caption_height() + control + BAR + HANDLE + control + 4.0 * sys::space::gap::xs();

    if lab.picking.as_deref() == Some("stop") {
        let [_, red, green, blue, alpha] = stops[selected];

        let picker = ColorPicker::new("lab.stop-picker", [red, green, blue, alpha])
            .width(width)
            .on_change(move |color| {
                edit_stops("stop.color", |stops| {
                    let [red, green, blue, alpha] = color;

                    if let Some(stop) = stops.get_mut(selected) {
                        *stop = [stop[0], red, green, blue, alpha];
                    }
                });
            })
            .build();

        rows.push(boxed(picker));
        height += panel::picker_height(width) + sys::space::gap::xs();
    }

    Some((Column::new(rows).gap(sys::space::gap::xs()), height))
}

/// The gradient, and a handle under each stop; the whole is one still target.
fn bar(stops: &[[f32; 5]], selected: usize, width: f32) -> Rectangle {
    let colors: Vec<(f32, Color)> = stops
        .iter()
        .map(|[at, red, green, blue, alpha]| (*at, to_color([*red, *green, *blue, *alpha])))
        .collect();

    let gradient = Rectangle::new()
        .width(width)
        .height(BAR)
        .radius(sys::radius::raised())
        .fill(Gradient::linear(90.0, colors))
        .border(sys::border::hairline(), material::separator());

    let mut layers: Vec<Box<dyn Widget>> = vec![boxed(gradient)];

    for (index, [at, red, green, blue, alpha]) in stops.iter().enumerate() {
        let picked = index == selected;
        let side = if picked { HANDLE + 2.0 } else { HANDLE };

        layers.push(boxed(
            Rectangle::new()
                .width(side)
                .height(side)
                .radius(Radius::Full)
                .fill(to_color([*red, *green, *blue, *alpha]))
                .border(if picked { 2.5 } else { 1.5 }, Color::WHITE)
                .translate(at * (width - side), BAR + 2.0),
        ));
    }

    let positions: Vec<f32> = stops.iter().map(|stop| stop[0]).collect();

    Rectangle::new()
        .width(width)
        .height(BAR + HANDLE + 4.0)
        .fill(Color::TRANSPARENT)
        .cursor(amane::Pointer)
        .on_drag(move |point| pressed(point, &positions, width))
        .on_release(|_| HELD.set(None))
        .child(Stack::new(layers))
}

fn pressed(point: Point, positions: &[f32], width: f32) {
    let at = (point.x / width).clamp(0.0, 1.0);

    // a held stop follows the pointer
    if let Some(index) = HELD.get() {
        edit_stops("stop.move", |stops| {
            if let Some(stop) = stops.get_mut(index) {
                stop[0] = at;
            }
        });

        return;
    }

    let nearest = positions
        .iter()
        .enumerate()
        .map(|(index, position)| (index, (position * width - point.x).abs()))
        .min_by(|a, b| a.1.total_cmp(&b.1));

    match nearest {
        Some((index, distance)) if distance <= REACH => {
            HELD.set(Some(index));

            let mut lab = Lab::write();

            // a second click on the selected stop opens its picker, or closes it
            if lab.stop == index {
                lab.picking = match lab.picking.as_deref() {
                    Some("stop") => None,
                    _ => Some("stop".into()),
                };
            }

            lab.stop = index;
        }

        _ if amane::click_count() >= 2 => add_at(at),
        _ => {}
    }
}

/// The selected stop's place, and adding and removing stops.
fn stop_row(stops: &[[f32; 5]], selected: usize) -> Row {
    let at = stops[selected][0];

    let label = panel::label(&format!("Stop {} at {:.0}%", selected + 1, at * 100.0));

    let add = PushButton::new("lab.stop.add", "+")
        .on_press(move || add_after(selected))
        .build();

    let remove = PushButton::new("lab.stop.remove", "−")
        .on_press(remove_selected)
        .build();

    let pick = PushButton::new("lab.stop.pick", "Colour")
        .on_press(|| {
            let mut lab = Lab::write();

            lab.picking = match lab.picking.as_deref() {
                Some("stop") => None,
                _ => Some("stop".into()),
            };
        })
        .build();

    Row::new(vec![
        boxed(
            Rectangle::new()
                .width(panel::WIDTH - 3.0 * 54.0)
                .height(Size::Fixed(sys::size::control::height()))
                .align_child(amane::Start, amane::Center)
                .child(label),
        ),
        boxed(pick),
        boxed(add),
        boxed(remove),
    ])
    .gap(sys::space::gap::xs())
}

/// A stop halfway to the next one, coloured as the gradient is there.
fn add_after(selected: usize) {
    let Some(stops) = current_stops() else {
        return;
    };

    let here = stops[selected][0];
    let next = stops.get(selected + 1).map_or(1.0, |stop| stop[0]);

    add_at((here + next) / 2.0);
}

fn add_at(at: f32) {
    let Some(stops) = current_stops() else {
        return;
    };

    if stops.len() >= MOST {
        return;
    }

    let [red, green, blue, alpha] = color_at(&stops, at);

    edit_stops("stop.add", |stops| {
        stops.push([at, red, green, blue, alpha])
    });

    // the new stop is wherever sorting put it
    if let Some(stops) = current_stops() {
        Lab::write().stop = stops.iter().position(|stop| stop[0] == at).unwrap_or(0);
    }
}

/// Removes the selected stop, keeping at least two.
pub fn remove_selected() {
    let Some(stops) = current_stops() else {
        return;
    };

    if stops.len() <= FEWEST {
        return;
    }

    let selected = Lab::read().stop.min(stops.len() - 1);

    edit_stops("stop.remove", |stops| {
        stops.remove(selected);
    });

    Lab::write().stop = selected.saturating_sub(1);
}

/// Changes the look's own stops, made from its preset's the first time, and
/// keeps them in order, following the selected stop to where it lands.
fn edit_stops(key: &str, edit: impl FnOnce(&mut Vec<[f32; 5]>)) {
    let Some(mut stops) = current_stops() else {
        return;
    };

    let selected = Lab::read().stop.min(stops.len().saturating_sub(1));

    edit(&mut stops);

    let kept = stops.get(selected).copied();

    stops.sort_by(|a, b| a[0].total_cmp(&b[0]));

    if let Some(kept) = kept {
        let index = stops
            .iter()
            .position(|stop| *stop == kept)
            .unwrap_or(selected);

        Lab::write().stop = index;

        if HELD.get().is_some() {
            HELD.set(Some(index));
        }
    }

    change(key, |state: &mut Config| state.gradient.stops = Some(stops));
}

fn current_stops() -> Option<Vec<[f32; 5]>> {
    let effects = Effects::read();
    let lab = Lab::read();

    let manifest = preview::current(&effects, &lab)?.manifest.as_ref().ok()?;

    lab.state.stops(manifest)
}

/// The gradient's colour at `at`, between its stops.
fn color_at(stops: &[[f32; 5]], at: f32) -> [f32; 4] {
    let colour = |stop: &[f32; 5]| [stop[1], stop[2], stop[3], stop[4]];

    let Some(first) = stops.first() else {
        return [0.0, 0.0, 0.0, 1.0];
    };

    if at <= first[0] {
        return colour(first);
    }

    for pair in stops.windows(2) {
        let (from, to) = (&pair[0], &pair[1]);

        if at <= to[0] {
            let t = ((at - from[0]) / (to[0] - from[0]).max(1e-5)).clamp(0.0, 1.0);
            let (a, b) = (colour(from), colour(to));

            return [0, 1, 2, 3].map(|lane| a[lane] + (b[lane] - a[lane]) * t);
        }
    }

    stops.last().map_or([0.0, 0.0, 0.0, 1.0], colour)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_stop_takes_the_colour_between_its_neighbours() {
        let stops = [[0.0, 1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0, 1.0]];

        assert_eq!(color_at(&stops, 0.5), [0.5, 0.0, 0.5, 1.0]);
        assert_eq!(color_at(&stops, -1.0), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(color_at(&stops, 2.0), [0.0, 0.0, 1.0, 1.0]);
    }
}
