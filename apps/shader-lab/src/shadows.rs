//! The shadows section: + Drop and + Inner, then a card per shadow with its
//! colour, offset, blur, spread and opacity. Shapes only, as amane's
//! shadows follow a rounded rectangle.

use amane::{Column, Row, Service, Widget};
use amane_kit::buttons::PushButton;
use amane_kit::color_picker::{ColorPicker, to_hex};
use amane_kit::lab::config::{Config, ShadowSpec};
use amane_kit::layout::boxed;
use amane_kit::tokens::sys;

use crate::panel::{self, Number};
use crate::{Lab, change};

/// The section and how tall it is.
pub fn section(lab: &Lab) -> (Column, f32) {
    let control = sys::size::control::height();
    let gap = sys::space::gap::xs();

    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(panel::caption("Shadows"))];
    let mut height = panel::caption_height();

    if !lab.state.subject.kind.is_shape() {
        rows.push(boxed(panel::note(
            "Shadows follow a rounded rectangle, so they show on the rectangle, pill and circle.",
        )));

        return (Column::new(rows).gap(gap), height + control + gap);
    }

    let add = |label: &'static str, inner: bool| {
        PushButton::new(format!("lab.shadow.add.{inner}"), label)
            .on_press(move || {
                change("shadow.add", |state| {
                    let shadow = if inner {
                        ShadowSpec {
                            inner: true,
                            color: "#00000040".into(),
                            y: 2.0,
                            blur: 8.0,
                            ..ShadowSpec::default()
                        }
                    } else {
                        ShadowSpec::default()
                    };

                    state.shadows.push(shadow);
                });
            })
            .build()
    };

    rows.push(boxed(
        Row::new(vec![
            boxed(add("+ Drop shadow", false)),
            boxed(add("+ Inner shadow", true)),
        ])
        .gap(sys::space::gap::s()),
    ));
    height += control + gap;

    for (index, shadow) in lab.state.shadows.iter().enumerate() {
        let (card, card_height) = card(lab, index, shadow);

        rows.push(boxed(card));
        height += card_height + gap;
    }

    (Column::new(rows).gap(gap), height)
}

fn card(lab: &Lab, index: usize, shadow: &ShadowSpec) -> (Column, f32) {
    let control = sys::size::control::height();
    let gap = sys::space::gap::xs();
    let count = lab.state.shadows.len();

    let kind = if shadow.inner { "Inner" } else { "Drop" };
    let id = format!("shadow.{index}");

    let header = Row::new(vec![
        boxed(panel::label(&format!("{kind} {}", index + 1))),
        boxed(panel::swatch(
            &id,
            shadow.rgba(),
            lab.picking.as_deref() == Some(id.as_str()),
        )),
        boxed(small("↑", &format!("{id}.up"), index > 0, move || {
            swap(index, index - 1)
        })),
        boxed(small(
            "↓",
            &format!("{id}.down"),
            index + 1 < count,
            move || swap(index, index + 1),
        )),
        boxed(small("✕", &format!("{id}.remove"), true, move || {
            change("shadow.remove", |state| {
                if index < state.shadows.len() {
                    state.shadows.remove(index);
                }
            });

            Lab::write().picking = None;
        })),
    ])
    .gap(sys::space::gap::xs());

    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(header)];
    let mut height = control;

    if lab.picking.as_deref() == Some(id.as_str()) {
        let picker = ColorPicker::new(format!("lab.picker.{id}"), shadow.rgba())
            .width(panel::WIDTH)
            .on_change(move |color| {
                set(index, "color", |shadow| shadow.color = to_hex(color));
            })
            .build();

        rows.push(boxed(picker));
        height += panel::picker_height(panel::WIDTH) + gap;
    }

    let numbers: [(&str, &str, f32, f32, f32, f32, &str); 5] = [
        ("x", "X", shadow.x, -80.0, 80.0, 1.0, "px"),
        ("y", "Y", shadow.y, -80.0, 80.0, 1.0, "px"),
        ("blur", "Blur", shadow.blur, 0.0, 120.0, 1.0, "px"),
        ("spread", "Spread", shadow.spread, -40.0, 40.0, 1.0, "px"),
        (
            "opacity",
            "Opacity",
            shadow.opacity * 100.0,
            0.0,
            100.0,
            1.0,
            "%",
        ),
    ];

    for (field, label, value, min, max, step, unit) in numbers {
        let number = Number {
            id: format!("lab.{id}.{field}"),
            label: label.into(),
            value,
            min,
            max,
            step,
            unit: unit.into(),
        };

        rows.push(boxed(panel::number_row(number, move |value| {
            set(index, field, |shadow| match field {
                "x" => shadow.x = value,
                "y" => shadow.y = value,
                "blur" => shadow.blur = value,
                "spread" => shadow.spread = value,
                _ => shadow.opacity = value / 100.0,
            });
        })));

        height += control + gap;
    }

    (Column::new(rows).gap(gap), height)
}

/// Changes one shadow, as one undo step per shadow and field.
fn set(index: usize, field: &str, edit: impl FnOnce(&mut ShadowSpec)) {
    change(&format!("shadow.{index}.{field}"), |state: &mut Config| {
        if let Some(shadow) = state.shadows.get_mut(index) {
            edit(shadow);
        }
    });
}

fn swap(a: usize, b: usize) {
    change("shadow.order", |state| {
        if a < state.shadows.len() && b < state.shadows.len() {
            state.shadows.swap(a, b);
        }
    });

    Lab::write().picking = None;
}

fn small(glyph: &str, id: &str, enabled: bool, press: impl Fn() + 'static) -> amane::Rectangle {
    let button = PushButton::new(format!("lab.{id}"), glyph).width(30.0);

    if enabled {
        button.on_press(press).build()
    } else {
        button.build().opacity(0.35)
    }
}
