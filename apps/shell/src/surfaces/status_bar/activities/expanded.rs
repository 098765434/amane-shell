//! An activity opened: the popup under its chip, with its buttons.

use super::*;
use amane_kit::layout::frame;

/// Opens the expanded view of `key` under the pointer, or closes it.
pub(super) fn toggle(key: &str) {
    let showing = OPEN.with_borrow(|open| open.as_ref().is_some_and(|open| open.key == key));
    if showing {
        close();
        return;
    }
    OPEN.set(Some(Open {
        key: key.to_string(),
        x: cursor().0,
        clicked: true,
    }));
    Tick::changed();
}

/// Closes the expanded view.
pub(super) fn close() {
    if OPEN.take().is_some() {
        registry::quiet_alert();
        Tick::changed();
    }
}

/// An alerting update brings its view up, unless another is open or Do
/// Not Disturb is on; it closes again once the alert is over.
pub(super) fn alert(shown: &[Shown]) {
    let alerting = registry::alerting();
    let key = alerting.map(|handle| format!("bus.{handle}"));
    let open_alert = OPEN.with_borrow(|open| {
        open.as_ref()
            .filter(|open| !open.clicked)
            .map(|open| open.key.clone())
    });
    match (key, open_alert) {
        // the alert is over, or another took its place
        (key, Some(open)) if key.as_ref() != Some(&open) => {
            OPEN.set(None);
            Tick::changed();
        }
        (Some(key), None) => {
            let handle = alerting.unwrap_or_default();
            let fresh = ALERTED.replace(Some(handle)) != Some(handle);
            let nothing_open = OPEN.with_borrow(Option::is_none);
            let there = shown.iter().any(|activity| activity.key == key);
            if fresh && nothing_open && there && !DoNotDisturb::read().on() {
                OPEN.set(Some(Open {
                    key,
                    // the activities sit towards the bar's right
                    x: (window_size().0 - CARD_WIDTH) as i32,
                    clicked: false,
                }));
                Tick::changed();
            }
        }
        _ => {}
    }
    if alerting.is_none() {
        ALERTED.set(None);
    }
}

/// The expanded view's popup, registered once with `App::popup`.
pub fn popup() -> Popup {
    Tick::watch();
    let open = OPEN.with_borrow(Clone::clone);
    let found = open.as_ref().and_then(|open| {
        gather()
            .into_iter()
            .find(|activity| activity.key == open.key)
    });
    let (Some(open), Some(activity)) = (open, found) else {
        if OPEN.with_borrow(Option::is_some) {
            // what it showed ended
            OPEN.set(None);
        }
        return Popup::new(POPUP_NS)
            .size(1, 1)
            .anchor_rect(0, 0, 1, 1)
            .parent(BAR_NS)
            .visible(false)
            .child(amane_kit::layout::blank());
    };

    let (card, height) = expanded(&activity);
    let mut popup = Popup::new(POPUP_NS)
        .size(CARD_WIDTH as u32, height as u32)
        .anchor_rect(open.x, menu_edge(), 1, 1)
        .vibrant(material::vibrant())
        .parent(BAR_NS)
        .gravity(Gravity::BottomRight)
        .constraint_adjustment(
            ConstraintAdjustment::SLIDE_X
                | ConstraintAdjustment::SLIDE_Y
                | ConstraintAdjustment::FLIP_X
                | ConstraintAdjustment::FLIP_Y,
        )
        .visible(true)
        .on_dismiss(close)
        .child(card);
    if open.clicked {
        popup = popup.grab();
    }
    popup
}

/// The expanded card and its height: the picture, title and subtitle and
/// the trailing part; the progress bar; the buttons.
pub(super) fn expanded(activity: &Shown) -> (Rectangle, f32) {
    let content = &activity.content;
    let padding = sys::space::inset::m();
    let gap = sys::space::gap::s();
    let inner = CARD_WIDTH - padding * 2.0;
    let tint = content.tint.map(tint_color);
    let primary = material::label(Level::Primary);
    let secondary = material::label(Level::Secondary);

    let mut text = vec![boxed(
        Text::new(clip(&content.title, sys::r#type::label(), inner - 120.0))
            .style(sys::r#type::label())
            .color(primary),
    )];
    if !content.subtitle.is_empty() {
        text.push(boxed(
            Text::new(clip(
                &content.subtitle,
                sys::r#type::caption(),
                inner - 120.0,
            ))
            .style(sys::r#type::caption())
            .color(secondary),
        ));
    }
    let mut header = vec![
        picture(
            &content.leading,
            LEADING_BIG,
            tint.unwrap_or(primary),
            activity.dim,
        ),
        boxed(
            amane::Column::new(text)
                .gap(2.0)
                .width(inner - LEADING_BIG - 12.0 - 72.0),
        ),
    ];
    let big = match &content.trailing {
        Trailing::Progress(done) => {
            Some(boxed(progress_ring(*done, 28.0, tint.unwrap_or(primary))))
        }
        Trailing::Text(_) if activity.key == "now-playing" => None,
        trailing => trailing_text(trailing, false).map(|(text, style)| {
            boxed(Text::new(text).style(style).color(tint.unwrap_or(primary)))
        }),
    };
    header.extend(big);
    let header = frame(inner, HEADER)
        .align_child(Start, Center)
        .child(Row::new(header).gap(12.0).align(Center));

    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(header)];
    let mut height = padding * 2.0 + HEADER;

    if let Some(done) = content.progress {
        let track = Rectangle::new()
            .width(inner)
            .height(BAR)
            .radius(BAR / 2.0)
            .fill(material::fill_role(FillLevel::Secondary))
            .align_child(Start, Center)
            .child(
                Rectangle::new()
                    .width((inner * done).max(BAR))
                    .height(BAR)
                    .radius(BAR / 2.0)
                    .fill(tint.unwrap_or_else(sys::color::accent::bg)),
            );
        rows.push(boxed(track));
        height += gap + BAR;
    }

    let buttons: Vec<&ActivityButton> = content.buttons.iter().take(3).collect();
    if !buttons.is_empty() {
        let row_gap = sys::space::gap::xs();
        let each = (inner - row_gap * (buttons.len() as f32 - 1.0)) / buttons.len() as f32;
        let row: Vec<Box<dyn Widget>> = buttons
            .into_iter()
            .map(|button| boxed(activity_button(activity, button, each)))
            .collect();
        rows.push(boxed(
            Row::new(row).gap(row_gap).height(BUTTONS).align(Center),
        ));
        height += gap + BUTTONS;
    }

    let card = Card::new(rows)
        .material(Material::Menu)
        .variant(Variant::Outlined)
        .glass(true)
        .gap(gap)
        .padding(padding)
        .build();
    (card, height.ceil())
}

/// A button of the expanded view: a symbol alone, or a label.
pub(super) fn activity_button(activity: &Shown, button: &ActivityButton, width: f32) -> Rectangle {
    let act = activity.act.clone();
    let id = button.id.clone();
    let key = format!("amane.bar.activity.{}.{}", activity.key, button.id);
    let style = if button.destructive {
        Style::Destructive
    } else {
        Style::Tonal
    };
    let glyph = button.icon.as_ref().and_then(|icon| match icon {
        ItemIcon::Glyph(name) => Some(match name.as_str() {
            "skip-previous" => ICON_SKIP_PREVIOUS,
            "skip-next" => ICON_SKIP_NEXT,
            "play" => ICON_PLAY,
            "pause" => ICON_PAUSE,
            name => named(name).unwrap_or(ICON_RECORD),
        }),
        _ => None,
    });
    let press = move || act(&id);
    match (glyph, button.label.is_empty()) {
        (Some(glyph), true) => IconButton::new(key, glyph)
            .style(style)
            .width(width)
            .height(BUTTONS)
            .on_press(press)
            .build(),
        (glyph, _) => {
            let mut push = PushButton::new(key, button.label.clone())
                .style(style)
                .width(width)
                .height(BUTTONS)
                .on_press(press);
            if let Some(glyph) = glyph {
                push = push.icon(glyph);
            }
            push.build()
        }
    }
}
