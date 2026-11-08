//! A notification's buttons: its own actions, the phone's, and the close button.

use super::*;

/// Whether the card has a row of buttons: the sender's, or Open on Phone.
pub(super) fn has_actions(n: &Notification) -> bool {
    !n.actions().is_empty() || !crate::services::phone::notifications::commands(n).is_empty()
}

/// The sender's actions as equal buttons across `width`, then Open in
/// Messages and Open on Phone for one of the phone's notifications.
pub(super) fn actions(n: &Notification, width: f32, color: Color) -> Row {
    let phone = crate::services::phone::notifications::commands(n);
    let count = (n.actions().len() + phone.len()) as f32;
    let gap = sys::space::gap::s();
    let each = ((width - gap * (count - 1.0)) / count).max(1.0);
    let id = n.id();

    let mut buttons: Vec<Box<dyn Widget>> = n
        .actions()
        .iter()
        .map(|action| {
            let key = action.key().to_string();
            boxed(
                PushButton::new(
                    format!("emaki.notification.{id}.action.{key}"),
                    action.label(),
                )
                .width(each)
                .text(sys::r#type::label())
                .color(color)
                .on_press(move || Notifications::invoke(id, &key))
                .build(),
            )
        })
        .collect();

    for (label, command) in phone {
        buttons.push(boxed(
            PushButton::new(
                format!("emaki.notification.{id}.action.phone.{label}"),
                label,
            )
            .width(each)
            .text(sys::r#type::label())
            .color(color)
            .on_press(move || {
                amane::Launch::argv(&command).activated();
                Notifications::dismiss(id);
            })
            .build(),
        ));
    }

    Row::new(buttons).gap(gap)
}

/// Makes card `id` clickable: a left click does `press`, and the pointer
/// coming onto it or leaving shows its close button and calls `hover`.
pub fn interactive(
    card: Rectangle,
    id: u32,
    hover_also: impl Fn(bool) + 'static,
    press: impl Fn() + 'static,
) -> Rectangle {
    card.cursor(Pointer)
        .on_hover(move |inside| {
            hover(id, inside);
            hover_also(inside);
        })
        .on_click(move |button| {
            if button == Button::Left {
                press();
            }
        })
}

/// A small tonal close button `side` square, as a card and a group use.
pub fn close_button(id: String, side: f32, color: Color, press: impl Fn() + 'static) -> Rectangle {
    IconButton::new(id, ICON_CLOSE)
        .style(Style::Tonal)
        .size(sys::size::icon::s())
        .color(color)
        .width(side)
        .height(side)
        .on_press(press)
        .build()
}
