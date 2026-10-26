//! A subject's shadows as layers of their own, so an effect on the subject
//! doesn't take them in (the chrome would turn a drop shadow into chrome).
//!
//! Drop shadows go behind the subject in a layer that also cuts the
//! subject's shape out of itself, so they don't show through a see-through
//! surface; inner shadows go over it.

use amane::{Color, Mask, Rectangle, Shadow, Stack, Widget};

use super::config::ShadowSpec;
use crate::color_picker::to_color;
use crate::layout::boxed;

/// The drop shadows, for behind a `width` by `height` subject with corners
/// `radius`; none when there are none.
pub fn behind(shadows: &[ShadowSpec], width: f32, height: f32, radius: f32) -> Option<Rectangle> {
    let mut layers: Vec<Box<dyn Widget>> = shadows
        .iter()
        .filter(|shadow| !shadow.inner)
        .map(|shadow| boxed(shape(width, height, radius).shadow(to_shadow(shadow))))
        .collect();

    if layers.is_empty() {
        return None;
    }

    // a hole in the layer itself: the cut makes the layer a group of its own,
    // so it only cuts these shadows, not what's behind them
    layers.push(boxed(shape(width, height, radius).fill(Mask)));

    Some(
        Rectangle::new()
            .width(width)
            .height(height)
            .child(Stack::new(layers)),
    )
}

/// The inner shadows, for over the subject; none when there are none.
pub fn in_front(shadows: &[ShadowSpec], width: f32, height: f32, radius: f32) -> Option<Rectangle> {
    let layers: Vec<Box<dyn Widget>> = shadows
        .iter()
        .filter(|shadow| shadow.inner)
        .map(|shadow| boxed(shape(width, height, radius).shadow(to_shadow(shadow))))
        .collect();

    if layers.is_empty() {
        return None;
    }

    Some(
        Rectangle::new()
            .width(width)
            .height(height)
            .child(Stack::new(layers)),
    )
}

pub fn to_shadow(spec: &ShadowSpec) -> Shadow {
    let color = to_color(spec.rgba());

    let shadow = if spec.inner {
        Shadow::inner(color)
    } else {
        Shadow::drop(color)
    };

    shadow
        .offset(spec.x, spec.y)
        .blur(spec.blur.max(0.0))
        .spread(spec.spread)
        .opacity(spec.opacity.clamp(0.0, 1.0))
}

fn shape(width: f32, height: f32, radius: f32) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(height)
        .radius(radius.max(0.0))
        .fill(Color::TRANSPARENT)
}
