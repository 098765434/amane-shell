//! The workspace indicator: one shape whose form says which workspace a
//! monitor shows. The first is a circle, the second a square, the third a
//! triangle, then a diamond, a pentagon, a hexagon and a star, and after
//! those more polygons and stars with more points. Every corner is rounded
//! with a true arc (`bar.workspace.shape.corner`), and a workspace with no
//! windows shows as an outline, its fill growing from the middle once one
//! opens.
//!
//! It morphs along with niri's own switch, a swipe's finger included, from
//! the fork's `WorkspaceSwitchProgress` ([`WorkspaceSwitch`]). On a monitor
//! that isn't focused, which that leaves out, it springs to the workspace
//! shown instead. Between two shapes it rolls like a die onto its next
//! face: the next shape is turned to sit closest to this one, and the turn
//! that undoes that lands on one of its symmetries, so it rests upright.
//!
//! It answers what's done to it and is still otherwise: it squashes against
//! the end of a rubber-banding swipe, bumps as it lands, shrinks with the
//! overview, swells under the pointer, and sends two ripples out when a
//! workspace turns urgent.
//!
//! Every path is one closed straight-sided outline (a fill, or a stroke
//! with round joins), so it draws with the quads and never reaches vello.

use std::f32::consts::PI;
use std::time::Duration;

use crate::chrome::per_output::PerOutput;

use amane::{
    Animation, Canvas, Center, Color, Easing, Join, Overview, Rectangle, Service, Shape as _, Size,
    Workspace, WorkspaceSwitch,
};

use crate::tokens::sys;
use emaki_kit::buttons::{self, DrawnButton};
use emaki_kit::material::{self, Level};
use emaki_kit::motion;

mod geometry;

use geometry::{BOX, CANVAS, Outline, Pose, polygon, shape_at, smoothstep};

/// How far it squashes against an end, at most, and how much per
/// workspace a swipe pulls past it.
const SQUASH: f32 = 0.12;
const SQUASH_PER: f32 = 0.6;

/// How much it swells as it lands on a workspace, and for how long.
const LANDING: f32 = 0.05;
const LANDING_TIME: Duration = Duration::from_millis(240);

/// How big it is under the pointer.
const HOVER: f32 = 1.08;

/// How much smaller it is with the overview open.
const OVERVIEW: f32 = 0.15;

/// The ripples an urgent workspace sends out: how far they spread, how
/// strong they start, each one's length and the wait between them.
const RIPPLE_SCALE: f32 = 1.6;
const RIPPLE_ALPHA: f32 = 0.6;
const RIPPLE_WIDTH: f32 = 1.0;
const RIPPLES: usize = 2;
const RIPPLE_TIME: Duration = Duration::from_millis(560);
const RIPPLE_GAP: Duration = Duration::from_millis(240);

/// How long it springs to the workspace shown on a monitor niri doesn't
/// report a switch for: about as long as niri's own critically damped
/// switch takes.
const FOLLOW_TIME: Duration = Duration::from_millis(280);

/// The fill's spring as the first window opens (plopping past the edge
/// and back) and as the last one goes (draining calmly).
const FILL_TIME: Duration = Duration::from_millis(420);
const FILL_BOUNCE: f32 = 0.3;
const DRAIN_TIME: Duration = Duration::from_millis(260);

/// How thick an empty workspace's outline is.
fn stroke() -> f32 {
    crate::tokens::bar::workspace::shape::stroke()
}

thread_local! {
    // each monitor's motion: where its shape is, how full, its colour and
    // the ripples and bumps under way
    static LIVE: PerOutput<Live> = const { PerOutput::new() };
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

/// How far a swipe on monitor `output` pulls past either end, in
/// workspaces: below 0 past the top.
fn pulled_past(switch: &WorkspaceSwitch, output: &str, count: usize) -> f32 {
    if switch.output() != output || count == 0 {
        return 0.0;
    }
    let at = switch.workspace_idx();
    (at - at.clamp(0.0, (count - 1) as f64)) as f32
}

/// The squash for a swipe `past` an end: shorter along the swipe and as
/// much wider, so it keeps its area, and pressed toward that end.
pub fn squashed(past: f32) -> Pose {
    let squash = (past.abs() * SQUASH_PER).min(SQUASH);
    Pose {
        turn: 0.0,
        scale_x: 1.0 / (1.0 - squash),
        scale_y: 1.0 - squash,
        shift_y: past.signum() * squash * BOX / 4.0,
    }
}

/// How full the shape is part way `at` along workspaces of which `full`
/// say which have windows: each one's own, eased between two.
pub fn fullness(at: f32, full: &[bool]) -> f32 {
    let of = |index: usize| {
        if full.get(index).copied().unwrap_or(false) {
            1.0
        } else {
            0.0
        }
    };
    let below = at.max(0.0).floor();
    let (from, to) = (of(below as usize), of(below as usize + 1));
    from + (to - from) * smoothstep(at - below)
}

fn spring(value: f32) -> Animation<f32> {
    motion::eased(value, FOLLOW_TIME, Easing::Spring { bounce: 0.0 })
}

/// The fill on its way to `full`: plopping in past the edge and back as
/// the first window opens, draining calmly as the last one goes.
fn filling(from: f32, full: bool) -> Animation<f32> {
    let (time, bounce) = if full {
        (FILL_TIME, FILL_BOUNCE)
    } else {
        (DRAIN_TIME, 0.0)
    };
    let mut fill = motion::eased(from, time, Easing::Spring { bounce });
    fill.to(if full { 1.0 } else { 0.0 });
    fill
}

/// 0 to 1 over `time`, from now.
fn run(time: Duration) -> Animation<f32> {
    let mut run = Animation::new(0.0).duration(time).easing(Easing::Linear);
    run.to(1.0);
    run
}

/// Something that ran once and is over.
fn done() -> Animation<f32> {
    Animation::new(1.0)
}

/// A monitor's indicator in motion.
struct Live {
    /// Where its shape is, on a monitor niri doesn't report the switch for.
    position: Animation<f32>,
    /// How full its shape is, and where that's heading.
    filled: Animation<f32>,
    filled_to: f32,
    color: Animation<Color>,
    urgent: bool,
    /// The urgent ripples, 0 to 1 over all of them.
    ripple: Animation<f32>,
    /// The bump as it lands, 0 to 1.
    landing: Animation<f32>,
    /// The workspace it last rested on.
    landed: f32,
}

/// How long all the ripples take, the last starting after the others.
fn ripples_time() -> Duration {
    RIPPLE_TIME + RIPPLE_GAP * (RIPPLES as u32 - 1)
}

/// What a monitor's shape looks like this frame.
struct Look {
    /// The shape, or the two mixed, where the switch is.
    outline: Outline,
    filled: f32,
    color: Color,
    pose: Pose,
    /// How far each ripple has spread, 0 to 1, the ones under way.
    ripples: Vec<f32>,
}

/// Moves monitor `output`'s indicator on, among `count` workspaces of
/// which `full` have windows, showing `shown`, and says how it looks.
fn look(output: &str, full: &[bool], shown: usize, urgent: bool) -> Look {
    let count = full.len();
    let switch = WorkspaceSwitch::read();
    let reported = switch_position(&switch, output, count);
    let past = pulled_past(&switch, output, count);
    drop(switch);
    let reduced = emaki_kit::theme::reduced_motion();
    let shown_full = full.get(shown).copied().unwrap_or(false);
    let target_color = if urgent {
        sys::color::status::critical::bg()
    } else {
        material::label(Level::Primary)
    };

    LIVE.with(|live| {
        live.with(
            output,
            || Live {
                position: spring(shown as f32),
                filled: Animation::new(if shown_full { 1.0 } else { 0.0 }),
                filled_to: if shown_full { 1.0 } else { 0.0 },
                color: motion::fade(target_color, motion::duration::crossfade()),
                urgent,
                ripple: done(),
                landing: done(),
                landed: shown as f32,
            },
            |live| {
                // where it is: niri's word while it reports this monitor
                let at = match reported {
                    Some(at) => {
                        // kept where niri says, so losing the report never jumps
                        live.position = spring(at);
                        at
                    }
                    None => {
                        live.position.to(shown as f32);
                        live.position.value()
                    }
                };

                // a bump as it comes to rest on another workspace
                if at.fract() == 0.0 && at != live.landed {
                    live.landed = at;
                    if !reduced {
                        live.landing = run(LANDING_TIME);
                    }
                }

                // full along with the switch, else springing as windows come and go
                let filled = match reported {
                    Some(at) => {
                        let now = fullness(at, full);
                        live.filled = Animation::new(now);
                        live.filled_to = now;
                        now
                    }
                    None => {
                        let to = if shown_full { 1.0 } else { 0.0 };
                        if to != live.filled_to {
                            live.filled = filling(live.filled.value(), shown_full);
                            live.filled_to = to;
                        }
                        live.filled.value()
                    }
                };

                live.color.to(target_color);
                if urgent && !live.urgent && !reduced {
                    live.ripple = run(ripples_time());
                }
                live.urgent = urgent;

                let (outline, mut pose) = shape_at(at, reduced);
                if !reduced {
                    let squash = squashed(past);
                    let bump = 1.0 + LANDING * (PI * live.landing.value()).sin();
                    pose.scale_x *= squash.scale_x * bump;
                    pose.scale_y *= squash.scale_y * bump;
                    pose.shift_y += squash.shift_y;
                }

                let ripples = {
                    let elapsed = live.ripple.value() * ripples_time().as_secs_f32();
                    (0..RIPPLES)
                        .map(|i| {
                            (elapsed - RIPPLE_GAP.as_secs_f32() * i as f32)
                                / RIPPLE_TIME.as_secs_f32()
                        })
                        .filter(|spread| (0.0..1.0).contains(spread))
                        .collect()
                };

                Look {
                    outline,
                    filled,
                    color: live.color.value(),
                    pose,
                    ripples,
                }
            },
        )
    })
}

/// The scale the pointer and the overview give the shape: swollen under
/// the pointer, smaller as the overview opens.
fn presence(id: &str, reduced: bool) -> f32 {
    let overview = 1.0 - OVERVIEW * Overview::read().progress().clamp(0.0, 1.0) as f32;
    if reduced {
        return overview;
    }
    let hovered = buttons::hovered(id);
    let hover = buttons::with_state(
        &format!("{id}.swell"),
        || motion::spring(1.0),
        |swell: &mut Animation<f32>| {
            swell.to(if hovered { HOVER } else { 1.0 });
            swell.value()
        },
    );
    hover * overview
}

/// The paths for `outline` in `pose`, `filled` full, in `color`, with
/// `ripples` spreading round it: the ripples, then the outline while it
/// isn't full, then the fill growing from the middle.
fn paths(
    outline: &Outline,
    pose: Pose,
    filled: f32,
    color: Color,
    ripples: &[f32],
) -> Vec<Box<dyn amane::Shape>> {
    let mut shapes: Vec<Box<dyn amane::Shape>> = Vec::new();
    for spread in ripples {
        let eased = 1.0 - (1.0 - spread) * (1.0 - spread);
        let grow = 1.0 + (RIPPLE_SCALE - 1.0) * eased;
        let ring = polygon(&outline.points(pose, grow, 0.0))
            .stroke(RIPPLE_WIDTH, color)
            .join(Join::Round)
            .opacity(RIPPLE_ALPHA * (1.0 - spread));
        shapes.push(Box::new(ring));
    }
    if filled < 1.0 {
        let width = stroke();
        let ring = polygon(&outline.points(pose, 1.0, width / 2.0))
            .stroke(width, color)
            .join(Join::Round);
        shapes.push(Box::new(ring));
    }
    if filled > 0.02 {
        let fill = polygon(&outline.points(pose, filled, 0.0)).fill(color);
        shapes.push(Box::new(fill));
    }
    shapes
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
    let full: Vec<bool> = workspaces.iter().map(|w| w.windows() > 0).collect();
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

    let id = format!("emaki.bar.workspace.{output}");
    let reduced = emaki_kit::theme::reduced_motion();
    let look = look(output, &full, shown, urgent);
    let presence = presence(&id, reduced);

    super::item(DrawnButton::new(id, move |color| {
        let pose = Pose {
            scale_x: look.pose.scale_x * presence,
            scale_y: look.pose.scale_y * presence,
            ..look.pose
        };
        Rectangle::new()
            .width(width)
            .height(Size::Parent)
            .align_child(Center, Center)
            .child(Canvas::new().width(CANVAS).height(CANVAS).shapes(paths(
                &look.outline,
                pose,
                look.filled,
                color,
                &look.ripples,
            )))
    }))
    .width(width)
    .color(look.color)
    .on_press(amane::Windows::toggle_overview)
    .tooltip(super::BAR_NS, tip)
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swollen_shapes_stay_on_the_canvas() {
        // swollen under the pointer, bumping, plopping full and squashed
        // all at once; ripples may spread past it, as the button clips them
        let most = HOVER * (1.0 + LANDING) / (1.0 - SQUASH) * 1.1;
        for index in 0..16 {
            let pose = Pose {
                scale_x: most,
                scale_y: most,
                ..Pose::REST
            };
            for (x, y) in Outline::of(index).points(pose, 1.0, 0.0) {
                assert!((0.0..=CANVAS).contains(&x) && (0.0..=CANVAS).contains(&y));
            }
        }
    }

    #[test]
    fn a_squash_keeps_the_area() {
        for past in [-0.5, -0.1, 0.05, 0.3] {
            let pose = squashed(past);
            assert!((pose.scale_x * pose.scale_y - 1.0).abs() < 0.01);
            assert!(pose.scale_y <= 1.0);
        }
        assert_eq!(squashed(0.0).scale_y, 1.0);
        assert!(squashed(-0.2).shift_y < 0.0 && squashed(0.2).shift_y > 0.0);
    }

    #[test]
    fn the_paths_stay_on_quads() {
        // a fill is one closed outline of no more than the quad outline's 256
        // corners; a ring one closed polyline
        for index in 0..12 {
            let points = Outline::of(index).points(Pose::REST, 1.0, 0.75);
            assert!(points.len() <= 256);
            assert!(points.len() < 121);
        }
    }

    #[test]
    fn fullness_follows_the_switch() {
        let full = [true, false, true];
        assert_eq!(fullness(0.0, &full), 1.0);
        assert_eq!(fullness(1.0, &full), 0.0);
        assert_eq!(fullness(2.0, &full), 1.0);
        let halfway = fullness(0.5, &full);
        assert!((halfway - 0.5).abs() < 1e-4);
        assert!(fullness(1.2, &full) < fullness(1.8, &full));
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
