//! The round mode buttons beside the pill: they bud off its end as the
//! launcher opens, fade and slide in under it as it grows into the panel,
//! and lift under the pointer.

use amane::{Center, Rectangle, Widget};

use emaki_kit::layout::{boxed, icon};
use emaki_kit::material::{self, Level, Material};

use super::mode::Mode;
use super::shape::{Bud, CIRCLE};
use super::{LIFT, switch_mode, with};

// TODO(token): `launcher.mode-button.glyph` and `.lift` once Penpot has
// them (manual §1.1).
/// How big a mode button's glyph is, and how much it lifts under the
/// pointer.
const MODE_GLYPH: f32 = 24.0;
const MODE_LIFT: f32 = 1.06;

/// The round mode buttons as `buds` draws them, `picked` the mode
/// showing: they bud off the pill as the launcher opens, and fade and
/// slide in under it as it grows. Under the pointer, one lifts on a spring.
pub(super) fn mode_buttons(buds: &[Bud; 4], picked: Mode) -> Vec<Box<dyn Widget>> {
    let hovered = with(|state| state.hover_mode);
    let lifts: [f32; 4] = LIFT.with_borrow_mut(|lifts| {
        std::array::from_fn(|at| {
            let mode = Mode::PICKED[at];
            let lifted = hovered == Some(mode) || picked == mode;
            lifts[at].to(if lifted { MODE_LIFT } else { 1.0 });
            lifts[at].value()
        })
    });
    Mode::PICKED
        .iter()
        .zip(buds)
        .zip(lifts)
        .filter(|((_, bud), _)| bud.shown > 0.01)
        .map(|((&mode, bud), lift)| {
            let glass = Rectangle::new()
                .width(CIRCLE)
                .height(CIRCLE)
                .radius(CIRCLE / 2.0)
                .fill(Material::Control.fill())
                .align_child(Center, Center)
                .on_hover(move |inside| {
                    let changed = with(|state| {
                        let before = state.hover_mode;
                        if inside {
                            state.hover_mode = Some(mode);
                        } else if before == Some(mode) {
                            state.hover_mode = None;
                        }
                        before != state.hover_mode
                    });
                    if changed {
                        super::Launcher::changed();
                    }
                })
                .on_click(move |_| {
                    switch_mode(mode);
                    super::Launcher::changed();
                })
                .child(icon(
                    mode.glyph(),
                    MODE_GLYPH,
                    material::label(Level::Primary),
                ));
            boxed(
                Material::Control
                    .lit(glass, CIRCLE / 2.0)
                    .opacity(bud.shown)
                    .scale(bud.scale * lift)
                    .translate(bud.centre.0 - CIRCLE / 2.0, bud.centre.1 - CIRCLE / 2.0),
            )
        })
        .collect()
}
