//! The status bar: which edge it runs along, what it's made of, its clock,
//! its workspaces and which icons it shows. Each control changes
//! `status-bar.json`, which the shell follows at once.

use amane::{Column, Widget};
use amane_kit::buttons::{Radio, Segment, SegmentedButton};
use amane_kit::dropdown::Dropdown;
use amane_kit::form::FormRow;
use amane_kit::layout::boxed;
use amane_kit::settings::{BarPosition, BarStyle, DateFormat, change_status_bar, status_bar};

use crate::{group, groups, radios_height, switch_row};

/// The icons it can leave out, by the names the shell knows them by.
const ICONS: [(&str, &str); 9] = [
    ("volume", "Volume"),
    ("bluetooth", "Bluetooth"),
    ("network", "Network"),
    ("battery", "Battery"),
    ("tailscale", "Tailscale, while connected"),
    ("keep-awake", "Keep Awake, while on"),
    ("status-icons", "Night Light, VPN, power mode, Airplane"),
    ("notifications", "Notifications and Do Not Disturb"),
    ("privacy", "Camera, microphone, screen sharing"),
];

const POSITIONS: [(&str, BarPosition); 2] =
    [("Bottom", BarPosition::Bottom), ("Top", BarPosition::Top)];

const STYLES: [(&str, BarStyle); 3] = [
    ("Frosted", BarStyle::Frosted),
    ("Glass", BarStyle::Glass),
    (
        "Solid (black in dark mode, white in light)",
        BarStyle::Solid,
    ),
];

pub fn page() -> Column {
    let now = status_bar().clone();

    let position = SegmentedButton::new(
        "settings.bar.position",
        POSITIONS
            .into_iter()
            .map(|(label, _)| Segment::new(label))
            .collect(),
        POSITIONS
            .iter()
            .position(|(_, position)| *position == now.position),
    )
    .on_select(|index, _| change_status_bar(|now| now.position = POSITIONS[index].1))
    .build();

    let styles: Vec<Box<dyn Widget>> = STYLES
        .into_iter()
        .enumerate()
        .map(|(index, (label, style))| {
            boxed(
                Radio::new(format!("settings.bar.style.{index}"), now.style == style)
                    .label(label)
                    .on_select(move || change_status_bar(|now| now.style = style))
                    .build(),
            )
        })
        .collect();

    let dates = [
        "Sunday 25 October",
        "Sun 25 Oct",
        "Numbers, as the locale has them",
    ];
    let date_format = Dropdown::new(
        "settings.bar.date",
        dates.map(String::from).to_vec(),
        DateFormat::ALL
            .iter()
            .position(|format| *format == now.date_format)
            .unwrap_or(0),
    )
    .width(240.0)
    .on_select(|index| change_status_bar(|now| now.date_format = DateFormat::ALL[index]));

    let icons = ICONS.into_iter().map(|(name, label)| {
        switch_row(
            &format!("settings.bar.icon.{name}"),
            label,
            now.shows(name),
            move |on| change_status_bar(|now| now.set_shown(name, on)),
        )
    });

    groups(vec![
        group()
            .row(FormRow::new("settings.bar.position", "Position").trailing(position))
            .row(
                switch_row(
                    "settings.bar.auto-hide",
                    "Hide automatically",
                    now.auto_hide,
                    |on| change_status_bar(|now| now.auto_hide = on),
                )
                .subtitle("Until the pointer meets the screen's edge"),
            )
            .footer(
                "Hidden, it keeps no room from windows, and comes back with Quick Settings or the \
                 notification center too. At the bottom, a hiding dock takes the edge first.",
            ),
        group().row(
            FormRow::new("settings.bar.material", "Material")
                .detail(Column::new(styles), radios_height(STYLES.len())),
        ),
        group()
            .header("Clock")
            .row(switch_row(
                "settings.bar.clock-24h",
                "24-hour time",
                now.clock_24h,
                |on| change_status_bar(|now| now.clock_24h = on),
            ))
            .row(switch_row(
                "settings.bar.clock-seconds",
                "Show seconds",
                now.clock_seconds,
                |on| change_status_bar(|now| now.clock_seconds = on),
            ))
            .row(
                FormRow::new("settings.bar.date", "Date in the tooltip")
                    .trailing(date_format.build()),
            ),
        group()
            .header("Workspaces")
            .row(switch_row(
                "settings.bar.workspaces",
                "Show workspaces",
                now.show_workspaces,
                |on| change_status_bar(|now| now.show_workspaces = on),
            ))
            .row(
                switch_row(
                    "settings.bar.scroll-workspaces",
                    "Scroll to switch",
                    now.scroll_workspaces,
                    |on| change_status_bar(|now| now.scroll_workspaces = on),
                )
                .subtitle("Scroll over the workspaces to move between them"),
            ),
        group().header("Show in the status bar").rows(icons).footer(
            "Quick Settings and the clock always show. Each Quick Settings page can show or \
                 hide its icon too.",
        ),
    ])
}
