//! The status bar: which edge it runs along, what it's made of, its clock,
//! its workspaces and which icons it shows. Each control changes
//! `status-bar.json`, which the shell follows at once.

use std::collections::BTreeMap;

use amane::{Center, Column, Rectangle, Row, Size, Widget};
use amane::{Service, TextInput, watch_file};
use emaki_kit::app::{Dialog, commit_on_blur, field_box, text_field};
use emaki_kit::buttons::{IconButton, PushButton, Switch};
use emaki_kit::dropdown::Dropdown;
use emaki_kit::form::FormRow;
use emaki_kit::layout::{boxed, fixed_width};
use emaki_kit::settings::{
    AutoHide, BAR_ITEMS, BUILTIN_ACTIVITIES, BarPosition, BarSize, BarStyle, ClockStyle,
    DateFormat, ShowDate, StatusBar, WeatherLookup, WeatherSeen, activities_seen,
    change_status_bar, status_apps_seen, status_bar, tray_seen, weather_seen, weather_seen_file,
};
use emaki_kit::style;
use emaki_kit::thumbnail_picker::ThumbnailPicker;

use crate::{View, group, groups, previews, switch_row};

/// The weather place's field.
const PLACE: &str = "settings.bar.weather-place.field";

/// `chevron_left` and `chevron_right`, to move an item left or right: the
/// bar's items run left to right.
const ICON_EARLIER: char = emaki_kit::icons::ICON_CHEVRON_LEFT;
const ICON_LATER: char = emaki_kit::icons::ICON_CHEVRON_RIGHT;

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
    .width(crate::MENU_W);

    let width = crate::page_width() - 2.0 * style::row_pad_h();
    let picker =
        |id: &'static str| ThumbnailPicker::new(id, previews::WIDTH, previews::HEIGHT).width(width);

    let mut positions = picker("settings.bar.position.picker")
        .chosen(POSITIONS.iter().position(|(_, at)| *at == now.position))
        .on_select(|index| change_status_bar(|now| now.position = POSITIONS[index].1));
    for (label, at) in POSITIONS {
        positions = positions.option(previews::bar(now.style, now.size, at), label);
    }
    let positions_height = positions.height();

    let mut sizes = picker("settings.bar.size.picker")
        .chosen(BarSize::ALL.iter().position(|size| *size == now.size))
        .on_select(|index| change_status_bar(|now| now.size = BarSize::ALL[index]));
    for size in BarSize::ALL {
        sizes = sizes.option(previews::bar(now.style, size, now.position), size.label());
    }
    let sizes_height = sizes.height();

    let mut styles = picker("settings.bar.style")
        .chosen(STYLES.iter().position(|(_, style)| *style == now.style))
        .on_select(|index| change_status_bar(|now| now.style = STYLES[index].1));
    for (label, style) in STYLES {
        styles = styles.option(previews::bar(style, now.size, now.position), label);
    }
    let styles_height = styles.height();

    let shown = now.items().iter().filter(|name| now.shows(name)).count();

    groups(vec![
        group()
            .header("Position and size")
            .row(
                FormRow::new("settings.bar.position", "Position")
                    .detail(positions.build(), positions_height),
            )
            .row(
                FormRow::new("settings.bar.size", "Size")
                    .subtitle("How tall it is, and how big its icons are")
                    .detail(sizes.build(), sizes_height),
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
            .header("Look")
            .row(
                FormRow::new("settings.bar.material", "Material")
                    .detail(styles.build(), styles_height),
            )
            .footer("Clear draws only the items, light or dark to suit the wallpaper. Solid is black in dark mode and white in light."),
        group()
            .header("In the bar")
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
            )
            .row(
                FormRow::new("settings.bar.quick-settings-link", "Quick Settings")
                    .value(crate::quick_settings::summary())
                    .on_press(|| crate::open(View::Sub("quick-settings"))),
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
                 as you move between them. An empty workspace shows as an outline. Click it \
                 for the overview.",
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
        "quick-settings" => (
            String::from("Quick Settings"),
            boxed(crate::quick_settings::page()),
        ),
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
    .width(crate::MENU_W);

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
    .width(crate::MENU_W_WIDE);

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
                    .subtitle(
                        "How the clock's tooltip writes the day; the bar's date is always short",
                    )
                    .trailing(date_format.build()),
            ),
    ])
}

fn weather() -> Column {
    let now = status_bar().clone();

    // the field shows the place kept until it's typed in
    emaki_kit::app::fill_once(PLACE, || now.weather_place.clone());
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
        crate::FIELD_W,
    );

    let typed = TextInput::text(PLACE);
    let seen = WeatherWatch::read().seen.clone();
    let state = place_state(typed.trim(), now.weather_place.trim(), now.weather, &seen);

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

/// What came of the shell's last weather lookup, followed as the shell
/// writes it, so the place's row says it at once.
struct WeatherWatch {
    seen: WeatherSeen,
}

impl Service for WeatherWatch {
    fn new() -> Self {
        Self {
            seen: weather_seen(),
        }
    }

    fn listen() {
        let path = weather_seen_file();
        // the folder is watched, so it has to be there
        if let Some(folder) = path.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        for _ in watch_file(&path.to_string_lossy()) {
            let seen = weather_seen();
            let mut now = Self::write();
            if now.seen == seen {
                now.quiet();
                continue;
            }
            now.seen = seen;
        }
    }
}

/// What the place's row says: what to do, or what came of the shell
/// looking `kept` up (`seen`), once it's been typed in as `typed`.
fn place_state(typed: &str, kept: &str, on: bool, seen: &WeatherSeen) -> String {
    if typed != kept {
        return String::from("Press Return or click away to keep it");
    }
    if kept.is_empty() {
        return String::from("Type a place to show its weather");
    }
    if !on {
        return String::from("Kept");
    }
    // the shell looks a new place up as soon as it's kept
    if seen.place != kept {
        return String::from("Looking it up…");
    }
    match seen.lookup {
        WeatherLookup::Pending => String::from("Looking it up…"),
        WeatherLookup::Found => String::from("Kept"),
        WeatherLookup::NotFound => {
            format!("Couldn't find \u{201c}{kept}\u{201d}. Try a nearby town.")
        }
        WeatherLookup::Unreachable => {
            String::from("Couldn't reach Open-Meteo. Trying again in a few minutes.")
        }
    }
}

fn items() -> Column {
    let now = status_bar().clone();
    // programs' own items too, each by `app:<id>`, so moving one keeps the rest in place
    let seen = status_apps_seen();
    let apps: Vec<String> = seen.keys().map(|id| format!("app:{id}")).collect();
    let order = now.items_with(&apps);
    let last = order.len().saturating_sub(1);
    let icons: Vec<FormRow> = order
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let row = ItemRow {
                name: name.clone(),
                index,
                last,
                shown: now.shows(name),
            };
            item_row(row, &apps, &seen)
        })
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

/// One row of the Items list: the item's name in the order, where it is,
/// and whether it shows.
struct ItemRow {
    name: String,
    index: usize,
    last: usize,
    shown: bool,
}

/// An item of the bar: its switch, when it can be hidden, and buttons that
/// move it a place left or right. `apps` are the programs' items among the
/// order, and `seen` what each program calls itself.
fn item_row(row: ItemRow, apps: &[String], seen: &BTreeMap<String, String>) -> FormRow {
    let ItemRow {
        name,
        index,
        last,
        shown,
    } = row;
    let (label, hideable) = match name.strip_prefix("app:") {
        Some(id) => (
            seen.get(id)
                .filter(|title| !title.is_empty())
                .cloned()
                .unwrap_or_else(|| id.to_string()),
            true,
        ),
        None => BAR_ITEMS
            .iter()
            .find(|item| item.0 == name)
            .map_or((name.clone(), false), |item| (item.1.to_string(), item.2)),
    };
    let id = format!("settings.bar.item.{name}");

    let mover = |delta: isize| {
        let (name, apps) = (name.clone(), apps.to_vec());
        move || {
            let to = index.saturating_add_signed(delta);
            change_status_bar(|now| now.move_item_with(&name, to, &apps));
        }
    };
    let mut trailing: Vec<Box<dyn Widget>> = vec![
        boxed(
            IconButton::new(format!("{id}.earlier"), ICON_EARLIER)
                .tooltip(crate::WINDOW, "Move left")
                .disabled(index == 0)
                .on_press(mover(-1))
                .build(),
        ),
        boxed(
            IconButton::new(format!("{id}.later"), ICON_LATER)
                .tooltip(crate::WINDOW, "Move right")
                .disabled(index == last)
                .on_press(mover(1))
                .build(),
        ),
    ];
    let toggled = name.clone();
    let switch = Switch::new(format!("{id}.switch"), shown)
        .on_toggle(move |on| change_status_bar(|now| now.set_shown(&toggled, on)))
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
    let (glyph, accent) = bar_item_icon(&name);
    FormRow::new(id, label).icon(glyph, accent).trailing(
        Row::new(trailing)
            .gap(emaki_kit::tokens::sys::space::gap::xs())
            .align(Center),
    )
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
    .width(crate::MENU_W)
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
                "Show activities",
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
            .subtitle("Show Now Playing even when nothing is playing. Off, it shows while something plays or was paused in the last few minutes"),
        ),
        group()
            .header("Allow Activities")
            .rows(rows)
            .footer("Other apps show here once they have started one."),
    ])
}

/// The apps the bar has had in its tray, each allowed in it or not.
fn allowed_apps(now: &emaki_kit::settings::StatusBar) -> emaki_kit::form::Group {
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

/// The glyph and tile colour Emaki Settings shows before a status bar
/// item: the glyph the bar itself draws at rest.
fn bar_item_icon(name: &str) -> (emaki_kit::icons::Glyph, emaki_kit::color::Accent) {
    use emaki_kit::color::Accent;
    use emaki_kit::icons::Glyph;
    let (glyph, accent) = match name {
        "tray" => (emaki_kit::icons::ICON_APPS, Accent::Gray),
        "privacy" => (emaki_kit::icons::ICON_MIC, Accent::Orange),
        "status-icons" => (emaki_kit::icons::ICON_NIGHT_LIGHT, Accent::Indigo),
        "activities" => (emaki_kit::icons::ICON_TIMER, Accent::Purple),
        "weather" => (emaki_kit::icons::ICON_PARTLY_CLOUDY, Accent::Cyan),
        "volume" => (emaki_kit::icons::ICON_VOLUME_UP, Accent::Pink),
        "bluetooth" => (emaki_kit::icons::ICON_BLUETOOTH, Accent::Blue),
        "network" => (emaki_kit::icons::ICON_WIFI, Accent::Blue),
        "tailscale" => return (Glyph::Tailscale, Accent::Gray),
        "keep-awake" => (emaki_kit::icons::ICON_KEEP_AWAKE, Accent::Brown),
        "keyboard" => (emaki_kit::icons::ICON_KEYBOARD, Accent::Gray),
        "phone" => (emaki_kit::icons::ICON_PHONE, Accent::Green),
        "battery" => (emaki_kit::icons::ICON_BATTERY_FULL, Accent::Green),
        "quick-settings" => (emaki_kit::icons::ICON_TUNE, Accent::Gray),
        "notifications" => (emaki_kit::icons::ICON_NOTIFICATIONS, Accent::Red),
        "clock" => (emaki_kit::icons::ICON_SCHEDULE, Accent::Blue),
        _ => (emaki_kit::icons::ICON_APPS, Accent::Gray),
    };
    (Glyph::Font(glyph), accent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(place: &str, lookup: WeatherLookup) -> WeatherSeen {
        WeatherSeen {
            place: String::from(place),
            lookup,
        }
    }

    #[test]
    fn the_place_says_why_no_weather_shows() {
        let found = seen("Oslo", WeatherLookup::Found);
        assert_eq!(
            place_state("Osl", "Oslo", true, &found),
            "Press Return or click away to keep it"
        );
        assert_eq!(place_state("Oslo", "Oslo", true, &found), "Kept");
        let missing = seen("Atlantis", WeatherLookup::NotFound);
        assert!(place_state("Atlantis", "Atlantis", true, &missing).starts_with("Couldn't find"));
        // the shell's word on another place: it's looking this one up
        assert_eq!(
            place_state("Oslo", "Oslo", true, &missing),
            "Looking it up…"
        );
        let down = seen("Oslo", WeatherLookup::Unreachable);
        assert!(place_state("Oslo", "Oslo", true, &down).starts_with("Couldn't reach"));
        assert_eq!(place_state("Oslo", "Oslo", false, &down), "Kept");
    }
}
