//! Where the launcher's glass is: one sheet that's a pill while nothing is
//! being searched and grows down into a panel when something is, and the
//! four round mode buttons beside the pill, there only while it's a pill.
//! [`Geometry`] blends, so one spring carries the sheet and the buttons
//! between the two, the frost with them. Pure, so the tests run it.

use amane::{Blend, BlurShape};

use emaki_kit::material::Material;

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
/// How small the mode buttons get as they slide in under the panel.
const PANEL_SCALE: f32 = 0.85;

pub(super) use emaki_kit::area::Area;

/// The glass this moment, in the surface's coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Geometry {
    pub(super) sheet: Area,
    pub(super) radius: f32,
    pub(super) circles: [Area; 4],
    /// How much the mode buttons show: 1 beside the pill, 0 gone.
    pub(super) circles_shown: f32,
    /// Whether the mode buttons sit beside the pill, else under it.
    pub(super) beside: bool,
}

impl Blend for Geometry {
    fn blend(from: Self, to: Self, amount: f32) -> Self {
        let mut circles = to.circles;
        for (at, circle) in circles.iter_mut().enumerate() {
            *circle = Area::blend(from.circles[at], to.circles[at], amount);
        }
        Self {
            sheet: Area::blend(from.sheet, to.sheet, amount),
            radius: f32::blend(from.radius, to.radius, amount),
            circles,
            circles_shown: f32::blend(from.circles_shown, to.circles_shown, amount),
            beside: to.beside,
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
    let sheet = Area {
        x: ((screen_w - width) / 2.0).max(0.0),
        y: top,
        width,
        height,
    };

    let pill_x = ((screen_w - PILL_W) / 2.0).max(0.0);
    let pill_top = (screen_h * TOP).min(screen_h - PILL_H - MARGIN).max(MARGIN);
    let mut circles = [Area::default(); 4];
    let beside = fits_beside(screen_w);
    let row = 4.0 * CIRCLE + 3.0 * CIRCLE_GAP;
    for (at, circle) in circles.iter_mut().enumerate() {
        let along = at as f32 * (CIRCLE + CIRCLE_GAP);
        *circle = if beside {
            Area {
                x: pill_x + PILL_W + CIRCLE_GAP + along,
                y: pill_top,
                width: CIRCLE,
                height: CIRCLE,
            }
        } else {
            Area {
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
        beside,
    }
}

/// Where the glass starts as it opens from closed: the sheet a drop, as
/// round as it is tall, at the centre of where it rests (`target`, the
/// pill), from which it stretches out sideways. The buttons bud off it on
/// their own clock ([`Geometry::buds`]).
pub(super) fn seed(target: Geometry) -> Geometry {
    let (centre_x, centre_y) = target.sheet.centre();
    Geometry {
        sheet: Area {
            x: centre_x - PILL_H / 2.0,
            y: centre_y - PILL_H / 2.0,
            width: PILL_H,
            height: PILL_H,
        },
        radius: PILL_H / 2.0,
        ..target
    }
}

/// A mode button as it's drawn this frame: where its centre is, how big it
/// is (1 its size) and how strongly it shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Bud {
    pub(super) centre: (f32, f32),
    pub(super) scale: f32,
    pub(super) shown: f32,
}

impl Bud {
    /// Where it's drawn, scaled about its centre.
    pub(super) fn area(self) -> Area {
        let side = CIRCLE * self.scale;
        Area {
            x: self.centre.0 - side / 2.0,
            y: self.centre.1 - side / 2.0,
            width: side,
            height: side,
        }
    }
}

impl Geometry {
    /// The sheet as it's drawn: never smaller than the pill, nor rounder
    /// than half its height, whatever the spring overshoots to.
    pub(super) fn drawn(self) -> (Area, f32) {
        let sheet = Area {
            width: self.sheet.width.max(PILL_H),
            height: self.sheet.height.max(PILL_H),
            ..self.sheet
        };
        (sheet, self.radius.clamp(0.0, sheet.height / 2.0))
    }

    pub(super) fn shown(self) -> f32 {
        self.circles_shown.clamp(0.0, 1.0)
    }

    /// The mode buttons, each `budded` this far off the sheet's end (0
    /// tucked into it … 1 at rest, a little past on the spring), growing
    /// from `from_scale`; as the sheet grows into the panel they slide in
    /// under it by `slide` and fade. Unless they `travel` (where motion is
    /// reduced), they fade in where they rest rather than move out.
    pub(super) fn buds(
        self,
        budded: [f32; 4],
        from_scale: f32,
        slide: f32,
        travel: bool,
    ) -> [Bud; 4] {
        let (sheet, _) = self.drawn();
        // the end of the sheet they bud from: its right end beside the
        // pill, its middle under it
        let tucked = if self.beside {
            (
                sheet.x + sheet.width - CIRCLE / 2.0,
                sheet.y + sheet.height / 2.0,
            )
        } else {
            (
                sheet.x + sheet.width / 2.0,
                sheet.y + sheet.height - CIRCLE / 2.0,
            )
        };
        let shown = self.shown();
        let mut buds = [Bud {
            centre: tucked,
            scale: 1.0,
            shown: 0.0,
        }; 4];
        for (bud, (circle, amount)) in buds.iter_mut().zip(self.circles.iter().zip(budded)) {
            let (rest_x, rest_y) = circle.centre();
            let rest = (rest_x - (1.0 - shown) * slide, rest_y);
            let out = if travel { amount } else { 1.0 };
            *bud = Bud {
                centre: (
                    f32::blend(tucked.0, rest.0, out),
                    f32::blend(tucked.1, rest.1, out),
                ),
                scale: f32::blend(from_scale, 1.0, amount.min(1.0))
                    * f32::blend(PANEL_SCALE, 1.0, shown),
                shown: shown * amount.clamp(0.0, 1.0),
            };
        }
        buds
    }

    /// Where the compositor frosts: behind the sheet, and the buttons
    /// (`buds`) as much as they show.
    pub(super) fn frost(self, buds: &[Bud]) -> Vec<BlurShape> {
        let (sheet, radius) = self.drawn();
        let mut shapes =
            vec![Material::Sheet.shape(sheet.x, sheet.y, sheet.width, sheet.height, radius, 1.0)];
        for bud in buds.iter().filter(|bud| bud.shown > 0.01) {
            let area = bud.area();
            shapes.push(Material::Control.shape(
                area.x,
                area.y,
                area.width,
                area.height,
                area.width / 2.0,
                bud.shown,
            ));
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
            sheet: Area {
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

    #[test]
    fn the_glass_opens_from_a_drop_at_the_pills_centre() {
        let pill = target(SCREEN, (0.0, 0.0), true);
        let drop = seed(pill);
        assert_eq!(drop.sheet.width, PILL_H);
        assert_eq!(drop.sheet.height, PILL_H);
        assert_eq!(drop.radius, PILL_H / 2.0);
        assert_eq!(drop.sheet.centre(), pill.sheet.centre());
        assert_eq!(drop.circles, pill.circles);
        assert_eq!(Geometry::blend(drop, pill, 1.0), pill);
    }

    #[test]
    fn the_buttons_bud_off_the_pills_end() {
        let pill = target(SCREEN, (0.0, 0.0), true);
        let tucked = pill.buds([0.0; 4], 0.5, 12.0, true);
        for bud in tucked {
            assert_eq!(bud.centre.0, pill.sheet.x + PILL_W - CIRCLE / 2.0);
            assert_eq!(bud.shown, 0.0);
            assert_eq!(bud.scale, 0.5);
        }
        let rest = pill.buds([1.0; 4], 0.5, 12.0, true);
        for (bud, circle) in rest.iter().zip(pill.circles) {
            assert_eq!(bud.centre, circle.centre());
            assert_eq!(bud.scale, 1.0);
            assert_eq!(bud.shown, 1.0);
            assert_eq!(bud.area(), circle);
        }
        // only what shows is frosted
        assert_eq!(pill.frost(&tucked).len(), 1);
        assert_eq!(pill.frost(&rest).len(), 5);
    }

    #[test]
    fn under_the_pill_they_bud_from_its_middle() {
        let pill = target((900.0, 800.0), (0.0, 0.0), true);
        let bud = pill.buds([0.0; 4], 0.5, 0.0, true)[2];
        assert_eq!(bud.centre.0, pill.sheet.x + PILL_W / 2.0);
    }
}
