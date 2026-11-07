//! The bar's three sizes ([`BarSize`]): how tall it is, how big its glyphs
//! and items are, and the one text size its labels share.
//!
//! Compact is the bar it always was, read from the tokens it always read.
//! Regular and Comfortable are held here until Penpot has them
//! (`docs/design-system/penpot-backlog.md`).

use amane_kit::settings::{BarSize, status_bar};

use crate::tokens::{TextStyle, bar, sys};

/// The glyph, item width and padding: TODO(token):
/// `bar.size.regular.{icon,item-width,padding-y}`. Its height is
/// [`BarSize::height`], which the apps read too.
const REGULAR: (f32, f32, f32) = (16.0, 30.0, 2.0);

/// TODO(token): `bar.size.comfortable.{icon,item-width,padding-y}`.
const COMFORTABLE: (f32, f32, f32) = (24.0, 40.0, 3.0);

/// TODO(token): `bar.label` (13 px on a 16 px line) and
/// `bar.label-comfortable` (14 px).
const LABEL: f32 = 13.0;
const LABEL_COMFORTABLE: f32 = 14.0;
pub const LINE_HEIGHT: f32 = 16.0;

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
            BarSize::Regular => REGULAR,
            BarSize::Comfortable => COMFORTABLE,
        };
        let label = if size == BarSize::Comfortable {
            LABEL_COMFORTABLE
        } else {
            LABEL
        };
        Self {
            height,
            icon,
            // the chips' camera and microphone, three quarters of a glyph
            small_icon: (icon * 0.75).round(),
            item_width,
            padding_y,
            label,
        }
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
    }
}
