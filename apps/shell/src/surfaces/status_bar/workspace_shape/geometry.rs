//! The workspace indicator's shapes: each one's outline, its corners
//! rounded with true arcs, centred by eye and on whole pixels, and the
//! morph between two, which rolls the next shape onto its face.

use std::cell::RefCell;
use std::f32::consts::{PI, TAU};

use amane::Path;

/// How many points go round every shape. 120 is a multiple of 3, 4, 5, 6
/// and 8, so those shapes' corners fall on a point.
pub(super) const POINTS: usize = 120;

/// The box the shape rests in: the bar's text line.
pub(super) const BOX: f32 = super::super::size::LINE_HEIGHT;

/// The canvas around the box, with room for the shape to swell and ripple.
pub(super) const CANVAS: f32 = BOX * 1.5;

/// From the canvas's corner to the box's.
pub(super) const MARGIN: f32 = (CANVAS - BOX) / 2.0;

/// How many straight steps round each rounded corner.
pub(super) const ARC_STEPS: usize = 12;

/// A corner that turns in, between a star's points, is rounded this much
/// of an outer one, so the arms stay sharp enough to read.
pub(super) const CONCAVE: f32 = 0.5;

/// A rounded corner takes at most this share of either side it touches.
pub(super) const MOST_OF_A_SIDE: f32 = 0.45;

/// How far a switch rolls the shape, about: it rolls the nearest of its
/// symmetries to this, in radians.
pub(super) const ROLL: f32 = PI / 4.0;

/// How much smaller it is halfway between two shapes.
pub(super) const BREATH: f32 = 0.06;

/// How much area each shape covers: a circle `bar.workspace.shape.size`
/// across, so a square or a star looks as heavy as the circle rather than
/// the same width.
pub(super) fn shape_area() -> f32 {
    let radius = crate::tokens::bar::workspace::shape::size() / 2.0;
    PI * radius * radius
}

/// How round the corners are, as a share of the shape's size.
pub(super) fn corner() -> f32 {
    crate::tokens::bar::workspace::shape::corner()
}

thread_local! {
    // the shapes, made once each
    static OUTLINES: RefCell<Vec<Outline>> = const { RefCell::new(Vec::new()) };

    // each shape's next one, turned to sit closest to it, and the roll on
    // the way there
    static STEPS: RefCell<Vec<(Outline, f32)>> = const { RefCell::new(Vec::new()) };
}

/// One shape: how far its edge is from its middle at each of [`POINTS`]
/// even angles from the right, clockwise on screen, and where its middle
/// sits in the box.
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    radii: Vec<f32>,
    centre: (f32, f32),
}

/// What the shape at `index` (0 the top workspace) is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Form {
    Circle,
    /// Sides, and the angle its first corner sits at.
    Polygon(usize, f32),
    /// Points, and how far in the corners between them are.
    Star(usize, f32),
}

pub(super) const UP: f32 = -PI / 2.0;

pub(super) fn form(index: usize) -> Form {
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

/// How many times turning `form` round lands it on itself; none for the
/// circle, which any turn does.
pub(super) fn symmetry(form: Form) -> Option<usize> {
    match form {
        Form::Circle => None,
        Form::Polygon(sides, _) => Some(sides),
        Form::Star(points, _) => Some(points),
    }
}

/// How much more area `form` covers than the circle, so a pointed shape
/// looks as heavy as a round one.
pub(super) fn weight(form: Form) -> f32 {
    match form {
        Form::Polygon(3, _) => 1.08,
        Form::Star(..) => 1.06,
        _ => 1.0,
    }
}

/// The corners of `form`, its outer ones 1 from its middle, in order of
/// angle.
pub(super) fn vertices(form: Form) -> Vec<(f32, f32)> {
    let polar = |angle: f32, r: f32| (r * angle.cos(), r * angle.sin());
    match form {
        Form::Circle => Vec::new(),
        Form::Polygon(sides, first) => (0..sides)
            .map(|k| polar(first + k as f32 * TAU / sides as f32, 1.0))
            .collect(),
        Form::Star(points, inner) => (0..2 * points)
            .map(|k| {
                let r = if k % 2 == 0 { 1.0 } else { inner };
                polar(UP + k as f32 * PI / points as f32, r)
            })
            .collect(),
    }
}

pub(super) fn sub(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0 - b.0, a.1 - b.1)
}

pub(super) fn along(a: (f32, f32), b: (f32, f32), by: f32) -> (f32, f32) {
    (a.0 + b.0 * by, a.1 + b.1 * by)
}

pub(super) fn length(a: (f32, f32)) -> f32 {
    a.0.hypot(a.1)
}

pub(super) fn unit(a: (f32, f32)) -> (f32, f32) {
    let l = length(a);
    (a.0 / l, a.1 / l)
}

pub(super) fn cross(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.1 - a.1 * b.0
}

/// The outline through `vertices` with every corner rounded by an arc
/// `radius` across (a corner turning in, half that), each kept to
/// [`MOST_OF_A_SIDE`] of its sides, as a ring of points.
pub(super) fn rounded(vertices: &[(f32, f32)], radius: f32) -> Vec<(f32, f32)> {
    let n = vertices.len();
    let mut ring = Vec::with_capacity(n * (ARC_STEPS + 1));
    for k in 0..n {
        let at = vertices[k];
        let before = vertices[(k + n - 1) % n];
        let after = vertices[(k + 1) % n];
        let (back, on) = (sub(before, at), sub(after, at));
        let (a, b) = (unit(back), unit(on));

        // half the angle between the two sides, on the side the arc is
        let half = (a.0 * b.0 + a.1 * b.1).clamp(-1.0, 1.0).acos() / 2.0;
        let convex = cross(sub(at, before), on) > 0.0;
        let wanted = if convex { radius } else { radius * CONCAVE };

        // how far along each side the arc starts, kept short of the side
        let reach = (wanted / half.tan()).min(MOST_OF_A_SIDE * length(back).min(length(on)));
        let r = reach * half.tan();
        let from = along(at, a, reach);
        let to = along(at, b, reach);
        let middle = along(at, unit((a.0 + b.0, a.1 + b.1)), r / half.sin());

        // round the short way from one side to the other
        let start = (from.1 - middle.1).atan2(from.0 - middle.0);
        let end = (to.1 - middle.1).atan2(to.0 - middle.0);
        let sweep = (end - start + PI).rem_euclid(TAU) - PI;
        for step in 0..=ARC_STEPS {
            let angle = start + sweep * step as f32 / ARC_STEPS as f32;
            ring.push((middle.0 + r * angle.cos(), middle.1 + r * angle.sin()));
        }
    }
    ring
}

/// How far out a ray from the middle in direction `ray` meets the line
/// from `a` to `b`, if it does.
pub(super) fn ray_hit(ray: (f32, f32), a: (f32, f32), b: (f32, f32)) -> Option<f32> {
    let edge = sub(b, a);
    let across = cross(ray, edge);
    if across.abs() < 1e-9 {
        return None;
    }
    // solve t·ray = a + s·edge
    let t = cross(a, edge) / across;
    let s = cross(a, ray) / across;
    (t > 0.0 && (-1e-5..=1.0 + 1e-5).contains(&s)).then_some(t)
}

/// How far the closed `ring` is from the middle at each of [`POINTS`]
/// angles. Every shape is star-shaped about its middle, so each ray meets
/// it once.
pub(super) fn radii_of(ring: &[(f32, f32)]) -> Vec<f32> {
    let n = ring.len();
    (0..POINTS)
        .map(|k| {
            let ray = (angle(k).cos(), angle(k).sin());
            (0..n)
                .filter_map(|i| ray_hit(ray, ring[i], ring[(i + 1) % n]))
                .fold(0.0, f32::max)
        })
        .collect()
}

pub(super) fn angle(point: usize) -> f32 {
    point as f32 * TAU / POINTS as f32
}

/// How a shape is drawn: turned, scaled about its middle across and down,
/// and moved down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub turn: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub shift_y: f32,
}

impl Pose {
    pub const REST: Self = Self {
        turn: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        shift_y: 0.0,
    };
}

impl Outline {
    /// The shape for the workspace at `index`, made once.
    pub fn of(index: usize) -> Self {
        OUTLINES.with_borrow_mut(|outlines| {
            while outlines.len() <= index {
                let next = Self::made(outlines.len());
                outlines.push(next);
            }
            outlines[index].clone()
        })
    }

    /// The shape for the workspace at `index`, its corners rounded,
    /// covering [`shape_area`] (weighted for its form) or as much as fits,
    /// centred by eye in the box and on whole pixels.
    fn made(index: usize) -> Self {
        let form = form(index);
        let radii = match form {
            Form::Circle => vec![1.0; POINTS],
            _ => radii_of(&rounded(&vertices(form), corner() * 2.0)),
        };

        let step = TAU / POINTS as f32;
        let area: f32 = radii.iter().map(|r| r * r).sum::<f32>() * step / 2.0;
        // where its area balances: halfway between that and the middle of
        // its bounds looks centred, a triangle's weight and point both
        let (cx, cy) = radii.iter().enumerate().fold((0.0, 0.0), |(x, y), (k, r)| {
            let moment = r * r * r / 3.0 * step;
            (x + moment * angle(k).cos(), y + moment * angle(k).sin())
        });
        let (left, right, top, bottom) = extent(&radii);
        let middle = (
            ((left + right) / 2.0 + cx / area) / 2.0,
            ((top + bottom) / 2.0 + cy / area) / 2.0,
        );

        // as big as covers the area, then no bigger than fits the box
        let half = (right - middle.0)
            .max(middle.0 - left)
            .max(bottom - middle.1)
            .max(middle.1 - top);
        let fit = BOX / 2.0 / half;
        let scale = (shape_area() * weight(form) / area).sqrt().min(fit);
        let mut centre = (BOX / 2.0 - middle.0 * scale, BOX / 2.0 - middle.1 * scale);

        // its edges on whole pixels, where that still fits, so it's sharp
        let snap = |centre: f32, low: f32, high: f32| {
            let low = centre + low * scale;
            let moved = low.round() - low;
            let high = centre + high * scale + moved;
            if low + moved >= 0.0 && high <= BOX {
                centre + moved
            } else {
                centre
            }
        };
        centre = (snap(centre.0, left, right), snap(centre.1, top, bottom));

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

    /// `self` turned by `shift` of its points, clockwise.
    fn shifted(&self, shift: isize) -> Self {
        let n = POINTS as isize;
        Self {
            radii: (0..n)
                .map(|k| self.radii[(k - shift).rem_euclid(n) as usize])
                .collect(),
            centre: self.centre,
        }
    }

    /// Its points in the canvas, in `pose`, each `grow` times as far out
    /// and then `inset` nearer in.
    pub fn points(&self, pose: Pose, grow: f32, inset: f32) -> Vec<(f32, f32)> {
        let (cx, cy) = (MARGIN + self.centre.0, MARGIN + self.centre.1);
        self.radii
            .iter()
            .enumerate()
            .map(|(k, r)| {
                let at = angle(k) + pose.turn;
                let r = (r * grow - inset).max(0.0);
                (
                    cx + r * at.cos() * pose.scale_x,
                    cy + r * at.sin() * pose.scale_y + pose.shift_y,
                )
            })
            .collect()
    }
}

/// One closed straight-sided outline through `points`.
pub(super) fn polygon(points: &[(f32, f32)]) -> Path {
    let mut path = Path::new().move_to(points[0].0, points[0].1);
    for (x, y) in &points[1..] {
        path = path.line_to(*x, *y);
    }
    path.close()
}

/// How far the outline reaches left, right, up and down of its middle.
pub(super) fn extent(radii: &[f32]) -> (f32, f32, f32, f32) {
    radii.iter().enumerate().fold(
        (0.0f32, 0.0f32, 0.0f32, 0.0f32),
        |(left, right, top, bottom), (k, r)| {
            let (x, y) = (r * angle(k).cos(), r * angle(k).sin());
            (left.min(x), right.max(x), top.min(y), bottom.max(y))
        },
    )
}

/// The shape after the one at `index`, turned to sit closest to it, and
/// how far to roll on the way: the nearest to [`ROLL`] of the turns that
/// bring it back to one of its symmetries, so it lands upright.
pub(super) fn step(index: usize) -> (Outline, f32) {
    STEPS.with_borrow_mut(|steps| {
        while steps.len() <= index {
            let from = steps.len();
            steps.push(aligned(from));
        }
        steps[index].clone()
    })
}

pub(super) fn aligned(index: usize) -> (Outline, f32) {
    let from = Outline::of(index);
    let to = Outline::of(index + 1);
    let Some(order) = symmetry(form(index + 1)) else {
        return (to, ROLL);
    };

    // the turn, within one of its symmetries either way, that differs least
    let reach = (POINTS / order / 2) as isize;
    let difference = |shift: isize| {
        let turned = to.shifted(shift);
        (from.radii.iter().zip(&turned.radii))
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>()
    };
    let shift = (-reach..=reach)
        .min_by(|a, b| difference(*a).total_cmp(&difference(*b)))
        .unwrap_or(0);
    let turned = shift as f32 * TAU / POINTS as f32;

    // a roll that undoes that turn and lands on a symmetry, near ROLL
    let symmetric = TAU / order as f32;
    let roll = (-3..=8)
        .map(|m| m as f32 * symmetric - turned)
        .filter(|roll| *roll > 0.05)
        .min_by(|a, b| (a - ROLL).abs().total_cmp(&(b - ROLL).abs()))
        .unwrap_or(symmetric - turned);

    (to.shifted(shift), roll)
}

pub(super) fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// The shape `at` along the workspaces, 0 the top one's, and its pose:
/// between two it's part of each, eased so it rests near the true shapes
/// and changes quickly between them, rolling on and breathing in a little.
/// Where motion is reduced it only changes shape.
pub fn shape_at(at: f32, reduced: bool) -> (Outline, Pose) {
    let at = at.max(0.0);
    let below = at.floor();
    let t = at - below;
    let from = Outline::of(below as usize);
    if t <= 0.0 {
        return (from, Pose::REST);
    }
    let eased = smoothstep(t);
    if reduced {
        let to = Outline::of(below as usize + 1);
        return (from.toward(&to, eased), Pose::REST);
    }
    let (to, roll) = step(below as usize);
    let breath = 1.0 - BREATH * (PI * t).sin();
    let pose = Pose {
        turn: roll * eased,
        scale_x: breath,
        scale_y: breath,
        shift_y: 0.0,
    };
    (from.toward(&to, eased), pose)
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

    /// Whether every point of `a` has one of `b`'s near it, and back.
    fn same_points(a: &[(f32, f32)], b: &[(f32, f32)], within: f32) -> bool {
        let near = |p: &(f32, f32), all: &[(f32, f32)]| {
            all.iter().any(|q| (p.0 - q.0).hypot(p.1 - q.1) < within)
        };
        a.iter().all(|p| near(p, b)) && b.iter().all(|p| near(p, a))
    }

    fn inside_box(points: &[(f32, f32)]) -> bool {
        points.iter().all(|(x, y)| {
            (MARGIN - 0.01..=MARGIN + BOX + 0.01).contains(x)
                && (MARGIN - 0.01..=MARGIN + BOX + 0.01).contains(y)
        })
    }

    #[test]
    fn every_shape_fits_its_box() {
        for index in 0..16 {
            let outline = Outline::of(index);
            assert!(outline.radii.iter().all(|r| *r > 0.5), "shape {index}");
            assert!(
                inside_box(&outline.points(Pose::REST, 1.0, 0.0)),
                "shape {index}"
            );
        }
    }

    #[test]
    fn a_morph_starts_and_ends_on_the_shapes() {
        let (start, pose) = shape_at(1.0, false);
        assert!(close(&start, &Outline::of(1)));
        assert_eq!(pose, Pose::REST);

        // the square rolls onto the triangle, landing upright
        let (end, pose) = shape_at(1.999_999, false);
        let landed = end.points(
            Pose {
                turn: pose.turn,
                ..Pose::REST
            },
            1.0,
            0.0,
        );
        let triangle = Outline::of(2).points(Pose::REST, 1.0, 0.0);
        assert!(same_points(&landed, &triangle, 0.05));

        let (middle, pose) = shape_at(1.5, false);
        assert!(!close(&middle, &Outline::of(1)) && !close(&middle, &Outline::of(2)));
        assert!(pose.turn > 0.0 && pose.scale_x < 1.0);
    }

    #[test]
    fn a_roll_lands_on_a_symmetry() {
        for index in 0..12 {
            let (to, roll) = step(index);
            assert!(roll > 0.0, "shape {index}");
            let Some(order) = symmetry(form(index + 1)) else {
                continue;
            };
            // the shift that aligned it plus the roll is a whole symmetry
            let shift = (0..POINTS as isize)
                .find(|s| close(&Outline::of(index + 1).shifted(*s), &to))
                .expect("aligned is a turn of the next shape");
            let shift = if shift > POINTS as isize / 2 {
                shift - POINTS as isize
            } else {
                shift
            };
            let total = shift as f32 * TAU / POINTS as f32 + roll;
            let symmetric = TAU / order as f32;
            let off = (total / symmetric - (total / symmetric).round()).abs();
            assert!(off < 1e-3, "shape {index}: {off}");
        }
    }

    #[test]
    fn going_back_rolls_the_other_way() {
        // the same frames played backward: the turn shrinks as it goes up
        let (_, down) = shape_at(2.3, false);
        let (_, further) = shape_at(2.6, false);
        assert!(further.turn > down.turn);
    }

    #[test]
    fn alignment_never_makes_the_morph_worse() {
        let difference = |a: &Outline, b: &Outline| {
            (a.radii.iter().zip(&b.radii))
                .map(|(x, y)| (x - y) * (x - y))
                .sum::<f32>()
        };
        for index in 0..12 {
            let from = Outline::of(index);
            let (aligned, _) = step(index);
            assert!(
                difference(&from, &aligned) <= difference(&from, &Outline::of(index + 1)) + 1e-4
            );
        }
    }

    #[test]
    fn reduced_motion_only_changes_shape() {
        let (_, pose) = shape_at(2.4, true);
        assert_eq!(pose, Pose::REST);
    }

    #[test]
    fn corners_are_rounded() {
        // the square's corner is pulled in from where a sharp one would be
        let square = Outline::of(3); // corners at 0°, 90°…
        let side = square.radii[POINTS / 8]; // the middle of a side, at 45°
        let sharp = side * 2f32.sqrt();
        let corner = square.radii[0];
        let rounding = sharp - corner;
        let expected = {
            // an arc r across, at a right-angled corner, cuts r(√2 − 1) off
            let size = sharp * 2.0 / 2f32.sqrt(); // the side's length, scaled
            let r = super::corner() * 2.0 * (size / 2f32.sqrt());
            r * (2f32.sqrt() - 1.0)
        };
        assert!(
            (rounding - expected).abs() < 0.15,
            "{rounding} vs {expected}"
        );
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
}
