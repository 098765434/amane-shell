//! The preview: the backdrop, the subject with its effect and shadows,
//! moved by dragging, and a corner readout of how fast it draws.
//!
//! One still, preview-sized layer on top takes every press and drag, and
//! everything under it draws without handlers; amane measures a drag
//! against the last frame, so a widget that moved with the drag would see
//! its own motion twice.

use std::path::PathBuf;
use std::time::Instant;

use amane::{
    Center, Color, Fill, Image, Point, Rectangle, Shadow, Size, Stack, Start, Text, Weight, Widget,
    click_count, request_frame,
};
use amane_kit::color_picker::to_color;
use amane_kit::lab::config::{Backdrop, Kind, Subject};
use amane_kit::lab::manifest::{Manifest, SurfaceSpec};
use amane_kit::lab::shadows;
use amane_kit::layout::boxed;
use amane_kit::tokens::sys;

use amane::Service;

use crate::effects::{Effects, Entry};
use crate::{Drag, Lab, change, panel};

/// A press that moves less than this is a click, not a drag.
const CLICK_SLOP: f32 = 3.0;

/// The effect the look is made with, found by its folder's name.
pub fn current<'a>(effects: &'a Effects, lab: &Lab) -> Option<&'a Entry> {
    effects
        .position(&lab.state.effect)
        .and_then(|index| effects.list.get(index))
}

/// The preview's size: the window less its padding, the gap and the controls.
pub fn area() -> (f32, f32) {
    let (width, height) = amane::window_size();
    let inset = sys::space::inset::l() * 2.0;

    (
        (width - inset - sys::space::gap::l() - panel::WIDTH).max(1.0),
        (height - inset).max(1.0),
    )
}

/// How big the subject is; its other measures follow `size`.
pub fn size_of(subject: &Subject) -> (f32, f32) {
    let size = subject.size;

    match subject.kind {
        Kind::Rounded => (size * 1.6, size),
        Kind::Pill => (size * 2.2, size * 0.7),
        Kind::Circle => (size, size),
        Kind::Text => (size * 4.0, size * 1.3),
        Kind::Image => (size * 1.6, size * 1.6),
        Kind::Photo => (size * 2.4, size * 1.5),
        Kind::Fill => area(),
    }
}

/// The subject's corner radius: a surface's own on a rounded rectangle.
pub fn radius_of(lab: &Lab, manifest: Option<&Manifest>) -> f32 {
    let subject = &lab.state.subject;
    let (width, height) = size_of(subject);

    match subject.kind {
        Kind::Rounded => {
            let surface =
                manifest.and_then(|manifest| Some((manifest, manifest.surface.as_ref()?)));

            let param = surface.and_then(|(manifest, surface)| {
                let param = manifest
                    .params
                    .iter()
                    .find(|param| param.key == surface.radius)?;

                Some(lab.value(manifest, param))
            });

            param.unwrap_or(subject.size * 0.22)
        }
        Kind::Pill => height / 2.0,
        Kind::Circle => width / 2.0,
        _ => 0.0,
    }
}

pub fn preview(effects: &Effects, lab: &Lab) -> Rectangle {
    let entry = current(effects, lab);
    let manifest = entry.and_then(|entry| entry.manifest.as_ref().ok());

    let subject_spec = &lab.state.subject;
    let (width, height) = size_of(subject_spec);
    let radius = radius_of(lab, manifest);

    let mut subject = subject(lab);

    if let Some(manifest) = manifest
        && let Some(surface) = &manifest.surface
    {
        subject = as_surface(subject, lab, manifest, surface, radius);
    }

    if let (Some(entry), Some(manifest)) = (entry, manifest) {
        let mut values = lab.rows(manifest);

        if let Some(surface) = &manifest.surface {
            manifest.put_surface(&mut values, surface, width, height, radius);
        }

        if let Some(clicks) = &manifest.clicks {
            let presses: Vec<[f32; 3]> = lab
                .presses
                .iter()
                .map(|(x, y, at)| [*x, *y, at.elapsed().as_secs_f32()])
                .filter(|press| press[2] < clicks.seconds)
                .collect();

            // the ripples' ages move on while any is still showing
            if !presses.is_empty() {
                request_frame();
            }

            manifest.put_clicks(&mut values, &presses);
        }

        if let Some(shader) = &manifest.shader {
            subject = subject
                .shader(entry.dir.join(shader))
                .shader_values(values.clone());
        }

        if let Some(effect) = manifest.effect(&entry.dir, values) {
            subject = subject.effect(effect);
        }
    }

    // the sweep moves every frame while it's on
    if lab.animate {
        request_frame();
    }

    let filled = subject_spec.kind == Kind::Fill;
    let (x, y) = if filled {
        (0.0, 0.0)
    } else {
        (subject_spec.x, subject_spec.y)
    };

    let mut layers: Vec<Box<dyn Widget>> = vec![boxed(
        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Parent)
            .fill(backdrop(lab.state.backdrop)),
    )];

    let shadows = if subject_spec.kind.is_shape() {
        lab.state.shadows.as_slice()
    } else {
        &[]
    };

    if let Some(behind) = shadows::behind(shadows, width, height, radius) {
        layers.push(boxed(centered(behind.translate(x, y))));
    }

    layers.push(boxed(centered(subject.translate(x, y))));

    if let Some(in_front) = shadows::in_front(shadows, width, height, radius) {
        layers.push(boxed(centered(in_front.translate(x, y))));
    }

    layers.push(boxed(readout()));
    layers.push(boxed(drag_layer(filled)));

    /*
     * the clip holds the backdrop's fill as well as the subject: a clip
     * holding an effect draws on its own canvas, and an effect reading the
     * backdrop sees only what that canvas holds
     */
    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .clip()
        .child(Stack::new(layers))
}

fn centered(child: impl Widget + 'static) -> Rectangle {
    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .align_child(Center, Center)
        .child(child)
}

/// The shape the effect is tried on. Only its alpha matters to most
/// effects, so it's drawn in plain white.
fn subject(lab: &Lab) -> Rectangle {
    let subject = &lab.state.subject;
    let size = subject.size;
    let (width, height) = size_of(subject);

    let sized = Rectangle::new().width(width).height(height);

    match subject.kind {
        Kind::Rounded => sized.radius(size * 0.22).fill(Color::WHITE),
        Kind::Pill => sized.radius(height / 2.0).fill(Color::WHITE),
        Kind::Circle => sized.radius(width / 2.0).fill(Color::WHITE),

        Kind::Text => sized.align_child(Center, Center).child(
            Text::new(subject.text.clone())
                .size(size)
                .weight(Weight::Bold)
                .color(Color::WHITE),
        ),

        Kind::Image => sized.fill(Image::contain(subject.image.clone())),
        Kind::Photo => sized.fill(Image::cover(subject.photo.clone())),

        Kind::Fill => Rectangle::new()
            .width(Size::Parent)
            .height(Size::Parent)
            .fill(Color::WHITE),
    }
}

/// The subject as a surface like the shell's: tinted, over a frosted
/// backdrop, with an even glow inside its edge.
fn as_surface(
    subject: Rectangle,
    lab: &Lab,
    manifest: &Manifest,
    surface: &SurfaceSpec,
    radius: f32,
) -> Rectangle {
    let number = |key: &str| {
        let param = manifest.params.iter().find(|param| param.key == key);

        param.map_or(0.0, |param| lab.value(manifest, param))
    };

    let color = |key: &str| {
        let param = manifest.colors.iter().find(|param| param.key == key);

        to_color(param.map_or([0.0; 4], |param| lab.state.color(param)))
    };

    subject
        .radius(radius)
        .fill(color(&surface.tint))
        .blur(number(&surface.frost))
        .shadow(Shadow::inner(color(&surface.glow)).blur(number(&surface.glow_blur)))
}

fn backdrop(backdrop: Backdrop) -> Fill {
    match backdrop {
        Backdrop::Dark => Fill::from("#1e1e2e"),
        Backdrop::Light => Fill::from("#eff1f5"),
        Backdrop::Grey => Fill::from("#7f7f86"),
        Backdrop::Wallpaper => {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();

            Fill::from(Image::cover(home.join("Pictures/overview.jpg")))
        }
    }
}

/// How fast the window draws, and the shader clock when it's held.
fn readout() -> Rectangle {
    let stats = amane::frame_stats(&amane::current_window()).unwrap_or_default();

    let mut text = format!(
        "{:.0} fps · view {:.1} · draw {:.1} · present {:.1} ms",
        stats.fps, stats.view_ms, stats.draw_ms, stats.present_ms
    );

    if amane::shader_time_paused() {
        text.push_str(&format!(" · held at {:.2} s", amane::shader_time()));
    }

    // monospace at 11 px is about 6.7 px a character
    let width = text.chars().count() as f32 * 6.7 + 16.0;

    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .padding(10.0)
        .align_child(Start, Start)
        .child(
            Rectangle::new()
                .width(width)
                .height(22.0)
                .radius(sys::radius::raised())
                .fill(Color::rgba(0, 0, 0, 140))
                .align_child(Center, Center)
                .child(
                    Text::new(text)
                        .font("monospace")
                        .size(11.0)
                        .color(Color::rgba(255, 255, 255, 220)),
                ),
        )
}

/// The still layer that takes every press: dragging moves the subject, a
/// click presses it, a double click puts it back in the middle.
fn drag_layer(filled: bool) -> Rectangle {
    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .fill(Color::TRANSPARENT)
        .cursor(if filled { amane::Pointer } else { amane::Grab })
        .on_drag(move |point| dragged(point, filled))
        .on_release(released)
}

fn dragged(point: Point, filled: bool) {
    let moved_to = {
        let mut lab = Lab::write();

        match &mut lab.drag {
            None => {
                let from = (lab.state.subject.x, lab.state.subject.y);

                lab.drag = Some(Drag {
                    start: point,
                    from,
                    moved: false,
                });

                if click_count() >= 2 && !filled {
                    Some((0.0, 0.0))
                } else {
                    None
                }
            }

            Some(drag) => {
                let (dx, dy) = (point.x - drag.start.x, point.y - drag.start.y);

                if !drag.moved && dx.hypot(dy) < CLICK_SLOP {
                    None
                } else if filled {
                    drag.moved = true;
                    None
                } else {
                    drag.moved = true;
                    Some((drag.from.0 + dx, drag.from.1 + dy))
                }
            }
        }
    };

    if let Some((x, y)) = moved_to {
        change("move", |state| {
            state.subject.x = x;
            state.subject.y = y;
        });
    }
}

fn released(point: Point) {
    let drag = Lab::write().drag.take();

    if drag.is_some_and(|drag| !drag.moved) {
        clicked(point);
    }
}

/// A click on the subject becomes a press the shader sees, for effects that take presses.
fn clicked(point: Point) {
    let effects = Effects::read();
    let mut lab = Lab::write();

    let takes_clicks = current(&effects, &lab)
        .and_then(|entry| entry.manifest.as_ref().ok())
        .is_some_and(|manifest| manifest.clicks.is_some());

    if !takes_clicks {
        return;
    }

    let subject = &lab.state.subject;
    let (area_width, area_height) = area();
    let (width, height) = size_of(subject);

    let (x, y) = if subject.kind == Kind::Fill {
        (0.0, 0.0)
    } else {
        (subject.x, subject.y)
    };

    let left = (area_width - width) / 2.0 + x;
    let top = (area_height - height) / 2.0 + y;

    let across = ((point.x - left) / width, (point.y - top) / height);

    if !(0.0..=1.0).contains(&across.0) || !(0.0..=1.0).contains(&across.1) {
        return;
    }

    // only the ones still showing are kept
    lab.presses
        .retain(|(_, _, at)| at.elapsed().as_secs_f32() < 10.0);
    lab.presses.push((across.0, across.1, Instant::now()));
}

/// Holds the shader clock where it is, or lets it run on.
pub fn toggle_time() {
    if amane::shader_time_paused() {
        amane::pause_shader_time(None);
    } else {
        amane::pause_shader_time(Some(amane::shader_time()));
    }
}
