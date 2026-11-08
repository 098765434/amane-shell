//! Calendar — a month at a glance: the month's name with buttons to the
//! months either side (and back to this one), a row of weekday letters,
//! then six weeks of days. Today is a filled accent circle; days of the
//! months either side are fainter. It's quads and text only.
//!
//! The month shown is the caller's, so it can keep it across redraws and
//! reset it as it likes; [`Calendar::on_month`] hears the buttons.
//! [`month_grid`] is the layout alone, for a view of another shape.

use std::rc::Rc;

use amane::{Center, Color, Column, Radius, Rectangle, Row, Text};
use chrono::{Datelike, Months, NaiveDate, Weekday};

use crate::buttons::{IconButton, PushButton, Shape, Style};
use crate::icons::{ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT};
use crate::layout::{boxed, split_row};
use crate::style::{self, px};
use crate::tokens::{Styled, sys};

/// One day of the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Day {
    /// The date.
    pub date: NaiveDate,
    /// Whether it's in the month shown, not one either side.
    pub in_month: bool,
    /// Whether it's today.
    pub today: bool,
}

/// Six weeks from the week holding the 1st of `month`'s month, each week
/// starting on `first`, with `today` marked.
pub fn month_grid(month: NaiveDate, today: NaiveDate, first: Weekday) -> [[Day; 7]; 6] {
    let start_of_month = first_of(month);
    let lead =
        (7 + start_of_month.weekday().num_days_from_monday() - first.num_days_from_monday()) % 7;
    let start = start_of_month - chrono::Days::new(u64::from(lead));
    std::array::from_fn(|week| {
        std::array::from_fn(|weekday| {
            let date = start + chrono::Days::new((week * 7 + weekday) as u64);
            Day {
                date,
                in_month: date.month() == month.month() && date.year() == month.year(),
                today: date == today,
            }
        })
    })
}

/// The 1st of `date`'s month.
pub fn first_of(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

/// The 1st of the month `by` months from `month`'s, back for a negative
/// `by`.
pub fn shift_month(month: NaiveDate, by: i32) -> NaiveDate {
    let month = first_of(month);
    let shifted = if by >= 0 {
        month.checked_add_months(Months::new(by.unsigned_abs()))
    } else {
        month.checked_sub_months(Months::new(by.unsigned_abs()))
    };
    shifted.unwrap_or(month)
}

/// The weekday letters, starting on `first`.
fn weekday_letters(first: Weekday) -> [&'static str; 7] {
    const LETTERS: [&str; 7] = ["M", "T", "W", "T", "F", "S", "S"];
    let from = first.num_days_from_monday() as usize;
    std::array::from_fn(|i| LETTERS[(from + i) % 7])
}

/// A month view.
pub struct Calendar {
    id: String,
    month: NaiveDate,
    today: NaiveDate,
    first: Weekday,
    width: f32,
    color: Color,
    faint: Color,
    on_month: Rc<dyn Fn(NaiveDate)>,
}

impl Calendar {
    /// `month`'s month (any day of it), with `today` marked; weeks start on
    /// Monday, in the label colours, 280 wide.
    pub fn new(id: impl Into<String>, month: NaiveDate, today: NaiveDate) -> Self {
        Self {
            id: id.into(),
            month: first_of(month),
            today,
            first: Weekday::Mon,
            width: px(280.0),
            color: style::label_primary(),
            faint: style::label_tertiary(),
            on_month: Rc::new(|_| {}),
        }
    }

    /// The day weeks start on.
    pub fn first_weekday(mut self, first: Weekday) -> Self {
        self.first = first;
        self
    }

    /// How wide it is; the days share it equally.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The text's colour, and the fainter one for the weekday letters and
    /// the days of the months either side, as on glass.
    pub fn colors(mut self, color: Color, faint: Color) -> Self {
        self.color = color;
        self.faint = faint;
        self
    }

    /// What the buttons do: given the 1st of the month to show.
    pub fn on_month(mut self, pick: impl Fn(NaiveDate) + 'static) -> Self {
        self.on_month = Rc::new(pick);
        self
    }

    /// How tall [`build`](Self::build) draws it: the header, the weekday
    /// letters and six weeks.
    pub fn height(&self) -> f32 {
        let (header, letters, row, gap) = self.rows();
        header + letters + row * 6.0 + gap * 7.0
    }

    // the header's, the letters', a week's height, and the gap between rows
    fn rows(&self) -> (f32, f32, f32, f32) {
        let row = (self.width / 7.0).min(px(32.0));
        (sys::size::control::height(), px(20.0), row, px(2.0))
    }

    /// The month.
    pub fn build(self) -> Column {
        let cell = self.width / 7.0;
        let (_, letters_height, row_height, gap) = self.rows();
        let mut rows: Vec<Box<dyn amane::Widget>> = vec![boxed(self.header())];

        let letters = weekday_letters(self.first).map(|letter| {
            boxed(
                Rectangle::new()
                    .width(cell)
                    .height(letters_height)
                    .align_child(Center, Center)
                    .child(
                        Text::new(letter)
                            .style(sys::r#type::caption())
                            .color(self.faint),
                    ),
            )
        });
        rows.push(boxed(Row::new(letters.into())));

        for week in month_grid(self.month, self.today, self.first) {
            let days = week.map(|day| boxed(self.day(day, cell, row_height)));
            rows.push(boxed(Row::new(days.into())));
        }
        Column::new(rows).gap(gap).width(self.width)
    }

    fn header(&self) -> Row {
        let title = Text::new(self.month.format("%B %Y").to_string())
            .style(sys::r#type::label())
            .color(self.color);
        let month = self.month;
        let this_month = first_of(self.today);

        let mut trailing: Vec<Box<dyn amane::Widget>> = Vec::new();
        if month != this_month {
            let pick = self.on_month.clone();
            trailing.push(boxed(
                PushButton::new(format!("{}.today", self.id), "Today")
                    .shape(Shape::Round)
                    .color(self.color)
                    .on_press(move || pick(this_month))
                    .build(),
            ));
        }
        for (key, glyph, by) in [
            ("back", ICON_CHEVRON_LEFT, -1),
            ("forward", ICON_CHEVRON_RIGHT, 1),
        ] {
            let pick = self.on_month.clone();
            trailing.push(boxed(
                IconButton::new(format!("{}.{key}", self.id), glyph)
                    .style(Style::Plain)
                    .size(sys::size::icon::s())
                    .color(self.color)
                    .on_press(move || pick(shift_month(month, by)))
                    .build(),
            ));
        }

        // the column is `width` wide, so the parent's width is the calendar's
        split_row(title, trailing)
    }

    fn day(&self, day: Day, width: f32, height: f32) -> Rectangle {
        let side = (height - px(2.0)).max(1.0);
        let (fill, color) = if day.today {
            (sys::color::accent::bg(), sys::color::accent::fg())
        } else if day.in_month {
            (Color::TRANSPARENT, self.color)
        } else {
            (Color::TRANSPARENT, self.faint)
        };
        let number = Rectangle::new()
            .width(side)
            .height(side)
            .radius(Radius::Full)
            .fill(fill)
            .align_child(Center, Center)
            .child(
                Text::new(day.date.day().to_string())
                    .style(sys::r#type::caption())
                    .color(color),
            );
        Rectangle::new()
            .width(width)
            .height(height)
            .align_child(Center, Center)
            .child(number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn a_month_starts_in_the_week_of_its_first() {
        // 1 November 2026 is a Sunday
        let grid = month_grid(date(2026, 11, 8), date(2026, 11, 8), Weekday::Mon);
        assert_eq!(grid[0][0].date, date(2026, 10, 26));
        assert!(!grid[0][0].in_month);
        assert_eq!(grid[0][6].date, date(2026, 11, 1));
        assert!(grid[0][6].in_month);
        assert_eq!(grid[5][6].date, date(2026, 12, 6));

        let today: Vec<Day> = grid.iter().flatten().filter(|d| d.today).copied().collect();
        assert_eq!(today.len(), 1);
        assert_eq!(today[0].date, date(2026, 11, 8));
    }

    #[test]
    fn weeks_can_start_on_sunday() {
        let grid = month_grid(date(2026, 11, 1), date(2026, 1, 1), Weekday::Sun);
        assert_eq!(grid[0][0].date, date(2026, 11, 1));
        assert!(grid.iter().flatten().all(|d| !d.today));
        assert_eq!(
            weekday_letters(Weekday::Sun),
            ["S", "M", "T", "W", "T", "F", "S"]
        );
    }

    #[test]
    fn months_shift_across_years_from_any_day() {
        assert_eq!(shift_month(date(2026, 12, 31), 1), date(2027, 1, 1));
        assert_eq!(shift_month(date(2026, 1, 15), -1), date(2025, 12, 1));
        assert_eq!(shift_month(date(2026, 3, 31), 0), date(2026, 3, 1));
    }
}
