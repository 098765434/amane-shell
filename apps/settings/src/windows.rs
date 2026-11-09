//! Windows: how niri lays windows out, how it marks the focused one, and
//! how to switch between them. These are niri's own settings, kept in
//! `settings.kdl` ([`crate::niri_settings`]): a setting changed here wins
//! over `config.kdl`'s, and Reset hands it back.

use amane::Column;
use emaki_kit::buttons::PushButton;
use emaki_kit::color::Accent;
use emaki_kit::dropdown::Dropdown;
use emaki_kit::form::{FormRow, Group, SliderRow};
use emaki_kit::icons::{ICON_LAYERS, ICON_RESIZE, ICON_SEARCH};
use emaki_kit::slider::Slider;
use emaki_kit::stepper::Stepper;

use crate::niri_settings::{self, Key, Shown};
use crate::{group, groups};

const GAPS: Key = ("layout", "gaps");
const WIDTH: Key = ("layout", "default-column-width");
const CENTER: Key = ("layout", "center-focused-column");
const SINGLE: Key = ("layout", "always-center-single-column");
const EMPTY_ABOVE: Key = ("layout", "empty-workspace-above-first");
const RING: Key = ("layout/focus-ring", "state");
const RING_WIDTH: Key = ("layout/focus-ring", "width");
const RING_COLOR: Key = ("layout/focus-ring", "active-color");
const BORDER: Key = ("layout/border", "state");
const BORDER_WIDTH: Key = ("layout/border", "width");
const SHADOW: Key = ("layout/shadow", "state");
const MAT: Key = ("layout/mat", "state");
const NO_CSD: Key = ("", "prefer-no-csd");
const SWITCHER: Key = ("recent-windows", "state");
const ZOOM: Key = ("overview", "zoom");
const CORNERS: Key = ("gestures", "hot-corners");

/// Every setting the page changes, for Reset.
const ALL: &[Key] = &[
    GAPS,
    WIDTH,
    CENTER,
    SINGLE,
    EMPTY_ABOVE,
    RING,
    RING_WIDTH,
    RING_COLOR,
    BORDER,
    BORDER_WIDTH,
    SHADOW,
    MAT,
    NO_CSD,
    SWITCHER,
    ZOOM,
    CORNERS,
];

/// New windows' widths, as niri writes them.
const WIDTHS: [(&str, &str); 4] = [
    ("A third", "{ proportion 0.33333; }"),
    ("Half", "{ proportion 0.5; }"),
    ("Two thirds", "{ proportion 0.66667; }"),
    ("As the app asks", "{ }"),
];

const CENTERING: [(&str, &str); 3] = [
    ("Never", "\"never\""),
    ("Always", "\"always\""),
    ("When it doesn't fit", "\"on-overflow\""),
];

const HOT_CORNERS: [(&str, &str); 5] = [
    ("None", "{ off; }"),
    ("Top left", "{ top-left; }"),
    ("Top right", "{ top-right; }"),
    ("Bottom left", "{ bottom-left; }"),
    ("Bottom right", "{ bottom-right; }"),
];

pub fn page() -> Column {
    let shown = niri_settings::shown();
    if !shown.looked {
        return groups(vec![group().row(crate::loading_row(
            "settings.windows.reading",
            "Reading niri's settings…",
        ))]);
    }
    let mut list = niri_settings::notices("settings.windows");
    list.push(layout(&shown));
    list.push(look(&shown));
    list.push(switching(&shown));
    if shown.editable && shown.any_ours(ALL) {
        list.push(
            group()
                .row(
                    FormRow::new("settings.windows.reset", "Reset the window settings").trailing(
                        PushButton::new("settings.windows.reset.button", "Reset…")
                            .on_press(|| {
                                emaki_kit::app::Dialog::new(
                                    "settings.windows.reset-dialog",
                                    "Reset the window settings?",
                                )
                                .message(
                                    "Every setting on this page goes back to what niri's \
                                     config.kdl sets.",
                                )
                                .button("Cancel", || {})
                                .destructive("Reset", || {
                                    niri_settings::reset("windows reset", ALL);
                                })
                                .open("main");
                            })
                            .build(),
                    ),
                )
                .footer("Hands every setting on this page back to niri's config.kdl."),
        );
    }
    groups(list)
}

/// A dropdown of niri's values for `key`; `index` finds the one in force.
fn choice_row(
    shown: &Shown,
    id: &'static str,
    title: &str,
    key: Key,
    choices: &'static [(&'static str, &'static str)],
    index: fn(&str) -> Option<usize>,
) -> FormRow {
    let now = shown.get(key).as_deref().and_then(index).unwrap_or(0);
    let labels = choices.iter().map(|(label, _)| label.to_string()).collect();
    let menu = Dropdown::new(format!("{id}.menu"), labels, now)
        .on_select(move |picked| {
            if let Some((_, value)) = choices.get(picked) {
                niri_settings::set(what(id), key, *value);
            }
        })
        .width(crate::MENU_W)
        .disabled(!shown.editable)
        .build();
    FormRow::new(id, title).trailing(menu)
}

/// What a commit says was changed: the row's id after the page's.
fn what(id: &'static str) -> &'static str {
    id.strip_prefix("settings.windows.").unwrap_or(id)
}

fn layout(shown: &Shown) -> Group {
    let gaps: u16 = shown.number(GAPS, 16).min(64);
    let editable = shown.editable;
    group()
        .header("Layout")
        .slider(
            SliderRow::new("Gaps between windows", move |width| {
                Slider::new(
                    ((f32::from(gaps) / 32.0) * 100.0).round().min(100.0) as u8,
                    width,
                )
                .wheel(false)
                .inert(!editable)
                .on_value_change(|value| {
                    let px = (f32::from(value) / 100.0 * 32.0).round() as u16;
                    niri_settings::set_settled("gaps", GAPS, px.to_string());
                })
                .build()
            })
            .value(format!("{gaps} px")),
        )
        .row(choice_row(
            shown,
            "settings.windows.width",
            "New windows' width",
            WIDTH,
            &WIDTHS,
            width_index,
        ))
        .row(choice_row(
            shown,
            "settings.windows.center",
            "Center the focused column",
            CENTER,
            &CENTERING,
            |value| CENTERING.iter().position(|(_, v)| *v == value),
        ))
        .row(niri_settings::switch(
            shown,
            "settings.windows.single",
            "Center a lone window",
            "center a lone window",
            SINGLE,
        ))
        .row(
            niri_settings::switch(
                shown,
                "settings.windows.empty-above",
                "An empty workspace above the first",
                "empty workspace above",
                EMPTY_ABOVE,
            )
            .subtitle("As well as the one always below the last"),
        )
}

/// Which of [`WIDTHS`] a `default-column-width` is, by its proportion.
fn width_index(value: &str) -> Option<usize> {
    let inner = value
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    let inner = inner.trim_end_matches(';').trim();
    if inner.is_empty() {
        return Some(3);
    }
    let proportion: f64 = inner.strip_prefix("proportion")?.trim().parse().ok()?;
    [1.0 / 3.0, 0.5, 2.0 / 3.0]
        .iter()
        .position(|p| (p - proportion).abs() < 0.01)
}

fn look(shown: &Shown) -> Group {
    let editable = shown.editable;
    let ring = shown.on(RING);
    let ring_width: u16 = shown.number(RING_WIDTH, 4);
    let border = shown.on(BORDER);
    let border_width: u16 = shown.number(BORDER_WIDTH, 4);
    let follows = shown.follows_accent();
    let mut look = group().header("Decorations").row(
        niri_settings::switch(
            shown,
            "settings.windows.focus-ring",
            "Focus ring",
            "focus ring",
            RING,
        )
        .icon(ICON_RESIZE, Accent::Blue)
        .subtitle("A line around the focused window, outside it"),
    );
    // the ring's and border's own settings stay in sight, dimmed while off
    look = look
        .row(width_row(
            "settings.windows.focus-ring.width",
            ring_width,
            editable,
            ring,
            RING_WIDTH,
        ))
        .row(
            crate::child_switch_row(
                "settings.windows.focus-ring.accent",
                "Use the accent color",
                follows,
                editable && ring,
                niri_settings::set_follows_accent,
            )
            .subtitle(if follows {
                String::from("Follows the accent chosen under Appearance")
            } else {
                format!("Now {}", shown.word(RING_COLOR, "#7fc8ff"))
            }),
        );
    look = look.row(
        niri_settings::switch(shown, "settings.windows.border", "Border", "border", BORDER)
            .subtitle("A line inside each window's edge, which takes room from it"),
    );
    look = look.row(width_row(
        "settings.windows.border.width",
        border_width,
        editable,
        border,
        BORDER_WIDTH,
    ));
    look.row(niri_settings::switch(
        shown,
        "settings.windows.shadow",
        "Shadows",
        "shadow",
        SHADOW,
    ))
    .row(
        niri_settings::switch(shown, "settings.windows.mat", "Mat", "mat", MAT)
            .subtitle("A backing behind the columns, filling the gaps between them"),
    )
    .row(niri_settings::switch(
        shown,
        "settings.windows.no-csd",
        "Ask apps to leave out their title bars",
        "title bars",
        NO_CSD,
    ))
    .footer("Apps take the title bar setting as they start again.")
}

/// A line's width in px, set in under its switch and dimmed while it's off;
/// the stepper shows the number itself.
fn width_row(id: &'static str, width: u16, editable: bool, on: bool, key: Key) -> FormRow {
    FormRow::new(id, "Width (px)")
        .child()
        .disabled(!on)
        .trailing(
            Stepper::new(format!("{id}.stepper"), f64::from(width))
                .range(1.0, 16.0)
                .step(1.0)
                .width(crate::COMPACT_W)
                .disabled(!editable || !on)
                .on_change(move |px| {
                    niri_settings::set_settled(what(id), key, (px.round() as u16).to_string())
                })
                .build(),
        )
}

fn switching(shown: &Shown) -> Group {
    let zoom: f64 = shown.number(ZOOM, 0.5);
    let editable = shown.editable;
    // niri keeps the overview's zoom to 0.75 at most
    let at = (((zoom.clamp(0.25, 0.75) - 0.25) / 0.5) * 100.0).round() as u8;
    group()
        .header("Switching")
        .row(
            niri_settings::switch(
                shown,
                "settings.windows.switcher",
                "Window switcher",
                "window switcher",
                SWITCHER,
            )
            .icon(ICON_LAYERS, Accent::Indigo)
            .subtitle("Alt+Tab shows the windows, the most recent first"),
        )
        .slider(
            SliderRow::new("Overview size", move |width| {
                Slider::new(at, width)
                    .wheel(false)
                    .inert(!editable)
                    .on_value_change(|value| {
                        let zoom = 0.25 + f64::from(value) / 100.0 * 0.5;
                        niri_settings::set_settled(
                            "overview zoom",
                            ZOOM,
                            format!("{:.2}", (zoom * 100.0).round() / 100.0),
                        );
                    })
                    .build()
            })
            .icon(ICON_SEARCH, Accent::Gray)
            .value(format!("{:.0}%", zoom.clamp(0.25, 0.75) * 100.0)),
        )
        .row(
            choice_row(
                shown,
                "settings.windows.hot-corner",
                "Hot corner",
                CORNERS,
                &HOT_CORNERS,
                corner_index,
            )
            .subtitle("Pushing the pointer into it opens the overview"),
        )
}

/// Which of [`HOT_CORNERS`] a `hot-corners` block is: none when it's off,
/// else the first corner it names, else niri's top left.
fn corner_index(value: &str) -> Option<usize> {
    if value.contains("off") {
        return Some(0);
    }
    let named = HOT_CORNERS
        .iter()
        .skip(1)
        .position(|(_, v)| value.contains(v.trim_matches(['{', '}', ' ', ';'])));
    Some(named.map_or(1, |at| at + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_are_found_by_their_proportion() {
        assert_eq!(width_index("{ proportion 0.5; }"), Some(1));
        assert_eq!(width_index("{ proportion 0.33333; }"), Some(0));
        assert_eq!(width_index("{ proportion 0.66667; }"), Some(2));
        assert_eq!(width_index("{ }"), Some(3));
        assert_eq!(width_index("{}"), Some(3));
        assert_eq!(width_index("{ fixed 800; }"), None);
    }

    #[test]
    fn hot_corners_read_as_one() {
        assert_eq!(corner_index("{ off; }"), Some(0));
        assert_eq!(corner_index("{ top-right; }"), Some(2));
        assert_eq!(corner_index("{ bottom-left; top-right; }"), Some(2));
        assert_eq!(corner_index("{ }"), Some(1), "niri's own is the top left");
    }
}
