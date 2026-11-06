//! What a list looks like: one [`ListTokens`] per [`ListStyle`], every
//! number and colour an item is drawn from.
//!
//! - `Shell` is the shell's own look, from `sys.*` (the look today's
//!   [`crate::list`] has), with the states M3 adds.
//! - `M3` is Material 3's baseline list: Material Web's tokens v0.192
//!   (`md-comp-list`) and the legacy Compose `ListItem`'s paddings.
//! - `M3Expressive` is the 2025 expressive list: Compose material3's
//!   `ListTokens` 29.0.0 and `ListItemDefaults` (segmented items 2 dp
//!   apart, shapes that change with the state, selected and dragged
//!   containers).
//!
//! The M3 styles use Material's sizes and type scale with the shell's
//! font (Roboto isn't shipped), and the baseline scheme
//! ([`super::scheme`]). See `docs/kit/lists-m3.md` for the sources.

use amane::{Color, Shadow};

use super::scheme;
use crate::color::{over, with_alpha};
use crate::layout::text_height;
use crate::material::{self, FillLevel, Level};
use crate::theme;
use crate::tokens::{TextStyle, sys};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ListStyle {
    #[default]
    Shell,
    M3,
    M3Expressive,
}

impl ListStyle {
    pub const ALL: [ListStyle; 3] = [ListStyle::Shell, ListStyle::M3, ListStyle::M3Expressive];

    pub fn name(self) -> &'static str {
        match self {
            ListStyle::Shell => "Shell",
            ListStyle::M3 => "M3 baseline",
            ListStyle::M3Expressive => "M3 Expressive",
        }
    }

    pub fn tokens(self) -> ListTokens {
        match self {
            ListStyle::Shell => shell(),
            ListStyle::M3 => m3(),
            ListStyle::M3Expressive => m3_expressive(),
        }
    }

    /// Whether the selection controls are drawn as M3's (only showing the
    /// state, the row takes the click) rather than the kit's own.
    pub fn material_controls(self) -> bool {
        self != ListStyle::Shell
    }
}

/// One line of text: its style and how tall its line is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub style: TextStyle,
    pub height: f32,
}

/// An item's corner radius in each state. `outer` is the radius the first
/// and last items of a segmented list round their outside corners to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shapes {
    pub base: f32,
    pub outer: f32,
    pub hovered: Option<f32>,
    pub focused: Option<f32>,
    pub pressed: Option<f32>,
    pub selected: Option<f32>,
    pub dragged: Option<f32>,
}

/// Not `Clone`: build another from its style.
pub struct ListTokens {
    pub style: ListStyle,

    /// Least height of a one, two and three-line item.
    pub heights: [f32; 3],
    pub leading_space: f32,
    pub trailing_space: f32,
    /// Above and below the content, by line count.
    pub vertical_padding: [f32; 3],
    /// Between the leading element and the text, and the text and trailing.
    pub leading_gap: f32,
    pub trailing_gap: f32,
    /// Between items: 0 for a plain list, a segmented list's gap.
    pub item_gap: f32,

    pub icon: f32,
    pub avatar: f32,
    pub image: f32,
    pub image_radius: f32,
    pub video_small: (f32, f32),
    pub video_large: (f32, f32),
    pub video_radius: f32,

    pub headline: Line,
    pub supporting: Line,
    pub overline: Line,
    pub trailing_text: Line,
    pub avatar_label: TextStyle,

    /// What the list sits on: the colour behind the items and their gaps.
    pub backdrop: Color,
    pub container: Color,
    pub headline_color: Color,
    pub supporting_color: Color,
    pub overline_color: Color,
    pub leading_color: Color,
    pub trailing_color: Color,
    pub avatar_fill: Color,
    pub avatar_text: Color,
    /// A selected item's container and content; none keeps the unselected.
    pub selected_container: Option<Color>,
    pub selected_content: Option<Color>,
    pub selected_headline: Option<Color>,
    /// The trailing icon of a selected item.
    pub selected_trailing: Color,
    pub dragged_container: Option<Color>,
    pub dragged_content: Option<Color>,
    /// The dragged item's drop shadow: opacity, blur and how far down.
    pub dragged_shadow: (f32, f32, f32),
    pub divider: Color,
    pub focus_ring: Color,
    pub focus_ring_width: f32,

    /// The state layer's colour (opaque) and its opacity in each state.
    pub state_layer: Color,
    pub hover_opacity: f32,
    pub focus_opacity: f32,
    pub pressed_opacity: f32,
    pub dragged_opacity: f32,
    /// What a disabled item's content is drawn at.
    pub disabled_opacity: f32,

    pub shapes: Shapes,

    /// For the M3-drawn controls.
    pub control_on: Color,
    pub control_on_mark: Color,
    pub control_off: Color,
    pub track_off: Color,
}

impl ListTokens {
    /// Least height of an item of `lines` lines (1 to 3).
    pub fn min_height(&self, lines: usize) -> f32 {
        self.heights[lines.clamp(1, 3) - 1]
    }

    pub fn padding_y(&self, lines: usize) -> f32 {
        self.vertical_padding[lines.clamp(1, 3) - 1]
    }

    /// The dragged item's drop shadow.
    pub fn drag_shadow(&self) -> Shadow {
        let (opacity, blur, y) = self.dragged_shadow;
        Shadow::drop(Color::BLACK)
            .opacity(opacity)
            .blur(blur)
            .offset(0.0, y)
    }

    /// The state layer at `opacity`, over `fill`.
    pub fn layer_over(&self, fill: Color, opacity: f32) -> Color {
        if opacity <= 0.0 {
            return fill;
        }
        over(with_alpha(self.state_layer, opacity), fill)
    }
}

/// An M3 type-scale role: `size`/`line` px at `weight`, `tracking` px,
/// in the shell's font, times the text scale.
fn m3_type(size: f32, line: f32, weight: u16, tracking: f32) -> Line {
    let scale = theme::text_scale();
    Line {
        style: TextStyle {
            family: sys::r#type::label().family,
            size: size * scale,
            weight,
            italic: false,
            line_height: Some(line / size),
            letter_spacing: Some(tracking * scale),
            text_case: None,
            text_decoration: None,
        },
        height: line * scale,
    }
}

/// Elevation level 4 (8 dp): a dragged item.
const LEVEL4: (f32, f32, f32) = (0.3, 12.0, 6.0);

fn shell() -> ListTokens {
    let d = sys::density::factor();
    let line = |style: TextStyle| Line {
        style,
        height: text_height(style).ceil() + 2.0,
    };
    let headline = line(sys::r#type::label());
    let caption = line(sys::r#type::caption());
    let hover = sys::color::state::hover();
    let hover_alpha = f32::from(hover.alpha()) / 255.0;
    let selected_alpha = f32::from(sys::color::state::selected().alpha()) / 255.0;
    let leading_side = sys::size::icon::m() + sys::space::gap::xs() * 2.0;
    let raised = sys::radius::raised();
    ListTokens {
        style: ListStyle::Shell,
        heights: [
            sys::size::row::height(),
            sys::size::row::two_line_height(),
            sys::size::row::two_line_height() + 16.0 * d,
        ],
        leading_space: sys::space::inset::l(),
        trailing_space: sys::space::inset::l(),
        vertical_padding: [sys::space::gap::xs(); 3],
        leading_gap: sys::space::gap::m(),
        trailing_gap: sys::space::gap::s(),
        item_gap: sys::space::gap::xs(),

        icon: sys::size::icon::m(),
        avatar: leading_side + 4.0 * d,
        image: sys::size::row::two_line_height() - sys::space::gap::xs() * 4.0,
        image_radius: sys::radius::raised() * 0.5,
        video_small: (64.0 * d, 36.0 * d),
        video_large: (80.0 * d, 45.0 * d),
        video_radius: sys::radius::raised() * 0.5,

        headline,
        supporting: caption,
        overline: caption,
        trailing_text: caption,
        avatar_label: sys::r#type::title(),

        backdrop: Color::TRANSPARENT,
        container: material::fill_role(FillLevel::Secondary),
        headline_color: material::label(Level::Secondary),
        supporting_color: material::label(Level::Secondary),
        overline_color: material::label(Level::Tertiary),
        leading_color: material::label(Level::Secondary),
        trailing_color: material::label(Level::Secondary),
        avatar_fill: sys::color::accent::bg(),
        avatar_text: sys::color::accent::fg(),
        selected_container: Some(sys::color::state::selected()),
        selected_content: Some(material::label(Level::Primary)),
        selected_headline: Some(material::label(Level::Primary)),
        selected_trailing: sys::color::accent::bg(),
        dragged_container: Some(sys::color::surface::raised::bg()),
        dragged_content: Some(material::label(Level::Primary)),
        dragged_shadow: (0.25, 10.0, 4.0),
        divider: material::separator(),
        focus_ring: sys::color::accent::bg(),
        focus_ring_width: 2.0,

        state_layer: Color::rgba(hover.red(), hover.green(), hover.blue(), 255),
        hover_opacity: hover_alpha,
        focus_opacity: hover_alpha,
        pressed_opacity: (hover_alpha * 1.75).min(1.0),
        dragged_opacity: selected_alpha,
        disabled_opacity: 0.38,

        shapes: Shapes {
            base: raised,
            outer: raised,
            hovered: None,
            focused: None,
            pressed: None,
            selected: None,
            dragged: None,
        },

        control_on: sys::color::accent::bg(),
        control_on_mark: sys::color::accent::fg(),
        control_off: material::label(Level::Secondary),
        track_off: material::fill_role(FillLevel::Primary),
    }
}

/// Material 3's baseline list item: Material Web tokens v0.192, with the
/// legacy Compose `ListItem`'s paddings (8 dp above and below, 12 on a
/// three-line item; 16 between the leading element and the text).
fn m3() -> ListTokens {
    let s = scheme::current();
    ListTokens {
        style: ListStyle::M3,
        heights: [56.0, 72.0, 88.0],
        leading_space: 16.0,
        trailing_space: 16.0,
        vertical_padding: [8.0, 8.0, 12.0],
        leading_gap: 16.0,
        trailing_gap: 16.0,
        item_gap: 0.0,

        icon: 24.0,
        avatar: 40.0,
        image: 56.0,
        image_radius: 0.0,
        video_small: (100.0, 56.0),
        video_large: (100.0, 69.0),
        video_radius: 0.0,

        // body-large, body-medium, label-small, label-small
        headline: m3_type(16.0, 24.0, 400, 0.5),
        supporting: m3_type(14.0, 20.0, 400, 0.25),
        overline: m3_type(11.0, 16.0, 500, 0.5),
        trailing_text: m3_type(11.0, 16.0, 500, 0.5),
        // title-medium
        avatar_label: m3_type(16.0, 24.0, 500, 0.15).style,

        backdrop: s.surface,
        container: s.surface,
        headline_color: s.on_surface,
        supporting_color: s.on_surface_variant,
        overline_color: s.on_surface_variant,
        leading_color: s.on_surface_variant,
        trailing_color: s.on_surface_variant,
        avatar_fill: s.primary_container,
        avatar_text: s.on_primary_container,
        // the baseline only marks a selected item by its trailing icon
        selected_container: None,
        selected_content: None,
        selected_headline: None,
        selected_trailing: s.primary,
        dragged_container: None,
        dragged_content: None,
        dragged_shadow: LEVEL4,
        divider: s.outline_variant,
        focus_ring: s.secondary,
        focus_ring_width: 3.0,

        state_layer: s.on_surface,
        hover_opacity: 0.08,
        focus_opacity: 0.12,
        pressed_opacity: 0.12,
        dragged_opacity: 0.16,
        disabled_opacity: 0.38,

        shapes: Shapes {
            base: 0.0,
            outer: 0.0,
            hovered: None,
            focused: None,
            pressed: None,
            selected: None,
            dragged: None,
        },

        control_on: s.primary,
        control_on_mark: s.on_primary,
        control_off: s.on_surface_variant,
        track_off: s.surface_container_highest,
    }
}

/// Material 3 Expressive: Compose material3 `ListTokens` 29.0.0. Shapes
/// from the corner scale: extra-small 4, medium 12, large 16.
fn m3_expressive() -> ListTokens {
    let s = scheme::current();
    let base = m3();
    ListTokens {
        style: ListStyle::M3Expressive,
        // ListItemDefaults.ContentPadding: 10 dp above and below,
        // InteractiveListInternalSpacing 12 dp either side of the text
        vertical_padding: [10.0; 3],
        leading_gap: 12.0,
        trailing_gap: 12.0,
        // ListItemDefaults.SegmentedGap
        item_gap: 2.0,

        // ItemLeadingIconExpressiveSize, ItemTrailingIconExpressiveSize
        icon: 20.0,
        // ItemLeadingImageExpressiveShape, ItemLeadingVideoShape: corner small
        image_radius: 8.0,
        video_large: (114.0, 64.0),
        video_radius: 8.0,

        // a segmented list's items on their container, 2 dp apart
        backdrop: s.surface_container,
        container: s.surface,
        selected_container: Some(s.secondary_container),
        selected_content: Some(s.on_secondary_container),
        selected_headline: Some(s.on_secondary_container),
        selected_trailing: s.on_secondary_container,
        // ReorderListTokens
        dragged_container: Some(s.tertiary_container),
        dragged_content: Some(s.on_tertiary_container),
        focus_ring_width: 3.0,

        shapes: Shapes {
            base: 4.0,
            outer: 16.0,
            hovered: Some(12.0),
            focused: Some(16.0),
            pressed: Some(16.0),
            selected: Some(16.0),
            dragged: Some(16.0),
        },
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m3_baseline_numbers_match_material_web() {
        let t = m3();
        assert_eq!(t.heights, [56.0, 72.0, 88.0]);
        assert_eq!((t.leading_space, t.trailing_space), (16.0, 16.0));
        assert_eq!((t.icon, t.avatar, t.image), (24.0, 40.0, 56.0));
        assert_eq!(t.video_small, (100.0, 56.0));
        assert_eq!(t.video_large, (100.0, 69.0));
        assert_eq!(
            (
                t.hover_opacity,
                t.focus_opacity,
                t.pressed_opacity,
                t.dragged_opacity
            ),
            (0.08, 0.12, 0.12, 0.16)
        );
        assert_eq!(t.shapes.base, 0.0);
        assert_eq!(t.item_gap, 0.0);
    }

    #[test]
    fn m3_expressive_numbers_match_compose() {
        let t = m3_expressive();
        assert_eq!(t.heights, [56.0, 72.0, 88.0]);
        assert_eq!(t.vertical_padding, [10.0; 3]);
        assert_eq!((t.leading_gap, t.trailing_gap), (12.0, 12.0));
        assert_eq!(t.item_gap, 2.0);
        assert_eq!(t.icon, 20.0);
        assert_eq!(t.video_large, (114.0, 64.0));
        assert_eq!((t.shapes.base, t.shapes.outer), (4.0, 16.0));
        assert_eq!(t.shapes.hovered, Some(12.0));
        assert_eq!(t.shapes.pressed, Some(16.0));
        assert!(t.selected_container.is_some());
    }

    #[test]
    fn m3_type_scale() {
        let t = m3();
        // at text scale 1
        if (theme::text_scale() - 1.0).abs() < f32::EPSILON {
            assert_eq!(t.headline.height, 24.0);
            assert_eq!(t.supporting.height, 20.0);
            assert_eq!(t.overline.height, 16.0);
            assert_eq!(t.headline.style.size, 16.0);
        }
    }
}
