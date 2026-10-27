//! The dock: how big its icons are and how much they grow, whether it
//! hides, and what it shows and animates. Each control changes
//! `dock.json`, which the shell follows at once.

use amane::{Center, Column, Row};
use amane_kit::buttons::{Segment, SegmentedButton, Switch};
use amane_kit::layout::boxed;
use amane_kit::settings::{DOCK_ICON_SIZES, Magnify, change_dock, dock};
use amane_kit::slider::Slider;
use amane_kit::tokens::sys;

use crate::{labelled, note, setting};

const MAGNIFY: [&str; 4] = ["Off", "Small", "Medium", "Large"];

pub fn page() -> Column {
    let now = dock().clone();

    let (least, most) = DOCK_ICON_SIZES;
    let span = f32::from(most - least);
    let at = ((f32::from(now.icon_size.clamp(least, most) - least) / span) * 100.0).round() as u8;
    let size = Row::new(vec![
        boxed(
            Slider::new(at, 240.0)
                .on_value_change(move |value| {
                    let size = least + (f32::from(value) / 100.0 * span).round() as u8;
                    change_dock(|now| now.icon_size = size);
                })
                .build(),
        ),
        boxed(note(format!("{} pt", now.icon_size.clamp(least, most)))),
    ])
    .gap(sys::space::gap::m())
    .align(Center);

    let magnify = SegmentedButton::new(
        "settings.dock.magnify",
        MAGNIFY.into_iter().map(Segment::new).collect(),
        Magnify::ALL
            .iter()
            .position(|amount| *amount == now.magnification),
    )
    .on_select(|index, _| change_dock(|now| now.magnification = Magnify::ALL[index]))
    .build();

    let switch =
        |id: &'static str, label: &str, on: bool, set: fn(&mut amane_kit::settings::Dock, bool)| {
            boxed(labelled(
                label,
                Switch::new(id, on)
                    .on_toggle(move |on| change_dock(|now| set(now, on)))
                    .build(),
            ))
        };

    let behaviour = Column::new(vec![
        switch(
            "settings.dock.auto-hide",
            "Hide the dock until the pointer meets the screen's edge",
            now.auto_hide,
            |now, on| now.auto_hide = on,
        ),
        switch(
            "settings.dock.overview",
            "Show the dock with the overview",
            now.rise_with_overview,
            |now, on| now.rise_with_overview = on,
        ),
        boxed(note(
            "A dock that stays out keeps windows clear of it, above the status bar.",
        )),
    ])
    .gap(sys::space::gap::s());

    let shows = Column::new(vec![
        switch(
            "settings.dock.dots",
            "Show dots under open apps",
            now.running_dots,
            |now, on| now.running_dots = on,
        ),
        switch(
            "settings.dock.names",
            "Show an app's name over its icon",
            now.tooltips,
            |now, on| now.tooltips = on,
        ),
        switch(
            "settings.dock.animations",
            "Animate the dock",
            now.animations,
            |now, on| now.animations = on,
        ),
    ])
    .gap(sys::space::gap::s());

    Column::new(vec![
        boxed(setting("Icon size", size)),
        boxed(setting("Magnification", magnify)),
        boxed(setting("Hiding", behaviour)),
        boxed(setting("Icons", shows)),
    ])
    .gap(sys::space::gap::l())
}
