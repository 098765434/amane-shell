//! The month card at the top of the center: today marked, the months
//! either side a click away. The month shown goes back to this one when
//! the center closes.

use std::cell::Cell;

use amane::{Color, Rectangle, Service};
use chrono::NaiveDate;

use super::card::Place;
use crate::services::clock::Clock;
use crate::tokens::{notification, sys};
use amane_kit::calendar::{Calendar, first_of};
use amane_kit::material::{self, FillLevel, Level};

thread_local! {
    // the month shown, when it isn't this one
    static SHOWN: Cell<Option<NaiveDate>> = const { Cell::new(None) };
}

/// Back to this month, for the next time the center opens.
pub(super) fn reset() {
    SHOWN.with(|shown| shown.set(None));
}

/// The card, as wide as the notifications under it.
pub(super) fn card() -> Rectangle {
    // read for its minute tick, so today moves on at midnight
    drop(Clock::read());
    let today = chrono::Local::now().date_naive();
    let month = SHOWN.with(Cell::get).unwrap_or_else(|| first_of(today));

    let width = Place::Center.width();
    let inset = sys::space::inset::m();
    let calendar = Calendar::new("amane.notifications.calendar", month, today)
        .width(width - inset * 2.0)
        .colors(
            material::label(Level::Primary),
            material::label(Level::Secondary),
        )
        .on_month(move |month| {
            let shown = (month != first_of(today)).then_some(month);
            SHOWN.with(|now| now.set(shown));
        });
    let height = calendar.height() + inset * 2.0;

    // a raised card on the sheet, like the notifications'
    Rectangle::new()
        .width(width)
        .height(height)
        .radius(notification::card::radius())
        .fill(material::fill_role(FillLevel::Secondary))
        .border(sys::border::hairline(), Color::TRANSPARENT)
        .padding(inset)
        .child(calendar.build())
}
