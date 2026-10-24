//! The system row — GNOME's top row: the battery (opening its page), then
//! taking a screenshot, locking the screen and the power menu.

use std::time::Duration;

use amane::{Battery, Row, Service, Windows};

use super::grid::Cell;
use super::tiles::GridCell;
use super::{Page, open};
use crate::components::buttons::{IconButton, PushButton, Shape, Style};
use crate::components::icons::{ICON_LOCK, ICON_POWER, ICON_SCREENSHOT, battery_glyph};
use crate::components::layout::{boxed, nothing, split_row};
use crate::components::taskbar::{battery_color, close};
use crate::services::session::Session;
use crate::tokens::sys;

/// How long the card takes to fold away before the screenshot UI freezes
/// the screen, so the card isn't in it.
const SCREENSHOT_AFTER: Duration = Duration::from_millis(350);

pub(super) fn cell() -> GridCell {
    let battery = Battery::read();
    let present = battery.present();
    let percent = battery.percent();
    let on_ac = battery.on_ac();
    drop(battery);

    let leading = if present {
        boxed(
            PushButton::new("amane.qs.battery", format!("{percent}%"))
                .shape(Shape::Round)
                .icon(battery_glyph(percent, on_ac))
                .icon_size(sys::size::icon::m())
                .text(sys::r#type::numeric())
                .color(battery_color(
                    percent,
                    on_ac,
                    sys::color::surface::panel::fg(),
                ))
                .on_press(|| open(Page::Power))
                .build(),
        )
    } else {
        nothing()
    };

    let screenshot = button("amane.qs.screenshot", ICON_SCREENSHOT, || {
        close();
        std::thread::spawn(|| {
            std::thread::sleep(SCREENSHOT_AFTER);
            Windows::screenshot();
        });
    });
    let lock = button("amane.qs.lock", ICON_LOCK, || {
        close();
        Session::lock();
    });
    let power = button("amane.qs.power", ICON_POWER, || open(Page::Session));

    let row = split_row(
        Row::new(vec![leading]),
        vec![boxed(screenshot), boxed(lock), boxed(power)],
    );
    Cell::Full(boxed(row), sys::size::control::height())
}

fn button(id: &'static str, glyph: char, press: impl Fn() + 'static) -> amane::Rectangle {
    IconButton::new(id, glyph)
        .style(Style::Tonal)
        .on_press(press)
        .build()
}
