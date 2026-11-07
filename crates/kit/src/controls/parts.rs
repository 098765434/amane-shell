//! What the form controls share: a disabled control at half strength, a click's actions, and a
//! mark with its label (the checkbox's and the radio button's shape).

use amane::{Center, Rectangle, Row, Size, Text, Widget};

use crate::layout::{boxed, text_width};
use crate::style::{self, px};
use crate::tokens::Styled;

/// A disabled control's opacity.
pub(crate) const DISABLED_OPACITY: f32 = 0.5;

/// The kit's [`interactive`](crate::buttons::interactive), at half strength while `disabled`.
pub(crate) fn interactive(
    base: Rectangle,
    id: String,
    disabled: bool,
    actions: crate::buttons::Actions,
) -> Rectangle {
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
        width += gap + text_width(&label, style::control_font()).ceil();
        parts.push(boxed(
            Text::new(label)
                .style(style::control_font())
                .color(style::label_primary()),
        ));
    }
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(style::control_height()))
        .align_child(amane::Start, Center)
        .child(Row::new(parts).gap(gap).align(Center))
}
