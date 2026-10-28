//! Where the launcher's glass is: one sheet that's a pill while nothing is
//! being searched and grows down into a panel when something is, and the
//! four round mode buttons beside the pill, there only while it's a pill.
//! [`Geometry`] blends, so one spring carries the sheet and the buttons
//! between the two, the frost with them. Pure, so the tests run it.

use amane::{Blend, BlurShape};

use crate::components::material::Material;

// TODO(token): `launcher.pill.*`, `launcher.mode-button.*`,
// `launcher.sheet.radius` and `launcher.top` once Penpot has them
// (manual §1.1).
/// The pill: as wide as a search row, and round-ended.
pub(super) const PILL_W: f32 = 680.0;
pub(super) const PILL_H: f32 = 52.0;
/// A mode button, as tall as the pill, and the room between them.
pub(super) const CIRCLE: f32 = 52.0;
pub(super) const CIRCLE_GAP: f32 = 12.0;
/// The panel's corners once the pill has grown into one.
pub(super) const SHEET_RADIUS: f32 = 26.0;
/// How far down the screen the pill sits, as a share of its height.
const TOP: f32 = 0.2;
/// The least room between the glass and the screen's edges.
const MARGIN: f32 = 16.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Rect {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
}

impl Blend for Rect {
    fn blend(from: Self, to: Self, amount: f32) -> Self {
        Self {
            x: f32::blend(from.x, to.x, amount),
            y: f32::blend(from.y, to.y, amount),
            width: f32::blend(from.width, to.width, amount),
            height: f32::blend(from.height, to.height, amount),
        }
    }
}

/// The glass this moment, in the surface's coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Geometry {
    pub(super) sheet: Rect,
    pub(super) radius: f32,
    pub(super) circles: [Rect; 4],
    /// How much the mode buttons show: 1 beside the pill, 0 gone.
    pub(super) circles_shown: f32,
}

impl Blend for Geometry {
    fn blend(from: Self, to: Self, amount: f32) -> Self {
        let mut circles = to.circles;
        for (at, circle) in circles.iter_mut().enumerate() {
            *circle = Rect::blend(from.circles[at], to.circles[at], amount);
        }
        Self {
            sheet: Rect::blend(from.sheet, to.sheet, amount),
            radius: f32::blend(from.radius, to.radius, amount),
            circles,
            circles_shown: f32::blend(from.circles_shown, to.circles_shown, amount),
        }
    }
}

/// Whether the mode buttons fit beside the pill on a `screen_w` wide
/// output; if not, they go under it.
pub(super) fn fits_beside(screen_w: f32) -> bool {
    let buttons = 4.0 * CIRCLE + 4.0 * CIRCLE_GAP;
    (screen_w - PILL_W) / 2.0 >= buttons + MARGIN
}

/// Where the glass rests on a `screen` sized surface: the sheet `size`
/// (a pill while `compact`), centred across the screen with its top a
/// fifth of the way down, and the mode buttons beside the pill, showing
/// only while it's one.
pub(super) fn target(screen: (f32, f32), size: (f32, f32), compact: bool) -> Geometry {
    let (screen_w, screen_h) = screen;
    let (width, height) = if compact { (PILL_W, PILL_H) } else { size };
    // the top stays put as the pill grows, unless the panel wouldn't fit
    let top = (screen_h * TOP).min(screen_h - height - MARGIN).max(MARGIN);
    let sheet = Rect {
        x: ((screen_w - width) / 2.0).max(0.0),
        y: top,
        width,
        height,
    };

    let pill_x = ((screen_w - PILL_W) / 2.0).max(0.0);
    let pill_top = (screen_h * TOP).min(screen_h - PILL_H - MARGIN).max(MARGIN);
    let mut circles = [Rect::default(); 4];
    let beside = fits_beside(screen_w);
    let row = 4.0 * CIRCLE + 3.0 * CIRCLE_GAP;
    for (at, circle) in circles.iter_mut().enumerate() {
        let along = at as f32 * (CIRCLE + CIRCLE_GAP);
        *circle = if beside {
            Rect {
                x: pill_x + PILL_W + CIRCLE_GAP + along,
                y: pill_top,
                width: CIRCLE,
                height: CIRCLE,
            }
        } else {
            Rect {
                x: (screen_w - row) / 2.0 + along,
                y: pill_top + PILL_H + CIRCLE_GAP,
                width: CIRCLE,
                height: CIRCLE,
            }
        };
    }

    Geometry {
        sheet,
        radius: if compact { PILL_H / 2.0 } else { SHEET_RADIUS },
        circles,
        circles_shown: if compact { 1.0 } else { 0.0 },
    }
}

impl Geometry {
    /// The sheet as it's drawn: never smaller than the pill, nor rounder
    /// than half its height, whatever the spring overshoots to.
    pub(super) fn drawn(self) -> (Rect, f32) {
        let sheet = Rect {
            width: self.sheet.width.max(PILL_H),
            height: self.sheet.height.max(PILL_H),
            ..self.sheet
        };
        (sheet, self.radius.clamp(0.0, sheet.height / 2.0))
    }

    pub(super) fn shown(self) -> f32 {
        self.circles_shown.clamp(0.0, 1.0)
    }

    /// Where the compositor frosts: behind the sheet, and the buttons as
    /// much as they show.
    pub(super) fn frost(self) -> Vec<BlurShape> {
        let (sheet, radius) = self.drawn();
        let mut shapes =
            vec![Material::Sheet.shape(sheet.x, sheet.y, sheet.width, sheet.height, radius, 1.0)];
        let shown = self.shown();
        if shown > 0.01 {
            for circle in self.circles {
                shapes.push(Material::Control.shape(
                    circle.x,
                    circle.y,
                    circle.width,
                    circle.height,
                    CIRCLE / 2.0,
                    shown,
                ));
            }
        }
        shapes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (f32, f32) = (1645.0, 1028.0);

    #[test]
    fn the_pill_has_its_buttons_beside_it() {
        let pill = target(SCREEN, (900.0, 500.0), true);
        assert_eq!(pill.sheet.width, PILL_W);
        assert_eq!(pill.sheet.height, PILL_H);
        assert_eq!(pill.radius, PILL_H / 2.0);
        assert_eq!(pill.circles_shown, 1.0);
        assert_eq!(pill.circles[0].x, pill.sheet.x + PILL_W + CIRCLE_GAP);
        assert_eq!(pill.circles[0].y, pill.sheet.y);
        assert_eq!(pill.circles[3].x - pill.circles[2].x, CIRCLE + CIRCLE_GAP);
    }

    #[test]
    fn the_panel_grows_down_from_the_pills_top() {
        let pill = target(SCREEN, (0.0, 0.0), true);
        let panel = target(SCREEN, (PILL_W, 400.0), false);
        assert_eq!(panel.sheet.y, pill.sheet.y);
        assert_eq!(panel.sheet.x, pill.sheet.x);
        assert_eq!(panel.sheet.height, 400.0);
        assert_eq!(panel.circles_shown, 0.0);
        // the buttons fade where they are
        assert_eq!(panel.circles, pill.circles);
    }

    #[test]
    fn a_tall_panel_rises_to_fit() {
        let panel = target(SCREEN, (PILL_W, 1000.0), false);
        assert_eq!(panel.sheet.y, MARGIN);
        let short = target((1645.0, 600.0), (PILL_W, 500.0), false);
        assert_eq!(short.sheet.y, 600.0 - 500.0 - MARGIN);
    }

    #[test]
    fn a_narrow_screen_puts_the_buttons_under_the_pill() {
        assert!(fits_beside(SCREEN.0));
        assert!(!fits_beside(900.0));
        let pill = target((900.0, 800.0), (0.0, 0.0), true);
        assert_eq!(pill.circles[0].y, pill.sheet.y + PILL_H + CIRCLE_GAP);
    }

    #[test]
    fn the_drawn_sheet_never_turns_inside_out() {
        let squashed = Geometry {
            sheet: Rect {
                x: 0.0,
                y: 0.0,
                width: 600.0,
                height: 30.0,
            },
            radius: 40.0,
            ..Geometry::default()
        };
        let (sheet, radius) = squashed.drawn();
        assert_eq!(sheet.height, PILL_H);
        assert_eq!(radius, PILL_H / 2.0);
    }
}
