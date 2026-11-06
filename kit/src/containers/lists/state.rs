//! An item's interaction state, and the motion between states: the state
//! layer fades at hover speed and an expressive item's corners spring to
//! the shape of the state it's in.

use amane::Animation;

use super::style::{ListTokens, Shapes};
use crate::buttons::with_state;
use crate::motion;

/// The interaction state an item is drawn in. Selected and disabled are
/// separate: an item can be both selected and hovered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum State {
    #[default]
    Enabled,
    Hovered,
    Focused,
    Pressed,
    Dragged,
}

impl State {
    pub const ALL: [State; 5] = [
        State::Enabled,
        State::Hovered,
        State::Focused,
        State::Pressed,
        State::Dragged,
    ];

    pub fn name(self) -> &'static str {
        match self {
            State::Enabled => "Enabled",
            State::Hovered => "Hovered",
            State::Focused => "Focused",
            State::Pressed => "Pressed",
            State::Dragged => "Dragged",
        }
    }

    /// How strong the state layer is in this state.
    pub fn layer_opacity(self, t: &ListTokens) -> f32 {
        match self {
            State::Enabled => 0.0,
            State::Hovered => t.hover_opacity,
            State::Focused => t.focus_opacity,
            State::Pressed => t.pressed_opacity,
            State::Dragged => t.dragged_opacity,
        }
    }
}

/// Where an item sits in a segmented list: which of its corners round to
/// the list's outer radius.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Segment {
    pub index: usize,
    pub count: usize,
}

/// The four corner radii (top left, top right, bottom right, bottom left)
/// of an item in `state`, `selected` or not, at `segment` of its list.
/// The state's shape wins, in Compose's order: pressed, dragged, selected,
/// focused, hovered; else the segment's resting shape.
pub fn corners(
    shapes: &Shapes,
    state: State,
    selected: bool,
    segment: Option<Segment>,
) -> [f32; 4] {
    let chosen = match state {
        State::Pressed => shapes.pressed,
        State::Dragged => shapes.dragged,
        _ => None,
    }
    .or(if selected { shapes.selected } else { None })
    .or(match state {
        State::Focused => shapes.focused,
        State::Hovered => shapes.hovered,
        _ => None,
    });
    if let Some(radius) = chosen {
        return [radius; 4];
    }

    let base = shapes.base;
    let outer = shapes.outer;
    match segment {
        None => [base; 4],
        Some(Segment { count: 1, .. }) => [outer; 4],
        Some(Segment { index, count }) => {
            let top = if index == 0 { outer } else { base };
            let bottom = if index + 1 == count { outer } else { base };
            [top, top, bottom, bottom]
        }
    }
}

/// The state layer's opacity for item `id`, fading toward `target` at
/// hover speed; straight to it when `animate` is off (a forced state).
pub fn layer(id: &str, target: f32, animate: bool) -> f32 {
    if !animate {
        return target;
    }
    with_state(
        &format!("{id}.layer"),
        || motion::fade(target, motion::HOVER),
        |fade: &mut Animation<f32>| {
            fade.to(target);
            fade.value().clamp(0.0, 1.0)
        },
    )
}

struct Corners([Animation<f32>; 4]);

/// Item `id`'s corners, springing toward `target`.
pub fn shape(id: &str, target: [f32; 4], animate: bool) -> [f32; 4] {
    if !animate || crate::theme::reduced_motion() {
        return target;
    }
    with_state(
        &format!("{id}.shape"),
        || Corners(target.map(motion::spring)),
        |corners: &mut Corners| {
            let mut out = [0.0; 4];
            for (i, corner) in corners.0.iter_mut().enumerate() {
                corner.to(target[i]);
                out[i] = corner.value().max(0.0);
            }
            out
        },
    )
}

/// A value of item `id` keyed `key` springing toward `target`: an expand
/// chevron's turn, a reordered row's shift.
pub fn spring(id: &str, key: &str, target: f32) -> f32 {
    if crate::theme::reduced_motion() {
        return target;
    }
    with_state(
        &format!("{id}.{key}"),
        || motion::spring(target),
        |value: &mut Animation<f32>| {
            value.to(target);
            value.value()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lists::ListStyle;

    #[test]
    fn segments_round_their_outer_corners() {
        let s = ListStyle::M3Expressive.tokens().shapes;
        let at = |index, count| Some(Segment { index, count });
        assert_eq!(
            corners(&s, State::Enabled, false, at(0, 3)),
            [16.0, 16.0, 4.0, 4.0]
        );
        assert_eq!(corners(&s, State::Enabled, false, at(1, 3)), [4.0; 4]);
        assert_eq!(
            corners(&s, State::Enabled, false, at(2, 3)),
            [4.0, 4.0, 16.0, 16.0]
        );
        assert_eq!(corners(&s, State::Enabled, false, at(0, 1)), [16.0; 4]);
    }

    #[test]
    fn a_state_shape_wins_over_the_segment() {
        let s = ListStyle::M3Expressive.tokens().shapes;
        let middle = Some(Segment { index: 1, count: 3 });
        assert_eq!(corners(&s, State::Hovered, false, middle), [12.0; 4]);
        assert_eq!(corners(&s, State::Pressed, false, middle), [16.0; 4]);
        // selected beats hovered
        assert_eq!(corners(&s, State::Hovered, true, middle), [16.0; 4]);
    }

    #[test]
    fn the_baseline_stays_square() {
        let s = ListStyle::M3.tokens().shapes;
        for state in State::ALL {
            assert_eq!(corners(&s, state, true, None), [0.0; 4]);
        }
    }
}
