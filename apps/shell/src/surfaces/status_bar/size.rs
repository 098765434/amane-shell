//! The bar's three sizes ([`BarSize`]): how tall it is, how big its glyphs
//! and items are, and the one text size its labels share.
//!
//! Compact is the bar it always was, read from the tokens it always read;
//! Regular and Comfortable read `bar.size.regular.*` and
//! `bar.size.comfortable.*`. Their height is [`BarSize::height`], which the
//! apps read too.

use emaki_kit::settings::{BarSize, status_bar};

use crate::tokens::{TextStyle, bar, sys};

/// The labels' line, whatever their size (`bar.label.size` 13 px,
/// `bar.label.size-comfortable` 14 px).
pub const LINE_HEIGHT: f32 = 16.0;

/// How much bigger a picture (an app's icon, a song's art) is than a
/// glyph, so its rounded corners don't make it look smaller.
pub const PICTURE_OVER: f32 = 2.0;

/// What one size of bar is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// The strip's height.
    pub height: f32,
    /// A status glyph's size.
    pub icon: f32,
    /// A small glyph's, as the privacy chips'.
    pub small_icon: f32,
    /// A lone glyph's item, the hover tile's width.
    pub item_width: f32,
    /// Room above and below the items.
    pub padding_y: f32,
    /// Room between the items, a little more as they grow.
    pub gap: f32,
    /// Room each side of an item with text in it (`sys.space.inset.s`).
    pub item_pad: f32,
    /// Room between the parts of an item: its picture, its text and a
    /// badge (`sys.space.gap.xs`, as a push button's icon and label).
    pub inner_gap: f32,
    /// The labels' size, in pixels before the text scale.
    label: f32,
}

impl Metrics {
    pub fn of(size: BarSize) -> Self {
        let height = size.height();
        let (icon, item_width, padding_y) = match size {
            BarSize::Compact => (
                sys::size::icon::m(),
                bar::indicator::width(),
                bar::panel::padding_y(),
            ),
            BarSize::Regular => {
                use bar::size::regular;
                (regular::icon(), regular::item_width(), regular::padding_y())
            }
            BarSize::Comfortable => {
                use bar::size::comfortable;
                (
                    comfortable::icon(),
                    comfortable::item_width(),
                    comfortable::padding_y(),
                )
            }
        };
        let gap = if size == BarSize::Comfortable {
            bar::size::comfortable::gap()
        } else {
            sys::space::gap::_2xs()
        };
        let label = if size == BarSize::Comfortable {
            bar::label::size_comfortable()
        } else {
            bar::label::size()
        };
        Self {
            height,
            icon,
            // the chips' camera and microphone, three quarters of a glyph
            small_icon: (icon * 0.75).round(),
            item_width,
            padding_y,
            gap,
            item_pad: sys::space::inset::s(),
            inner_gap: sys::space::gap::xs(),
            label,
        }
    }

    /// A picture's size: a little over a glyph ([`PICTURE_OVER`]).
    pub fn picture(&self) -> f32 {
        self.icon + PICTURE_OVER
    }

    /// The width of an item whose parts are `content` wide: padded each
    /// side, and never narrower than a lone glyph's.
    pub fn item_width_for(&self, content: f32) -> f32 {
        (content + self.item_pad * 2.0).max(self.item_width)
    }

    /// How wide text capped at `base` at Compact's text size may be at
    /// this size: the cap grows with the text.
    pub fn text_cap(&self, base: f32) -> f32 {
        (base * self.label / bar::label::size()).round()
    }

    /// The bar's text: plain (`sys.type.label`'s weight).
    pub fn label(&self) -> TextStyle {
        self.sized(sys::r#type::label())
    }

    /// Figures, as the clock's and the weather's (`sys.type.numeric`).
    pub fn numeric(&self) -> TextStyle {
        self.sized(sys::r#type::numeric())
    }

    /// `base` at the bar's label size on a [`LINE_HEIGHT`] line, keeping
    /// its family, weight and the user's text scale.
    fn sized(&self, base: TextStyle) -> TextStyle {
        // `sys.type.label` is 13 px before the text scale
        let scale = sys::r#type::label().size / 13.0;
        TextStyle {
            size: self.label * scale,
            line_height: Some(LINE_HEIGHT / self.label),
            ..base
        }
    }
}

/// The bar's size now.
pub fn metrics() -> Metrics {
    Metrics::of(status_bar().size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sizes_grow() {
        let compact = Metrics::of(BarSize::Compact);
        let regular = Metrics::of(BarSize::Regular);
        let comfortable = Metrics::of(BarSize::Comfortable);

        assert_eq!((regular.height, regular.icon), (24.0, 16.0));
        assert_eq!((comfortable.height, comfortable.icon), (32.0, 24.0));
        assert!(compact.height < regular.height);
        assert!(compact.icon <= compact.height);
        assert!(compact.gap <= comfortable.gap);
        assert!(comfortable.text_cap(120.0) > compact.text_cap(120.0));
    }

    #[test]
    fn the_hover_tile_holds_a_glyph() {
        for size in [BarSize::Compact, BarSize::Regular, BarSize::Comfortable] {
            let metrics = Metrics::of(size);
            assert!(
                metrics.height - metrics.padding_y * 2.0 >= metrics.icon,
                "{size:?}: the tile is shorter than its glyph"
            );
        }
    }
}
