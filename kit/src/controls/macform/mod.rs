//! macform — macOS-style form controls, ported from the Quickshell
//! `components/macform` set for a side-by-side look against the kit's own.
//!
//! Experimental: nothing in the shell uses these yet. `cargo run -p
//! amane-kit --example macform` shows each one next to its kit counterpart.
//!
//! They follow the kit's conventions: a builder per control, `build()`
//! giving a `Rectangle`, closures for what input does, and hover and
//! motion kept in the kit's tables under each control's `id` (so ids must
//! be unique and stable). Colours and sizes come from [`theme`], which
//! reads the kit's tokens where the original's values match them.
//!
//! The original's Material press ripple is left out; a hover layer at 8%
//! of the content colour and the kit's press pulse stand in for it.

mod combo_box;
mod leading_icon;
pub(crate) mod menu;
mod pop_up_button;
mod push_button;
mod segmented_nav;
mod slider;
mod stepper;
mod switch;
pub(crate) mod text_field;

pub mod symbol;
pub mod theme;

pub use combo_box::MacComboBox;
pub use leading_icon::MacLeadingIcon;
pub use menu::MenuItem;
pub use pop_up_button::MacPopUpButton;
pub use push_button::MacPushButton;
pub use segmented_nav::MacSegmentedNav;
pub use slider::MacSlider;
pub use stepper::MacStepper;
pub use switch::MacSwitch;
pub use symbol::{Symbol, symbol};
pub use text_field::MacTextField;

use amane::{Animation, Blend, Center, Color, Rectangle, Row, Size, Text, Widget};

use crate::buttons::{hovered, with_state};
use crate::color::over;
use crate::layout::{boxed, text_width};
use crate::motion;
use crate::tokens::Styled;
use theme::px;

/// The original's disabled opacity.
const DISABLED_OPACITY: f32 = 0.5;

/// `fill` with an 8% layer of `content` over it while `id` is hovered,
/// fading in and out at hover speed.
pub(crate) fn hover_fill(id: &str, fill: Color, content: Color, disabled: bool) -> Color {
    let on = !disabled && hovered(id);
    let amount = fade(&format!("{id}.mac-hover"), on, motion::HOVER);
    Color::blend(fill, over(theme::hover_of(content), fill), amount)
}

/// A value fading to 1 while `on` and to 0 while not, over `duration`.
fn fade(key: &str, on: bool, duration: std::time::Duration) -> f32 {
    let target = if on { 1.0 } else { 0.0 };
    with_state(
        key,
        || motion::fade(target, duration),
        |fade: &mut Animation| {
            fade.to(target);
            fade.value().clamp(0.0, 1.0)
        },
    )
}

/// The original's colour crossfade (300 ms) between `off` and `on`.
pub(crate) fn crossfade(key: &str, on: bool, off_color: Color, on_color: Color) -> Color {
    let amount = fade(key, on, std::time::Duration::from_millis(300));
    Color::blend(off_color, on_color, amount)
}

/// The kit's [`interactive`](crate::buttons::interactive), with the
/// original's 50% for a disabled control.
fn interactive(
    base: amane::Rectangle,
    id: String,
    disabled: bool,
    actions: crate::buttons::Actions,
) -> amane::Rectangle {
    if disabled {
        return base.opacity(DISABLED_OPACITY);
    }
    crate::buttons::interactive(base, id, false, actions)
}

/// Actions with only a left click.
pub(crate) fn on_press(press: impl Fn() + 'static) -> crate::buttons::Actions {
    crate::buttons::Actions {
        press: Some(std::rc::Rc::new(press)),
        ..Default::default()
    }
}

/// A mark and its optional label, 3 px apart, the shared shape of the
/// checkbox and the radio button.
pub(crate) fn labelled(mark: Rectangle, label: Option<String>) -> Rectangle {
    let gap = px(3.0);
    let mut width = px(16.0);
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(mark)];
    if let Some(label) = label {
        width += gap + text_width(&label, theme::control_font()).ceil();
        parts.push(boxed(
            Text::new(label)
                .style(theme::control_font())
                .color(theme::label_primary()),
        ));
    }
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(theme::control_height()))
        .align_child(amane::Start, Center)
        .child(Row::new(parts).gap(gap).align(Center))
}
