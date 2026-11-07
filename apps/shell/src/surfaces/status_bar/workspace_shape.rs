//! The workspace indicator: one shape whose form says which workspace a
//! monitor shows. The first is a circle, the second a square, the third a
//! triangle, then a diamond, a pentagon, a hexagon and a star, and after
//! those more polygons and stars with more points.
//!
//! It morphs along with niri's own switch, a swipe's finger included, from
//! the fork's `WorkspaceSwitchProgress` ([`WorkspaceSwitch`]). On a monitor
//! that isn't focused, which that leaves out, it springs to the workspace
//! shown instead.
//!
//! The shape is one closed straight-sided `Path`, so it draws as a single
//! quad outline and never reaches vello.

use std::f32::consts::{PI, TAU};
use std::time::Duration;

use crate::chrome::per_output::PerOutput;

use amane::{
    Animation, Canvas, Center, Color, Easing, Path, Rectangle, Service, Shape as _, Size,
    Workspace, WorkspaceSwitch,
};

use crate::tokens::sys;
use amane_kit::buttons::DrawnButton;
use amane_kit::material::{self, Level};

/// How many points go round every shape. 120 is a multiple of 3, 4, 5, 6
/// and 8, so those shapes' corners fall on a point.
const POINTS: usize = 120;

/// The box it's drawn in: the bar's text line.
const BOX: f32 = super::size::LINE_HEIGHT;

/// How much area each shape covers: a circle 13 px across, so a square or
/// a star looks as heavy as the circle rather than the same width.
/// TODO(token): `bar.workspace.shape.size` (the box) and `.area`.
const AREA: f32 = PI * 6.5 * 6.5;

/// How far either way of a point the corners are softened, in points.
const SOFTEN: usize = 2;

/// How far it turns while halfway between two shapes, at most, in radians.
const TURN: f32 = PI / 12.0;

thread_local! {
    // where each monitor's shape is heading, on a monitor niri doesn't
    // report the switch for
    static SPRINGS: PerOutput<Animation<f32>> = const { PerOutput::new() };
}

/// One shape: how far its edge is from its middle at each of [`POINTS`]
/// even angles from the right, clockwise on screen, and where its middle
/// sits so it's centred in the box.
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    radii: Vec<f32>,
    centre: (f32, f32),
}

/// What the shape at `index` (0 the top workspace) is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Form {
    Circle,
    /// Sides, and the angle its first corner sits at.
    Polygon(usize, f32),
    /// Points, and how far in the corners between them are.
    Star(usize, f32),
}

const UP: f32 = -PI / 2.0;

fn form(index: usize) -> Form {
    match index {
        0 => Form::Circle,
        1 => Form::Polygon(4, PI / 4.0),
        2 => Form::Polygon(3, UP),
        3 => Form::Polygon(4, 0.0),
        4 => Form::Polygon(5, UP),
        5 => Form::Polygon(6, 0.0),
        6 => Form::Star(5, 0.5),
        7 => Form::Polygon(7, UP),
        8 => Form::Polygon(8, PI / 8.0),
        // stars with 6, 7, 8… points, a little plumper as they get more
        n => {
            let points = n - 3;
            Form::Star(points, (0.5 + 0.02 * (points - 6) as f32).min(0.7))
        }
    }
}

/// How far the edge of `form`, with corners `outer` from its middle, is
/// from it at angle `angle`.
fn radius(form: Form, outer: f32, angle: f32) -> f32 {
    match form {
        Form::Circle => outer,
        Form::Polygon(sides, first) => {
            let step = TAU / sides as f32;
            let along = (angle - first).rem_euclid(step);
            // `outer` at a corner, the apothem halfway along a side
            outer * (step / 2.0).cos() / (along - step / 2.0).cos()
        }
        Form::Star(points, inner) => {
            let step = TAU / points as f32;
            let along = (angle - UP).rem_euclid(step);
            // from a point out to the corner in, then back out to the next
            let (from, to, from_r, to_r) = if along < step / 2.0 {
                (0.0, step / 2.0, outer, outer * inner)
            } else {
                (step / 2.0, step, outer * inner, outer)
            };
            ray_to_segment(along, (from, from_r), (to, to_r))
        }
    }
}

/// How far out a ray at `angle` meets the straight line between the points
/// at polar `a` and `b` (angle, radius).
fn ray_to_segment(angle: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (ax, ay) = (a.1 * a.0.cos(), a.1 * a.0.sin());
    let (bx, by) = (b.1 * b.0.cos(), b.1 * b.0.sin());
    let (dx, dy) = (angle.cos(), angle.sin());
    // solve t·d = a + s·(b − a) for t
    let (ex, ey) = (bx - ax, by - ay);
    let cross = dx * ey - dy * ex;
    if cross.abs() < f32::EPSILON {
        return a.1;
    }
    (ax * ey - ay * ex) / cross
}

fn angle(point: usize) -> f32 {
    point as f32 * TAU / POINTS as f32
}

impl Outline {
    /// The shape for the workspace at `index`, its corners softened,
    /// covering [`AREA`] or as much as fits, and centred in the box.
    pub fn of(index: usize) -> Self {
        let form = form(index);
        let raw: Vec<f32> = (0..POINTS).map(|k| radius(form, 1.0, angle(k))).collect();
        let radii = soften(&raw);

        // as big as covers the area, then no bigger than fits the box
        let area: f32 = radii.iter().map(|r| r * r).sum::<f32>() * PI / POINTS as f32;
        let (left, right, top, bottom) = extent(&radii);
        let fit = BOX / (right - left).max(bottom - top);
        let scale = (AREA / area).sqrt().min(fit);
        let centre = (
            BOX / 2.0 - (left + right) / 2.0 * scale,
            BOX / 2.0 - (top + bottom) / 2.0 * scale,
        );
        Self {
            radii: radii.iter().map(|r| r * scale).collect(),
            centre,
        }
    }

    /// Part way from `self` to `other`, `t` from 0 (`self`) to 1.
    pub fn toward(&self, other: &Self, t: f32) -> Self {
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        Self {
            radii: (self.radii.iter().zip(&other.radii))
                .map(|(a, b)| lerp(*a, *b))
                .collect(),
            centre: (
                lerp(self.centre.0, other.centre.0),
                lerp(self.centre.1, other.centre.1),
            ),
        }
    }

    /// Its points in the box, turned by `turn` radians.
    pub fn points(&self, turn: f32) -> Vec<(f32, f32)> {
        self.radii
            .iter()
            .enumerate()
            .map(|(k, r)| {
                let at = angle(k) + turn;
                (self.centre.0 + r * at.cos(), self.centre.1 + r * at.sin())
            })
            .collect()
    }

    /// One closed straight-sided outline filled with `color`.
    pub fn path(&self, turn: f32, color: Color) -> Path {
        let points = self.points(turn);
        let mut path = Path::new().move_to(points[0].0, points[0].1);
        for (x, y) in &points[1..] {
            path = path.line_to(*x, *y);
        }
        path.close().fill(color)
    }
}

/// The radii averaged with their neighbours, [`SOFTEN`] either way, which
/// rounds the corners. Averaging is linear, so a morph between two softened
/// shapes is the softened morph.
fn soften(radii: &[f32]) -> Vec<f32> {
    let n = radii.len();
    // a tent: the point itself counts most
    let weights: Vec<f32> = (0..=2 * SOFTEN)
        .map(|i| (SOFTEN + 1 - i.abs_diff(SOFTEN)) as f32)
        .collect();
    let total: f32 = weights.iter().sum();
    (0..n)
        .map(|k| {
            weights
                .iter()
                .enumerate()
                .map(|(i, w)| w * radii[(k + n + i - SOFTEN) % n])
                .sum::<f32>()
                / total
        })
        .collect()
}

/// How far the outline reaches left, right, up and down of its middle.
fn extent(radii: &[f32]) -> (f32, f32, f32, f32) {
    radii.iter().enumerate().fold(
        (0.0f32, 0.0f32, 0.0f32, 0.0f32),
        |(left, right, top, bottom), (k, r)| {
            let (x, y) = (r * angle(k).cos(), r * angle(k).sin());
            (left.min(x), right.max(x), top.min(y), bottom.max(y))
        },
    )
}

/// The shape `at` along the workspaces, 0 the top one's, and how far it's
/// turned: between two it's part of each, eased so it rests near the true
/// shapes and changes quickly between them.
pub fn shape_at(at: f32) -> (Outline, f32) {
    let at = at.max(0.0);
    let below = at.floor();
    let t = at - below;
    let from = Outline::of(below as usize);
    if t <= 0.0 {
        return (from, 0.0);
    }
    let eased = t * t * (3.0 - 2.0 * t);
    let to = Outline::of(below as usize + 1);
    (from.toward(&to, eased), (PI * t).sin() * TURN)
}

/// Where niri's switch is on monitor `output` among its `count`
/// workspaces, from 0 at the top: kept to the ends while a swipe rubber-
/// bands past them, and on a workspace once it's within 0.01 of it, as niri
/// stops reporting a few thousandths short of a landing. None while niri
/// isn't reporting that monitor.
pub fn switch_position(switch: &WorkspaceSwitch, output: &str, count: usize) -> Option<f32> {
    if switch.output() != output || count == 0 {
        return None;
    }
    Some(settle(switch.workspace_idx(), count))
}

fn settle(position: f64, count: usize) -> f32 {
    let at = position.clamp(0.0, (count - 1) as f64);
    let at = if (at - at.round()).abs() < 0.01 {
        at.round()
    } else {
        at
    };
    at as f32
}

fn spring(value: f32) -> Animation<f32> {
    // about as long as niri's own critically damped switch takes
    amane_kit::motion::eased(
        value,
        Duration::from_millis(280),
        Easing::Spring { bounce: 0.0 },
    )
}

/// Where the shape on monitor `output` is: niri's switch while niri
/// reports it there, else springing to the workspace `shown` (0 the top).
fn position(output: &str, count: usize, shown: usize) -> f32 {
    let reported = switch_position(&WorkspaceSwitch::read(), output, count);
    SPRINGS.with(|springs| {
        springs.with(
            output,
            || spring(shown as f32),
            |spring_here| match reported {
                Some(at) => {
                    // kept where niri says, so losing the report never jumps
                    *spring_here = spring(at);
                    at
                }
                None => {
                    spring_here.to(shown as f32);
                    spring_here.value()
                }
            },
        )
    })
}

/// The indicator on monitor `output`'s bar, among `workspaces` (that
/// monitor's, top first): the shape for the one shown, in a bar item
/// `width` wide. A click opens niri's overview.
pub fn indicator(output: &str, workspaces: &[&Workspace], width: f32) -> Rectangle {
    let shown = workspaces
        .iter()
        .position(|workspace| workspace.focused())
        .or_else(|| workspaces.iter().position(|workspace| workspace.active()))
        .unwrap_or(0);
    let at = position(output, workspaces.len(), shown);
    let urgent = workspaces.iter().any(|workspace| workspace.urgent());
    let tip = workspaces
        .get(shown)
        .map(|workspace| {
            workspace
                .name()
                .map(String::from)
                .unwrap_or_else(|| format!("Workspace {}", workspace.index()))
        })
        .unwrap_or_default();

    let color = if urgent {
        sys::color::status::critical::bg()
    } else {
        material::label(Level::Primary)
    };

    super::item(DrawnButton::new("amane.bar.workspace", move |color| {
        let (outline, turn) = shape_at(at);
        Rectangle::new()
            .width(width)
            .height(Size::Parent)
            .align_child(Center, Center)
            .child(
                Canvas::new()
                    .width(BOX)
                    .height(BOX)
                    .shapes(vec![Box::new(outline.path(turn, color))]),
            )
    }))
    .width(width)
    .color(color)
    .on_press(amane::Windows::toggle_overview)
    .tooltip(super::BAR_NS, tip)
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Outline, b: &Outline) -> bool {
        a.radii
            .iter()
            .zip(&b.radii)
            .all(|(x, y)| (x - y).abs() < 1e-4)
            && (a.centre.0 - b.centre.0).abs() < 1e-4
            && (a.centre.1 - b.centre.1).abs() < 1e-4
    }

    #[test]
    fn every_shape_fits_its_box() {
        for index in 0..16 {
            let outline = Outline::of(index);
            assert!(outline.radii.iter().all(|r| *r > 0.5), "shape {index}");
            for (x, y) in outline.points(0.0) {
                assert!((-0.01..=BOX + 0.01).contains(&x), "shape {index}: x {x}");
                assert!((-0.01..=BOX + 0.01).contains(&y), "shape {index}: y {y}");
            }
        }
    }

    #[test]
    fn a_morph_starts_and_ends_on_the_shapes() {
        let (start, turn) = shape_at(1.0);
        assert!(close(&start, &Outline::of(1)));
        assert_eq!(turn, 0.0);
        let (end, _) = shape_at(1.999_999);
        assert!(close(&end, &Outline::of(2)));
        let (middle, turn) = shape_at(1.5);
        assert!(!close(&middle, &Outline::of(1)) && !close(&middle, &Outline::of(2)));
        assert!(turn > 0.0);
    }

    #[test]
    fn no_two_of_the_first_shapes_are_alike() {
        let shapes: Vec<Outline> = (0..12).map(Outline::of).collect();
        for (i, a) in shapes.iter().enumerate() {
            for b in &shapes[i + 1..] {
                assert!(!close(a, b));
            }
        }
    }

    #[test]
    fn the_shape_stays_on_quads() {
        // one closed outline of no more than the quad outline's 256 corners
        assert!(Outline::of(6).points(0.0).len() <= 256);
    }

    #[test]
    fn a_switch_settles_on_workspaces_and_stops_at_the_ends() {
        assert_eq!(settle(-0.3, 4), 0.0);
        assert_eq!(settle(3.4, 4), 3.0);
        assert_eq!(settle(2.996, 4), 3.0);
        assert_eq!(settle(1.5, 4), 1.5);
        assert_eq!(settle(0.0, 1), 0.0);
    }
}
