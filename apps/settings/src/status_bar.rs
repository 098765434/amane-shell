//! The status bar: which edge it runs along, what it's made of, its clock,
//! its workspaces and which icons it shows. Each control changes
//! `status-bar.json`, which the shell follows at once.

use amane::TextInput;
use amane::{Center, Column, Rectangle, Row, Size, Widget};
use amane_kit::app::{Dialog, commit_on_blur, field_box, text_field};
use amane_kit::buttons::{IconButton, PushButton, Switch};
use amane_kit::dropdown::Dropdown;
use amane_kit::form::FormRow;
use amane_kit::layout::{boxed, fixed_width};
use amane_kit::settings::{
    AutoHide, BAR_ITEMS, BUILTIN_ACTIVITIES, BarPosition, BarSize, BarStyle, ClockStyle,
    DateFormat, ShowDate, StatusBar, activities_seen, change_status_bar, status_apps_seen,
    status_bar, tray_seen,
};
use amane_kit::style;
use amane_kit::thumbnail_picker::ThumbnailPicker;

use crate::{View, group, groups, previews, switch_row};

/// The weather place's field.
const PLACE: &str = "settings.bar.weather-place.field";

/// `expand_less` and `expand_more`, to move an item left or right.
const ICON_EARLIER: char = '\u{e5ce}';
const ICON_LATER: char = '\u{e5cf}';

const POSITIONS: [(&str, BarPosition); 2] =
    [("Bottom", BarPosition::Bottom), ("Top", BarPosition::Top)];

const STYLES: [(&str, BarStyle); 4] = [
    ("Clear", BarStyle::Clear),
    ("Frosted", BarStyle::Frosted),
    ("Glass", BarStyle::Glass),
    ("Solid", BarStyle::Solid),
];

pub fn page() -> Column {
    let now = status_bar().clone();

    let position = Dropdown::choice(
        "settings.bar.position",
        POSITIONS,
        now.position,
        |position| change_status_bar(|now| now.position = position),
    )
    .width(200.0)
    .build();

    let size = Dropdown::choice(
        "settings.bar.size",
        BarSize::ALL.map(|size| (size.label(), size)),
        now.size,
        |size| change_status_bar(|now| now.size = size),
    )
    .width(200.0)
    .build();

    let hiding = [
        "Never",
        "Always",
        "On the desktop only",
        "In full screen only",
    ];
    let auto_hide = Dropdown::choice(
        "settings.bar.auto-hide",
        hiding.into_iter().zip(AutoHide::ALL),
        now.auto_hide,
        |mode| change_status_bar(|now| now.auto_hide = mode),
    )
    .width(200.0);

    let width = crate::page_width() - 2.0 * style::row_pad_h();
    let mut styles = ThumbnailPicker::new("settings.bar.style", previews::WIDTH, previews::HEIGHT)
        .width(width)
        .chosen(STYLES.iter().position(|(_, style)| *style == now.style))
        .on_select(|index| change_status_bar(|now| now.style = STYLES[index].1));
    for (label, style) in STYLES {
        styles = styles.option(previews::bar(style, now.size), label);
    }
    let styles_height = styles.height();

    let shown = now.items().iter().filter(|name| now.shows(name)).count();

    groups(vec![
        group()
            .row(FormRow::new("settings.bar.position", "Position").trailing(position))
            .row(
                FormRow::new("settings.bar.size", "Size")
                    .subtitle("How tall it is, and how big its icons are")
                    .trailing(size),
            )
            .row(
                FormRow::new("settings.bar.auto-hide", "Hide automatically")
                    .subtitle("Until the pointer meets the screen's edge")
                    .trailing(auto_hide.build()),
            )
            .footer(
                "Hidden, it keeps no room from windows, and comes back with Quick Settings or the \
                 notification center too. At the bottom, a hiding dock takes the edge first.",
            ),
        group()
            .header("Material")
            .row(
                FormRow::new("settings.bar.material", "Look")
                    .detail(styles.build(), styles_height),
            )
            .footer("Clear draws only the items, light or dark to suit the wallpaper. Solid is black in dark mode and white in light."),
        group()
            .row(
                FormRow::new("settings.bar.clock-link", "Clock")
                    .value(clock_summary(&now))
                    .on_press(|| crate::open(View::Sub("clock"))),
            )
            .row(
                FormRow::new("settings.bar.weather-link", "Weather")
                    .value(weather_summary(&now))
                    .on_press(|| crate::open(View::Sub("weather"))),
            )
            .row(
                FormRow::new("settings.bar.items-link", "Items")
                    .value(format!("{shown} shown"))
                    .on_press(|| crate::open(View::Sub("items"))),
            )
            .row(
                FormRow::new("settings.bar.activities-link", "Activities")
                    .value(activities_summary(&now))
                    .on_press(|| crate::open(View::Sub("activities"))),
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
                .subtitle("Scroll over the workspaces to move between them")
                .child()
                .disabled(!now.show_workspaces),
            )
            .footer(
                "One shape per workspace: a circle, then a square, a triangle and on, changing \
                 as you move between them. Click it for the overview.",
            ),
    ])
}

/// Activities as their row sums them up: "Off", or how many show
/// with their text.
fn activities_summary(now: &StatusBar) -> String {
    if !now.shows("activities") {
        return String::from("Off");
    }
    match now.compact_activities() {
        1 => String::from("1 with its text"),
        many => format!("{many} with their text"),
    }
}

/// The clock as its row sums it up: "Analog", or "24-hour · seconds · date".
fn clock_summary(now: &StatusBar) -> String {
    if now.clock_style == ClockStyle::Analog {
        return String::from("Analog");
    }
    let mut parts = vec![if now.clock_24h { "24-hour" } else { "12-hour" }];
    if now.clock_seconds {
        parts.push("seconds");
    }
    match now.show_date {
        ShowDate::Always => parts.push("date"),
        ShowDate::WhenSpace => parts.push("date when space allows"),
        ShowDate::Never => {}
    }
    parts.join(" · ")
}

/// The weather as its row sums it up: the place, or why there's none.
fn weather_summary(now: &StatusBar) -> String {
    match (now.weather, now.weather_place.trim()) {
        (false, _) => String::from("Off"),
        (true, "") => String::from("No place set"),
        (true, place) => place.to_string(),
    }
}

/// The page's sub-pages: the clock, the weather, and the items in order.
pub fn sub(name: &str) -> Option<(String, Box<dyn Widget>)> {
    Some(match name {
        "clock" => (String::from("Clock"), boxed(clock())),
        "weather" => (String::from("Weather"), boxed(weather())),
        "items" => (String::from("Items"), boxed(items())),
        "activities" => (String::from("Activities"), boxed(activities())),
        _ => return None,
    })
}

fn clock() -> Column {
    let now = status_bar().clone();
    let analog = now.clock_style == ClockStyle::Analog;

    let width = crate::page_width() - 2.0 * style::row_pad_h();
    let styles = ThumbnailPicker::new(
        "settings.bar.clock-style",
        previews::WIDTH,
        previews::HEIGHT,
    )
    .width(width)
    .chosen(Some(usize::from(analog)))
    .on_select(|index| {
        change_status_bar(|now| {
            now.clock_style = if index == 1 {
                ClockStyle::Analog
            } else {
                ClockStyle::Digital
            }
        })
    })
    .option(previews::clock(false), "Digital")
    .option(previews::clock(true), "Analog");
    let styles_height = styles.height();

    let show_date = Dropdown::choice(
        "settings.bar.show-date",
        ["When space allows", "Always", "Never"]
            .into_iter()
            .zip(ShowDate::ALL),
        now.show_date,
        |when| change_status_bar(|now| now.show_date = when),
    )
    .width(200.0);

    let dates = [
        "Sunday 25 October",
        "Sun 25 Oct",
        "Numbers, as the locale has them",
    ];
    let date_format = Dropdown::choice(
        "settings.bar.date",
        dates.into_iter().zip(DateFormat::ALL),
        now.date_format,
        |format| change_status_bar(|now| now.date_format = format),
    )
    .width(240.0);

    groups(vec![
        group().row(
            FormRow::new("settings.bar.clock-style-row", "Style")
                .detail(styles.build(), styles_height),
        ),
        group()
            .header("Time")
            .row(
                switch_row(
                    "settings.bar.clock-24h",
                    "24-hour time",
                    now.clock_24h,
                    |on| change_status_bar(|now| now.clock_24h = on),
                )
                .disabled(analog),
            )
            .row(
                switch_row(
                    "settings.bar.clock-seconds",
                    "Show seconds",
                    now.clock_seconds,
                    |on| change_status_bar(|now| now.clock_seconds = on),
                )
                .disabled(analog),
            )
            .row(
                switch_row(
                    "settings.bar.flash",
                    "Flash the time separators",
                    now.flash_separators,
                    |on| change_status_bar(|now| now.flash_separators = on),
                )
                .disabled(analog),
            )
            .footer(if analog {
                "The analog face shows the hours and minutes only."
            } else {
                ""
            }),
        group()
            .header("Date")
            .row(
                FormRow::new("settings.bar.show-date", "Show the date").trailing(show_date.build()),
            )
            .row(switch_row(
                "settings.bar.weekday",
                "Show the day of the week",
                now.show_weekday,
                |on| change_status_bar(|now| now.show_weekday = on),
            ))
            .row(
                FormRow::new("settings.bar.date", "Date in the tooltip")
                    .trailing(date_format.build()),
            ),
    ])
}

fn weather() -> Column {
    let now = status_bar().clone();

    // the field shows the place kept until it's typed in
    amane_kit::app::fill_once(PLACE, || now.weather_place.clone());
    let keep = |text: String| {
        let text = text.trim().to_string();
        if text != status_bar().weather_place {
            change_status_bar(|now| now.weather_place = text);
        }
    };
    commit_on_blur(PLACE, keep);
    let place = field_box(
        PLACE,
        text_field(PLACE, "A town or city").on_submit(keep),
        220.0,
    );

    let typed = TextInput::text(PLACE);
    let state = if typed.trim() == now.weather_place.trim() {
        if now.weather_place.trim().is_empty() {
            String::from("Type a place to show its weather")
        } else {
            String::from("Kept")
        }
    } else {
        String::from("Press Return or click away to keep it")
    };

    groups(vec![
        group()
            .row(switch_row(
                "settings.bar.weather",
                "Show the weather",
                now.weather,
                |on| change_status_bar(|now| now.weather = on),
            ))
            .row(
                FormRow::new("settings.bar.weather-place", "Place")
                    .subtitle(state)
                    .trailing(place)
                    .child()
                    .disabled(!now.weather),
            )
            .footer(
                "Today's weather from Open-Meteo, read every half hour. Only the place's name and \
                 coordinates are sent.",
            ),
    ])
}

fn items() -> Column {
    let now = status_bar().clone();
    let order = now.items();
    let last = order.len().saturating_sub(1);
    let icons: Vec<FormRow> = order
        .iter()
        .enumerate()
        .map(|(index, name)| item_row(name, index, last, now.shows(name)))
        .collect();
    let reset = PushButton::new("settings.bar.reset-order", "Reset Order…")
        .disabled(now.order.is_empty())
        .on_press(|| {
            Dialog::new("settings.bar.reset-dialog", "Put the items back in order?")
                .message("They go back to the bar's usual order. Which ones show stays as it is.")
                .button("Cancel", || {})
                .destructive("Reset", || change_status_bar(|now| now.order.clear()))
                .open("main");
        })
        .build();

    groups(vec![
        group()
            .header("Items, Left to Right")
            .rows(icons)
            .row(FormRow::new("settings.bar.order", "Usual order").trailing(reset))
            .footer(
                "Drag an item along the bar to move it, or off the bar to hide it. Quick \
                 Settings and the clock always show, and each Quick Settings page can show or \
                 hide its icon too.",
            ),
        allowed_apps(&now),
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
                .width(fixed_width(&switch))
                .height(Size::Fixed(1.0)),
        ));
    }
    let (glyph, accent) = bar_item_icon(name);
    FormRow::new(id, label)
        .icon(glyph, accent)
        .trailing(Row::new(trailing).gap(4.0).align(Center))
}

/// Activities: whether they show, how many with their text, Now
/// Playing's "Always show", and which apps may show them.
fn activities() -> Column {
    let now = status_bar().clone();
    let on = now.shows("activities");

    let compact = Dropdown::new(
        "settings.bar.activities-compact",
        ["1 activity", "2 activities", "3 activities"]
            .map(String::from)
            .to_vec(),
        now.compact_activities().clamp(1, 3) - 1,
    )
    .width(200.0)
    .on_select(|index| change_status_bar(|now| now.activities_compact = index as u8 + 1))
    .build();

    let mut apps: Vec<String> = BUILTIN_ACTIVITIES.map(String::from).to_vec();
    apps.extend(
        activities_seen()
            .into_iter()
            .filter(|app| !BUILTIN_ACTIVITIES.contains(&app.as_str())),
    );
    let rows: Vec<FormRow> = apps
        .into_iter()
        .map(|app| {
            let changed = app.clone();
            switch_row(
                &format!("settings.bar.activity.{app}"),
                &app,
                now.allows_activity(&app),
                move |on| change_status_bar(|now| now.set_activity_allowed(&changed, on)),
            )
            .child()
            .disabled(!on)
        })
        .collect();

    groups(vec![
        group()
            .row(switch_row(
                "settings.bar.activities",
                "Show Activities",
                on,
                |on| change_status_bar(|now| now.set_shown("activities", on)),
            ))
            .row(
                FormRow::new("settings.bar.activities-compact", "Show with their text")
                    .subtitle("The most relevant; the rest show their icon alone")
                    .trailing(compact)
                    .child()
                    .disabled(!on),
            )
            .footer(
                "What's going on now: what plays, a timer, screen sharing, an upload. Click \
                 one for its controls.",
            ),
        group().header("Now Playing").row(
            switch_row(
                "settings.bar.media-always",
                "Always show",
                now.media_always,
                |on| change_status_bar(|now| now.media_always = on),
            )
            .subtitle("Else only while something plays, or was paused in the last few minutes"),
        ),
        group()
            .header("Allow Activities")
            .rows(rows)
            .footer("Other apps show here once they have started one."),
    ])
}

/// The apps the bar has had in its tray, each allowed in it or not.
fn allowed_apps(now: &amane_kit::settings::StatusBar) -> amane_kit::form::Group {
    let seen = tray_seen();
    // programs' own items (amane's status items), kept by `app:<id>`
    let own = status_apps_seen().into_iter().map(|(id, title)| {
        let name = format!("app:{id}");
        let label = if title.is_empty() {
            id.clone()
        } else {
            title.clone()
        };
        let changed = name.clone();
        let mut row = switch_row(
            &format!("settings.bar.app.{id}"),
            &label,
            now.shows(&name),
            move |on| change_status_bar(|now| now.set_shown(&changed, on)),
        );
        if !title.is_empty() && title != id {
            row = row.subtitle(id);
        }
        row
    });
    let tray = seen.iter().map(|(id, title)| {
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
    });
    let rows: Vec<FormRow> = own.chain(tray).collect();
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

/// The glyph and tile colour Shell Settings shows before a status bar
/// item: the glyph the bar itself draws at rest.
fn bar_item_icon(name: &str) -> (amane_kit::icons::Glyph, amane_kit::color::Accent) {
    use amane_kit::color::Accent;
    use amane_kit::icons::Glyph;
    let (glyph, accent) = match name {
        "tray" => (amane_kit::icons::ICON_APPS, Accent::Gray),
        "privacy" => (amane_kit::icons::ICON_MIC, Accent::Orange),
        "status-icons" => (amane_kit::icons::ICON_NIGHT_LIGHT, Accent::Indigo),
        "activities" => (amane_kit::icons::ICON_TIMER, Accent::Purple),
        "weather" => (amane_kit::icons::ICON_PARTLY_CLOUDY, Accent::Cyan),
        "volume" => (amane_kit::icons::ICON_VOLUME_UP, Accent::Pink),
        "bluetooth" => (amane_kit::icons::ICON_BLUETOOTH, Accent::Blue),
        "network" => (amane_kit::icons::ICON_WIFI, Accent::Blue),
        "tailscale" => return (Glyph::Tailscale, Accent::Gray),
        "keep-awake" => (amane_kit::icons::ICON_KEEP_AWAKE, Accent::Brown),
        "keyboard" => (amane_kit::icons::ICON_KEYBOARD, Accent::Gray),
        "phone" => (amane_kit::icons::ICON_PHONE, Accent::Green),
        "battery" => (amane_kit::icons::ICON_BATTERY_FULL, Accent::Green),
        "quick-settings" => (amane_kit::icons::ICON_TUNE, Accent::Gray),
        "notifications" => (amane_kit::icons::ICON_NOTIFICATIONS, Accent::Red),
        "clock" => (amane_kit::icons::ICON_SCHEDULE, Accent::Blue),
        _ => (amane_kit::icons::ICON_APPS, Accent::Gray),
    };
    (Glyph::Font(glyph), accent)
}
