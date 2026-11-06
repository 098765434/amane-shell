//! The status bar: which edge it runs along, what it's made of, its clock,
//! its workspaces and which icons it shows. Each control changes
//! `status-bar.json`, which the shell follows at once.

use amane::{Center, Column, Rectangle, Row, Size, Widget};
use amane_kit::buttons::{IconButton, PushButton, Radio, Segment, SegmentedButton, Switch};
use amane_kit::dropdown::Dropdown;
use amane_kit::form::FormRow;
use amane_kit::layout::{boxed, fixed_width};
use amane_kit::settings::{
    BAR_ITEMS, BarPosition, BarStyle, DateFormat, change_status_bar, status_bar, tray_seen,
};

use crate::{group, groups, radios_height, switch_row};

/// `expand_less` and `expand_more`, to move an item left or right.
const ICON_EARLIER: char = '\u{e5ce}';
const ICON_LATER: char = '\u{e5cf}';

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

    let order = now.items();
    let last = order.len().saturating_sub(1);
    let icons: Vec<FormRow> = order
        .iter()
        .enumerate()
        .map(|(index, name)| item_row(name, index, last, now.shows(name)))
        .collect();
    let reset = PushButton::new("settings.bar.reset-order", "Reset Order")
        .disabled(now.order.is_empty())
        .on_press(|| change_status_bar(|now| now.order.clear()))
        .build();

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
        allowed_apps(&now),
        group()
            .header("Items, Left to Right")
            .rows(icons)
            .row(FormRow::new("settings.bar.order", "Usual order").trailing(reset))
            .footer(
                "Drag an item along the bar to move it, or off the bar to hide it. Quick \
                 Settings and the clock always show, and each Quick Settings page can show or \
                 hide its icon too.",
            ),
    ])
}

/// An item of the bar: its switch, when it can be hidden, and buttons that
/// move it a place left or right.
fn item_row(name: &'static str, index: usize, last: usize, shown: bool) -> FormRow {
    let (_, label, hideable) = BAR_ITEMS
        .iter()
        .copied()
        .find(|item| item.0 == name)
        .unwrap_or((name, name, false));
    let id = format!("settings.bar.item.{name}");

    let mut trailing: Vec<Box<dyn Widget>> = vec![
        boxed(
            IconButton::new(format!("{id}.earlier"), ICON_EARLIER)
                .disabled(index == 0)
                .on_press(move || change_status_bar(|now| now.move_item(name, index - 1)))
                .build(),
        ),
        boxed(
            IconButton::new(format!("{id}.later"), ICON_LATER)
                .disabled(index == last)
                .on_press(move || change_status_bar(|now| now.move_item(name, index + 1)))
                .build(),
        ),
    ];
    let switch = Switch::new(format!("{id}.switch"), shown)
        .on_toggle(move |on| change_status_bar(|now| now.set_shown(name, on)))
        .build();
    if hideable {
        trailing.push(boxed(switch));
    } else {
        // the arrows line up with the rows that have a switch
        trailing.push(boxed(
            Rectangle::new()
                .width(Size::Fixed(fixed_width(&switch)))
                .height(Size::Fixed(1.0)),
        ));
    }
    FormRow::new(id, label).trailing(Row::new(trailing).gap(4.0).align(Center))
}

/// The apps the bar has had in its tray, each allowed in it or not, as
/// macOS's "Allow in the Menu Bar".
fn allowed_apps(now: &amane_kit::settings::StatusBar) -> amane_kit::form::Group {
    let seen = tray_seen();
    let rows: Vec<FormRow> = seen
        .iter()
        .map(|(id, title)| {
            let name = if title.is_empty() {
                id.clone()
            } else {
                title.clone()
            };
            let changed = id.clone();
            let mut row = switch_row(
                &format!("settings.bar.tray.{id}"),
                &name,
                now.allows(id),
                move |on| change_status_bar(|now| now.set_allowed(&changed, on)),
            );
            if !title.is_empty() && title != id {
                row = row.subtitle(id.clone());
            }
            row
        })
        .collect();
    let group = group().header("Allow in the Status Bar");
    if rows.is_empty() {
        group.footer(
            "Apps that put an icon in the tray, like Steam or Discord, show here once they have.",
        )
    } else {
        group
            .rows(rows)
            .footer("An app left out keeps running; only its icon is gone from the bar.")
    }
}
