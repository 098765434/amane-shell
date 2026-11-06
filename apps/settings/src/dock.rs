//! The dock: how big its icons are and how much they grow, whether it
//! hides, and what it shows and animates. Each control changes
//! `dock.json`, which the shell follows at once.

use amane::Column;
use amane_kit::dropdown::Dropdown;
use amane_kit::form::{FormRow, SliderRow};
use amane_kit::settings::{DOCK_ICON_SIZES, Dock, Magnify, change_dock, dock};
use amane_kit::slider::Slider;

use crate::{group, groups, switch_row};

const MAGNIFY: [&str; 4] = ["Off", "Small", "Medium", "Large"];

pub fn page() -> Column {
    let now = dock().clone();

    let (least, most) = DOCK_ICON_SIZES;
    let span = f32::from(most - least);
    let at = ((f32::from(now.icon_size.clamp(least, most) - least) / span) * 100.0).round() as u8;
    let magnify = Dropdown::new(
        "settings.dock.magnify",
        MAGNIFY.map(String::from).to_vec(),
        Magnify::ALL
            .iter()
            .position(|amount| *amount == now.magnification)
            .unwrap_or(0),
    )
    .width(200.0)
    .on_select(|index| change_dock(|now| now.magnification = Magnify::ALL[index]))
    .build();

    let switch = |id: &str, title: &str, on: bool, set: fn(&mut Dock, bool)| {
        switch_row(id, title, on, move |on| change_dock(|now| set(now, on)))
    };

    groups(vec![
        group()
            .header("Size")
            .slider(
                SliderRow::new("Icon size", move |width| {
                    Slider::new(at, width)
                        .on_value_change(move |value| {
                            let size = least + (f32::from(value) / 100.0 * span).round() as u8;
                            change_dock(|now| now.icon_size = size);
                        })
                        .build()
                })
                .value(format!("{} pt", now.icon_size.clamp(least, most))),
            )
            .row(FormRow::new("settings.dock.magnify", "Magnification").trailing(magnify)),
        group()
            .header("Hiding")
            .row(
                switch(
                    "settings.dock.auto-hide",
                    "Hide automatically",
                    now.auto_hide,
                    |now, on| now.auto_hide = on,
                )
                .subtitle("Until the pointer meets the screen's edge"),
            )
            .row(
                switch(
                    "settings.dock.overview",
                    "Show the dock with the overview",
                    now.rise_with_overview,
                    |now, on| now.rise_with_overview = on,
                )
                .child()
                .disabled(!now.auto_hide),
            )
            .footer("A dock that stays out keeps windows clear of it, above the status bar."),
        group()
            .header("Icons")
            .row(switch(
                "settings.dock.dots",
                "Show dots under open apps",
                now.running_dots,
                |now, on| now.running_dots = on,
            ))
            .row(switch(
                "settings.dock.names",
                "Show an app's name over its icon",
                now.tooltips,
                |now, on| now.tooltips = on,
            ))
            .row(switch(
                "settings.dock.animations",
                "Animate the dock",
                now.animations,
                |now, on| now.animations = on,
            )),
    ])
}
