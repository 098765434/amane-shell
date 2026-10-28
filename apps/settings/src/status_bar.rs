//! The status bar: which edge it runs along, what it's made of, its clock,
//! its workspaces and which icons it shows. Each control changes
//! `status-bar.json`, which the shell follows at once.

use amane::{Column, Widget};
use amane_kit::buttons::{Radio, Segment, SegmentedButton, Switch};
use amane_kit::dropdown::Dropdown;
use amane_kit::layout::boxed;
use amane_kit::settings::{BarPosition, BarStyle, DateFormat, change_status_bar, status_bar};
use amane_kit::tokens::sys;

use crate::{labelled, note, setting};

/// The icons it can leave out, by the names the shell knows them by.
const ICONS: [(&str, &str); 9] = [
    ("volume", "Volume"),
    ("bluetooth", "Bluetooth"),
    ("network", "Network"),
    ("battery", "Battery"),
    ("tailscale", "Tailscale, while connected"),
    ("keep-awake", "Keep Awake, while on"),
    (
        "status-icons",
        "Night Light, VPN, power mode and Airplane Mode, while on",
    ),
    ("notifications", "New notifications and Do Not Disturb"),
    (
        "privacy",
        "Camera, microphone and screen sharing, while in use",
    ),
];

const POSITIONS: [(&str, BarPosition); 2] =
    [("Bottom", BarPosition::Bottom), ("Top", BarPosition::Top)];

const STYLES: [(&str, BarStyle); 3] = [
    ("Frosted", BarStyle::Frosted),
    ("Glass", BarStyle::Glass),
    ("Solid: black in dark mode, white in light", BarStyle::Solid),
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

    let clock = Column::new(vec![
        boxed(labelled(
            "24-hour time",
            Switch::new("settings.bar.clock-24h", now.clock_24h)
                .on_toggle(|on| change_status_bar(|now| now.clock_24h = on))
                .build(),
        )),
        boxed(labelled(
            "Show seconds",
            Switch::new("settings.bar.clock-seconds", now.clock_seconds)
                .on_toggle(|on| change_status_bar(|now| now.clock_seconds = on))
                .build(),
        )),
    ])
    .gap(sys::space::gap::s());

    let workspaces = Column::new(vec![
        boxed(labelled(
            "Show workspaces",
            Switch::new("settings.bar.workspaces", now.show_workspaces)
                .on_toggle(|on| change_status_bar(|now| now.show_workspaces = on))
                .build(),
        )),
        boxed(labelled(
            "Scroll over them to move between workspaces",
            Switch::new("settings.bar.scroll-workspaces", now.scroll_workspaces)
                .on_toggle(|on| change_status_bar(|now| now.scroll_workspaces = on))
                .build(),
        )),
    ])
    .gap(sys::space::gap::s());

    let mut icons: Vec<Box<dyn Widget>> = ICONS
        .into_iter()
        .map(|(name, label)| {
            boxed(labelled(
                label,
                Switch::new(format!("settings.bar.icon.{name}"), now.shows(name))
                    .on_toggle(move |on| change_status_bar(|now| now.set_shown(name, on)))
                    .build(),
            ))
        })
        .collect();
    icons.push(boxed(note(
        "Quick Settings and the clock always show. Each Quick Settings page can show or hide its icon too.",
    )));

    let hiding = Column::new(vec![
        boxed(labelled(
            "Hide the status bar until the pointer meets the screen's edge",
            Switch::new("settings.bar.auto-hide", now.auto_hide)
                .on_toggle(|on| change_status_bar(|now| now.auto_hide = on))
                .build(),
        )),
        boxed(note(
            "Hidden, it keeps no room from windows, and comes back with Quick Settings or the \
             notification center too. At the bottom, a hiding dock takes the edge first.",
        )),
    ])
    .gap(sys::space::gap::s());

    Column::new(vec![
        boxed(setting("Position", position)),
        boxed(setting("Hiding", hiding)),
        boxed(setting("Material", Column::new(styles))),
        boxed(setting("Clock", clock)),
        boxed(setting("Date in the clock's tooltip", date_format.build())),
        boxed(setting("Workspaces", workspaces)),
        boxed(setting(
            "Show in the status bar",
            Column::new(icons).gap(sys::space::gap::s()),
        )),
    ])
    .gap(sys::space::gap::l())
}
