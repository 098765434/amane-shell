//! Where the status bar is, for the surfaces that keep clear of it: the
//! edge it runs along and how tall it is. Both come from its settings, so a
//! surface asks here rather than the bar.

use amane::Vertical;
use amane_kit::settings::{BarPosition, status_bar};

/// The edge the bar runs along.
pub fn edge() -> Vertical {
    match status_bar().position {
        BarPosition::Bottom => Vertical::Bottom,
        BarPosition::Top => Vertical::Top,
    }
}

/// How tall the bar is.
pub fn height() -> f32 {
    status_bar().size.height()
}
