//! Lists: the kit's new Material 3 list component next to Material's own
//! versions and the lists the shell has today, to compare and try:
//! `cargo run -p amane-kit --example lists`.
//!
//! Comparison pages have five columns: today's kit `list::ListItem`, the
//! Settings form's `FormRow`, and the new `lists::ListItem` in the
//! shell's style, Material 3's baseline and Material 3 Expressive. Under
//! each column its items' heights are measured, to check against the spec
//! (`docs/kit/lists-m3.md`). `LISTS_PAGE=n` opens on a page, for
//! screenshots; `AMANE_THEME=light|dark` and `AMANE_DENSITY=compact` show
//! the shell's other looks, and the toolbar flips the M3 colours.

use std::cell::RefCell;
use std::time::Instant;

use amane::{
    Center, Color, Column, Key, Radius, Rectangle, Row, Scroll, ScrollArea, Service, Size, Start,
    Text, Widget, Window, request_frame,
};
use amane_kit::app::{Page, app, app_window, sidebar, standard_shortcut};
use amane_kit::avatar::Avatar;
use amane_kit::buttons::{Checkbox, PushButton, Radio, Switch};
use amane_kit::color::Accent;
use amane_kit::form::{FormRow, Group};
use amane_kit::icons::{
    ICON_APPS, ICON_BLUETOOTH, ICON_CHECK, ICON_CHEVRON_RIGHT, ICON_FOLDER, ICON_HEADPHONES,
    ICON_IMAGE, ICON_INFO, ICON_KEYBOARD, ICON_LAYERS, ICON_MAIL, ICON_MOUSE, ICON_PERSON,
    ICON_PHONE, ICON_SEARCH, ICON_SETTINGS, ICON_SPEAKER, ICON_STAR, ICON_SWAP, ICON_TUNE,
    ICON_WIFI,
};
use amane_kit::kinetic::{Axis, Kinetic};
use amane_kit::layout::{boxed, icon, pad};
use amane_kit::list;
use amane_kit::lists::{
    self, Dividers, ItemKind, Leading, List, ListItem, ListStyle, Selection, State, Trailing,
    scheme,
};
use amane_kit::macform::theme as mac;
use amane_kit::material::{self, Level};
use amane_kit::scroll_view::ScrollView;
use amane_kit::theme::Mode;
use amane_kit::tokens::{Styled, sys};

struct Lab {
    page: usize,
    // the M3 colours: 0 the shell's mode, 1 light, 2 dark
    m3_mode: usize,
    // the states page: real pointer and keys instead of forced states
    live: bool,
    single: Option<usize>,
    multi: Vec<usize>,
    switches: Vec<usize>,
    open: Vec<bool>,
    order: Vec<usize>,
    pressed: String,
}

impl Service for Lab {
    fn new() -> Self {
        Self {
            page: std::env::var("LISTS_PAGE")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(0),
            m3_mode: 0,
            live: false,
            single: Some(1),
            multi: vec![0, 2],
            switches: vec![1],
            open: vec![true, false],
            order: (0..5).collect(),
            pressed: String::from("nothing yet"),
        }
    }

    fn listen() {}
}

thread_local! {
    // the long lists' scroll, one per style
    static SCROLLS: RefCell<[Kinetic; 3]> = RefCell::new([Kinetic::new(), Kinetic::new(), Kinetic::new()]);
    // how long building each long list took
    static BUILD_TIMES: RefCell<[u128; 3]> = const { RefCell::new([0; 3]) };
}

const PAGES: [&str; 9] = [
    "Anatomy",
    "Line types",
    "Leading",
    "Trailing",
    "States",
    "Selection",
    "Dividers & sections",
    "Interaction",
    "Current lists",
];

/// A comparison column's width, and the list's inside its backdrop.
const COL: f32 = 232.0;
const INSET: f32 = 8.0;
const W: f32 = COL - 2.0 * INSET;
/// A three-column page's.
const WIDE: f32 = 340.0;

fn main() {
    app("amane-lists-showcase")
        .popup(amane_kit::tooltip::popup)
        .normal_window("lists", window)
        .run();
}

fn window() -> Window {
    let state = Lab::read();
    scheme::set_mode(match state.m3_mode {
        1 => Some(Mode::Light),
        2 => Some(Mode::Dark),
        _ => None,
    });

    let pages: Vec<Page> = [
        ICON_INFO,
        ICON_LAYERS,
        ICON_PERSON,
        ICON_CHEVRON_RIGHT,
        ICON_TUNE,
        ICON_CHECK,
        ICON_FOLDER,
        ICON_KEYBOARD,
        ICON_APPS,
    ]
    .iter()
    .zip(PAGES)
    .map(|(glyph, name)| Page::new(*glyph, name))
    .collect();
    let nav = sidebar("lists.nav", &pages, state.page, |page| {
        Lab::write().page = page
    });

    let page: Vec<Box<dyn Widget>> = match state.page {
        0 => anatomy(),
        1 => line_types(),
        2 => leading(),
        3 => trailing(),
        4 => states(&state),
        5 => selection(&state),
        6 => dividers(&state),
        7 => interaction(&state),
        _ => current(&state),
    };

    let mut body: Vec<Box<dyn Widget>> = vec![
        boxed(
            Text::new(PAGES[state.page.min(PAGES.len() - 1)])
                .style(mac::page_title_font())
                .color(mac::label_primary()),
        ),
        boxed(toolbar(&state)),
    ];
    body.extend(page);

    let content = Row::new(vec![
        boxed(Column::new(vec![boxed(nav)]).width(Size::Fixed(170.0))),
        boxed(ScrollArea::new(
            "lists.page",
            Column::new(body)
                .gap(sys::space::gap::l())
                .width(Size::Parent),
        )),
    ])
    .gap(sys::space::gap::l())
    .width(Size::Parent)
    .height(Size::Parent);

    app_window(content)
        .title("Lists showcase")
        .size(1640.0, 960.0)
        .min_size(1200.0, 640.0)
        .on_key(|key: Key| {
            for s in 0..3 {
                lists::handle_key(&format!("kbd.{s}"), key);
            }
        })
        .on_shortcut(standard_shortcut)
}

// ---- layout ----

fn caption(text: impl Into<String>) -> Text {
    Text::new(text)
        .style(sys::r#type::caption())
        .color(material::label(Level::Secondary))
}

fn heading(text: impl Into<String>) -> Box<dyn Widget> {
    boxed(
        Text::new(text)
            .style(sys::r#type::title())
            .color(material::label(Level::Primary)),
    )
}

fn fixed_h(widget: &dyn Widget) -> f32 {
    match widget.height() {
        Size::Fixed(h) => h,
        Size::Parent => 0.0,
    }
}

/// `list` on its style's backdrop (an M3 list on its surface), `INSET`
/// all round.
fn on_backdrop(style: ListStyle, list: Rectangle, width: f32) -> Rectangle {
    let h = fixed_h(&list);
    let fill = match style {
        ListStyle::Shell => Color::TRANSPARENT,
        _ => style.tokens().backdrop,
    };
    Rectangle::new()
        .width(Size::Fixed(width + 2.0 * INSET))
        .height(Size::Fixed(h + 2.0 * INSET))
        .fill(fill)
        .radius(12.0)
        .padding(pad(INSET, INSET))
        .child(list)
}

/// A column: its title, the widget, and what was measured.
fn column(title: &str, widget: Box<dyn Widget>, note: String, width: f32) -> Box<dyn Widget> {
    boxed(
        Column::new(vec![
            boxed(caption(title.to_uppercase())),
            widget,
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(width))
                    .height(Size::Fixed(32.0))
                    .fill(Color::TRANSPARENT)
                    .align_child(Start, Start)
                    .child(caption(note).wrap()),
            ),
        ])
        .gap(sys::space::gap::s())
        .width(Size::Fixed(width)),
    )
}

/// What a current list can't show.
fn unsupported(why: &str) -> Box<dyn Widget> {
    boxed(
        Rectangle::new()
            .width(Size::Fixed(COL))
            .height(Size::Fixed(56.0))
            .radius(8.0)
            .fill(Color::TRANSPARENT)
            .border(1.0, material::separator())
            .padding(pad(12.0, 8.0))
            .align_child(Start, Center)
            .child(caption(format!("Not supported: {why}")).wrap()),
    )
}

/// The new list in one style: the list on its backdrop, and its items'
/// heights.
fn new_list(
    style: ListStyle,
    id: &str,
    width: f32,
    items: impl Fn(&str) -> Vec<ListItem>,
    tune: impl FnOnce(List) -> List,
) -> (Box<dyn Widget>, String) {
    let prefix = format!("{id}.{style:?}");
    let heights: Vec<String> = items(&format!("{prefix}.m"))
        .into_iter()
        .map(|item| format!("{}", fixed_h(&item.render(style, width))))
        .collect();
    let list = tune(
        List::new(prefix.clone(), items(&prefix))
            .style(style)
            .width(width),
    )
    .build();
    (
        boxed(on_backdrop(style, list, width)),
        format!("heights {}", heights.join(" · ")),
    )
}

/// The three new styles' columns for `items`.
fn new_columns(
    id: &str,
    width: f32,
    items: impl Fn(&str) -> Vec<ListItem> + Clone,
    tune: impl Fn(List) -> List + Clone,
) -> Vec<Box<dyn Widget>> {
    ListStyle::ALL
        .into_iter()
        .map(|style| {
            let (widget, note) = new_list(style, id, width, items.clone(), tune.clone());
            column(
                &format!("New · {}", style.name()),
                widget,
                note,
                width + 2.0 * INSET,
            )
        })
        .collect()
}

/// A comparison: today's kit list and form, then the three new styles.
fn five(
    old: (Box<dyn Widget>, String),
    form: (Box<dyn Widget>, String),
    items: impl Fn(&str) -> Vec<ListItem> + Clone,
    id: &str,
) -> Box<dyn Widget> {
    let mut cols = vec![
        column("Current · kit list", old.0, old.1, COL),
        column("Current · Settings form", form.0, form.1, COL),
    ];
    cols.extend(new_columns(id, W, items, |l| l));
    boxed(Row::new(cols).gap(sys::space::gap::l()).align(Start))
}

/// Today's kit items, built `COL` wide, and their heights.
fn old(make: impl Fn() -> Vec<list::ListItem>) -> (Box<dyn Widget>, String) {
    let heights: Vec<String> = list::with_row_width(COL, || {
        make()
            .into_iter()
            .map(|item| format!("{}", fixed_h(&item.build())))
            .collect()
    });
    let widget = list::with_row_width(COL, || list::List::new(make()).build());
    (
        boxed(Column::new(vec![boxed(widget)]).width(Size::Fixed(COL))),
        format!("heights {}", heights.join(" · ")),
    )
}

/// Settings form rows in a group, and their heights.
fn form(make: impl Fn() -> Vec<FormRow>) -> (Box<dyn Widget>, String) {
    let heights: Vec<String> = make()
        .iter()
        .map(|r| format!("{}", r.height(COL).round()))
        .collect();
    let group = Group::new().width(COL).rows(make()).build();
    (boxed(group), format!("heights {}", heights.join(" · ")))
}

fn toolbar(s: &Lab) -> Row {
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(caption("M3 colours"))];
    for (i, name) in ["Follow the shell", "Light", "Dark"].iter().enumerate() {
        parts.push(boxed(
            Radio::new(format!("bar.mode.{i}"), s.m3_mode == i)
                .label(*name)
                .on_select(move || Lab::write().m3_mode = i)
                .build(),
        ));
    }
    parts.push(boxed(caption(format!(
        "   Shell look: {} · {} (AMANE_THEME, AMANE_DENSITY)   Last press: {}",
        match amane_kit::theme::mode() {
            Mode::Light => "light",
            Mode::Dark => "dark",
        },
        amane_kit::theme::density().theme(),
        s.pressed
    ))));
    Row::new(parts).gap(sys::space::gap::m()).align(Center)
}

fn pressed(what: impl Into<String>) {
    Lab::write().pressed = what.into();
}

// ---- pages ----

fn anatomy() -> Vec<Box<dyn Widget>> {
    let item = |id: &str| {
        vec![
            ListItem::new(format!("{id}.a"), "Headline")
                .overline("Overline")
                .supporting("Supporting text that is long enough to wrap onto a second line")
                .supporting_lines(2)
                .leading(Leading::Avatar {
                    name: String::from("Ada Lovelace"),
                    photo: None,
                })
                .trailing_text("100+")
                .trailing(Trailing::Icon(ICON_STAR))
                .on_press(|| pressed("anatomy")),
            ListItem::new(format!("{id}.b"), "Headline")
                .supporting("Supporting text")
                .leading(Leading::Icon(ICON_PERSON))
                .trailing(Trailing::Checkbox(true))
                .on_toggle(|_| pressed("anatomy checkbox")),
            ListItem::new(format!("{id}.c"), "Headline only")
                .leading(Leading::Image(None))
                .kind(ItemKind::Link)
                .on_press(|| pressed("anatomy link")),
        ]
    };
    let mut out = vec![
        heading("One three-line, two-line and one-line item in each style"),
        boxed(
            Row::new(new_columns("anat", WIDE, item, |l| l))
                .gap(sys::space::gap::l())
                .align(Start),
        ),
        heading("Tokens"),
    ];
    let mut cols = Vec::new();
    for style in ListStyle::ALL {
        let t = style.tokens();
        let lines = [
            format!(
                "Heights 1 / 2 / 3 lines: {} / {} / {}",
                t.heights[0], t.heights[1], t.heights[2]
            ),
            format!(
                "Leading / trailing space: {} / {}",
                t.leading_space, t.trailing_space
            ),
            format!(
                "Vertical padding: {} / {} / {}",
                t.vertical_padding[0], t.vertical_padding[1], t.vertical_padding[2]
            ),
            format!(
                "Leading → text gap: {}, text → trailing: {}",
                t.leading_gap, t.trailing_gap
            ),
            format!("Gap between items: {}", t.item_gap),
            format!("Icon {}, avatar {}, image {}", t.icon, t.avatar, t.image),
            format!(
                "Video small {}×{}, large {}×{}",
                t.video_small.0, t.video_small.1, t.video_large.0, t.video_large.1
            ),
            format!(
                "Headline {:.0}/{:.0} w{}",
                t.headline.style.size, t.headline.height, t.headline.style.weight
            ),
            format!(
                "Supporting {:.0}/{:.0} w{}",
                t.supporting.style.size, t.supporting.height, t.supporting.style.weight
            ),
            format!(
                "Overline, trailing text {:.0}/{:.0} w{}",
                t.overline.style.size, t.overline.height, t.overline.style.weight
            ),
            format!(
                "State layer: hover {:.2}, focus {:.2}, pressed {:.2}, dragged {:.2}",
                t.hover_opacity, t.focus_opacity, t.pressed_opacity, t.dragged_opacity
            ),
            format!("Disabled content: {:.2}", t.disabled_opacity),
            format!(
                "Corners: resting {}, segment ends {}, hovered {}, pressed/selected {}",
                t.shapes.base,
                t.shapes.outer,
                t.shapes.hovered.map_or("—".into(), |r| r.to_string()),
                t.shapes.pressed.map_or("—".into(), |r| r.to_string()),
            ),
            String::from(match style {
                ListStyle::Shell => "Colours: sys.* (state.hover, state.selected, accent)",
                ListStyle::M3 => {
                    "Colours: surface, onSurface, onSurfaceVariant; selected trailing primary"
                }
                ListStyle::M3Expressive => {
                    "Colours: segments of surface on surfaceContainer; selected secondaryContainer; dragged tertiaryContainer"
                }
            }),
        ];
        let rows: Vec<Box<dyn Widget>> = lines.into_iter().map(|l| boxed(caption(l))).collect();
        cols.push(boxed(
            Column::new(rows)
                .gap(sys::space::gap::xs())
                .width(Size::Fixed(WIDE + 2.0 * INSET)),
        ));
    }
    out.push(boxed(Row::new(cols).gap(sys::space::gap::l()).align(Start)));
    out.push(boxed(caption(
        "The M3 styles use Material's sizes and type scale in the shell's font (Atkinson \
         Hyperlegible Next), not Roboto, so glyph widths differ from Material's images.",
    )));
    out
}

fn line_types() -> Vec<Box<dyn Widget>> {
    let items = |id: &str| {
        vec![
            ListItem::new(format!("{id}.1"), "One line")
                .leading(Leading::Icon(ICON_FOLDER))
                .on_press(|| pressed("one line")),
            ListItem::new(format!("{id}.2"), "Two lines")
                .supporting("Supporting text")
                .leading(Leading::Icon(ICON_FOLDER))
                .on_press(|| pressed("two lines")),
            ListItem::new(format!("{id}.3o"), "Three lines")
                .overline("Overline")
                .supporting("Supporting text")
                .leading(Leading::Icon(ICON_FOLDER))
                .on_press(|| pressed("three lines")),
            ListItem::new(format!("{id}.3w"), "Three lines, wrapped")
                .supporting("Supporting text long enough to need a second line")
                .supporting_lines(2)
                .leading(Leading::Icon(ICON_FOLDER))
                .on_press(|| pressed("three lines wrapped")),
        ]
    };
    let old = old(|| {
        vec![
            list::ListItem::new("old.lt.1", "One line")
                .leading(ICON_FOLDER)
                .on_press(|| pressed("old one")),
            list::ListItem::new("old.lt.2", "Two lines")
                .supporting("Supporting text")
                .leading(ICON_FOLDER)
                .on_press(|| pressed("old two")),
        ]
    });
    let form = form(|| {
        vec![
            FormRow::new("form.lt.1", "One line").icon(ICON_FOLDER, Accent::Blue),
            FormRow::new("form.lt.2", "Two lines")
                .icon(ICON_FOLDER, Accent::Blue)
                .subtitle("Supporting text"),
            FormRow::new("form.lt.3", "Three lines, wrapped")
                .icon(ICON_FOLDER, Accent::Blue)
                .subtitle("Supporting text long enough to need a second line"),
        ]
    });
    vec![
        heading("One, two and three lines"),
        five(old, form, items, "lt"),
        boxed(caption(
            "The kit list stops at two lines (no overline, one supporting line). The form wraps its \
             subtitle up to four lines but has no overline. M3 sets each line count's least height \
             (56 / 72 / 88) and top-aligns the leading element on three-line items.",
        )),
    ]
}

fn leading() -> Vec<Box<dyn Widget>> {
    let items = |id: &str| {
        let item = |key: &str, title: &str, leading: Leading| {
            ListItem::new(format!("{id}.{key}"), title.to_string())
                .supporting("Supporting text")
                .leading(leading)
        };
        vec![
            item("icon", "Icon", Leading::Icon(ICON_BLUETOOTH)),
            item(
                "avatar",
                "Avatar",
                Leading::Avatar {
                    name: String::from("Grace Hopper"),
                    photo: None,
                },
            ),
            item("image", "Image", Leading::Image(None)),
            item(
                "vs",
                "Video, small",
                Leading::Video {
                    thumbnail: None,
                    large: false,
                },
            ),
            item(
                "vl",
                "Video, large",
                Leading::Video {
                    thumbnail: None,
                    large: true,
                },
            ),
            item("cb", "Checkbox", Leading::Checkbox(true))
                .on_toggle(|on| pressed(format!("checkbox {on}"))),
            item("rd", "Radio", Leading::Radio(true)).on_toggle(|_| pressed("radio")),
            item("sw", "Switch", Leading::Switch(false))
                .on_toggle(|on| pressed(format!("switch {on}"))),
            item(
                "tile",
                "Tile (shell)",
                Leading::Tile(ICON_WIFI, Accent::Blue),
            ),
        ]
    };
    let old = old(|| {
        vec![
            list::ListItem::new("old.ld.icon", "Icon")
                .supporting("Supporting text")
                .leading(ICON_BLUETOOTH),
            list::ListItem::new("old.ld.av", "Avatar (as a widget)")
                .supporting("Supporting text")
                .leading_widget(Avatar::new("Grace Hopper", list::leading_side()).build()),
            list::ListItem::new("old.ld.tile", "Tile")
                .supporting("Supporting text")
                .leading_tile_accent(ICON_WIFI, Accent::Blue),
        ]
    });
    let form = form(|| {
        vec![
            FormRow::new("form.ld.tile", "Tile")
                .icon(ICON_WIFI, Accent::Blue)
                .subtitle("Supporting text"),
            FormRow::new("form.ld.av", "Avatar (as a widget)")
                .leading(Avatar::new("Grace Hopper", 28.0).build(), 28.0)
                .subtitle("Supporting text"),
        ]
    });
    vec![
        heading("Leading elements"),
        five(old, form, items, "ld"),
        boxed(caption(
            "Images and videos are placeholders until a file loads (Leading::Image(Some(path))). \
             In the M3 styles the whole item toggles its control; in the shell style the kit's own \
             control takes its click.",
        )),
    ]
}

fn trailing() -> Vec<Box<dyn Widget>> {
    let items = |id: &str| {
        let item = |key: &str, title: &str| {
            ListItem::new(format!("{id}.{key}"), title.to_string())
                .leading(Leading::Icon(ICON_SPEAKER))
        };
        vec![
            item("icon", "Icon").trailing(Trailing::Icon(ICON_INFO)),
            item("text", "Supporting text").trailing_text("100+"),
            item("cb", "Checkbox")
                .trailing(Trailing::Checkbox(false))
                .on_toggle(|on| pressed(format!("checkbox {on}"))),
            item("rd", "Radio")
                .trailing(Trailing::Radio(false))
                .on_toggle(|_| pressed("radio")),
            item("sw", "Switch")
                .trailing(Trailing::Switch(true))
                .on_toggle(|on| pressed(format!("switch {on}"))),
            item("link", "Link (chevron)")
                .kind(ItemKind::Link)
                .on_press(|| pressed("link")),
            item("exp", "Expand")
                .trailing(Trailing::Expand(false))
                .on_press(|| pressed("expand")),
            item("drag", "Drag handle").trailing(Trailing::DragHandle),
            item("check", "Chosen (check)")
                .trailing(Trailing::Check)
                .selected(true),
        ]
    };
    let old = old(|| {
        let item = |key: &str, title: &str| {
            list::ListItem::new(format!("old.tr.{key}"), title.to_string()).leading(ICON_SPEAKER)
        };
        vec![
            item("text", "Supporting text").trailing(list::trailing_text(
                "100+",
                material::label(Level::Secondary),
            )),
            item("sw", "Switch").trailing(Switch::new("old.tr.sw.c", true).build()),
            item("check", "Chosen (check)")
                .trailing(list::check())
                .selected(true),
        ]
    });
    let form = form(|| {
        vec![
            FormRow::new("form.tr.value", "Value")
                .icon(ICON_SPEAKER, Accent::Gray)
                .value("100+"),
            FormRow::new("form.tr.sw", "Switch")
                .icon(ICON_SPEAKER, Accent::Gray)
                .trailing(amane_kit::macform::MacSwitch::new("form.tr.sw.c", true).build()),
            FormRow::new("form.tr.link", "Link (chevron)")
                .icon(ICON_SPEAKER, Accent::Gray)
                .on_press(|| pressed("form link")),
        ]
    });
    vec![heading("Trailing elements"), five(old, form, items, "tr")]
}

fn states(s: &Lab) -> Vec<Box<dyn Widget>> {
    let live = s.live;
    let mut out: Vec<Box<dyn Widget>> = vec![boxed(
        Row::new(vec![boxed(
            Checkbox::new("states.live", live)
                .label("Live: drop the forced states and use the pointer")
                .on_toggle(|on| Lab::write().live = on)
                .build(),
        )])
        .gap(sys::space::gap::m()),
    )];

    let cases: [(&str, Option<State>, bool, bool); 9] = [
        ("Enabled", Some(State::Enabled), false, false),
        ("Hovered", Some(State::Hovered), false, false),
        ("Focused", Some(State::Focused), false, false),
        ("Pressed", Some(State::Pressed), false, false),
        ("Dragged", Some(State::Dragged), false, false),
        ("Disabled", Some(State::Enabled), false, true),
        ("Selected", Some(State::Enabled), true, false),
        ("Selected, hovered", Some(State::Hovered), true, false),
        ("Selected, disabled", Some(State::Enabled), true, true),
    ];
    for (n, (name, state, selected, disabled)) in cases.into_iter().enumerate() {
        let items = move |id: &str| {
            let item = ListItem::new(format!("{id}.{n}"), "Headline")
                .supporting("Supporting text")
                .leading(Leading::Icon(ICON_HEADPHONES))
                .trailing(Trailing::Icon(ICON_STAR))
                .selected(selected)
                .disabled(disabled)
                .on_press(|| pressed("state item"));
            vec![match state {
                Some(state) if !live => item.force_state(state),
                _ => item,
            }]
        };
        let old_widget: (Box<dyn Widget>, String) = match name {
            "Enabled" | "Selected" | "Hovered" | "Selected, hovered" => old(move || {
                vec![
                    list::ListItem::new(format!("old.st.{n}"), "Headline")
                        .supporting("Supporting text")
                        .leading(ICON_HEADPHONES)
                        .trailing(icon(
                            ICON_STAR,
                            sys::size::icon::m(),
                            material::label(Level::Secondary),
                        ))
                        .selected(selected)
                        .on_press(|| pressed("old state item")),
                ]
            }),
            _ => (unsupported("no such state"), String::new()),
        };
        let old_widget = if name.contains("Hovered") || name.ends_with("hovered") {
            let (w, _) = old_widget;
            (w, String::from("hover the item: no forced state"))
        } else {
            old_widget
        };
        let form_widget: (Box<dyn Widget>, String) = match name {
            "Focused" | "Pressed" | "Dragged" => (unsupported("no such state"), String::new()),
            _ => {
                let (w, note) = form(move || {
                    vec![
                        FormRow::new(format!("form.st.{n}"), "Headline")
                            .icon(ICON_HEADPHONES, Accent::Indigo)
                            .subtitle("Supporting text")
                            .selected(selected)
                            .disabled(disabled)
                            .on_press(|| pressed("form state item")),
                    ]
                });
                if name.contains("overed") {
                    (w, String::from("hover the row: no forced state"))
                } else {
                    (w, note)
                }
            }
        };
        out.push(heading(name));
        out.push(five(old_widget, form_widget, items, &format!("st{n}")));
    }
    out
}

fn selection(s: &Lab) -> Vec<Box<dyn Widget>> {
    const SOUNDS: [&str; 4] = ["Speakers", "Headphones", "HDMI", "Bluetooth"];
    let single = s.single;
    let multi = s.multi.clone();
    let switches = s.switches.clone();

    let single_items = |id: &str| -> Vec<ListItem> {
        SOUNDS
            .iter()
            .enumerate()
            .map(|(i, name)| {
                ListItem::new(format!("{id}.{i}"), name.to_string())
                    .leading(Leading::Radio(false))
                    .disabled(i == 2)
            })
            .collect()
    };
    let multi_items = |id: &str| -> Vec<ListItem> {
        ["Notifications", "Sounds", "Badges", "Previews"]
            .iter()
            .enumerate()
            .map(|(i, name)| {
                ListItem::new(format!("{id}.{i}"), name.to_string())
                    .supporting("Show on the lock screen")
                    .leading(Leading::Checkbox(false))
            })
            .collect()
    };
    let switch_items = |id: &str| -> Vec<ListItem> {
        [
            (ICON_WIFI, "Wi-Fi"),
            (ICON_BLUETOOTH, "Bluetooth"),
            (ICON_PHONE, "Phone"),
        ]
        .iter()
        .enumerate()
        .map(|(i, (glyph, name))| {
            ListItem::new(format!("{id}.{i}"), name.to_string())
                .leading(Leading::Icon(*glyph))
                .trailing(Trailing::Switch(false))
        })
        .collect()
    };

    let single_sel = Selection::Single(single);
    let multi_sel = Selection::Multi(multi.clone());
    let switch_sel = Selection::Multi(switches.clone());

    let old_single = old(move || {
        SOUNDS
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let item = list::ListItem::new(format!("old.sel.{i}"), name.to_string())
                    .selected(single == Some(i))
                    .on_press(move || Lab::write().single = Some(i));
                if single == Some(i) {
                    item.trailing(list::check())
                } else {
                    item
                }
            })
            .collect()
    });
    let form_switches = form(move || {
        ["Wi-Fi", "Bluetooth", "Phone"]
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let on = switches.contains(&i);
                FormRow::new(format!("form.sel.{i}"), name.to_string()).trailing(
                    amane_kit::macform::MacSwitch::new(format!("form.sel.{i}.c"), on)
                        .on_toggle(move |on| toggle_switch(i, on))
                        .build(),
                )
            })
            .collect()
    });

    let row = |old: (Box<dyn Widget>, String),
               form: (Box<dyn Widget>, String),
               cols: Vec<Box<dyn Widget>>| {
        let mut all = vec![
            column("Current · kit list", old.0, old.1, COL),
            column("Current · Settings form", form.0, form.1, COL),
        ];
        all.extend(cols);
        boxed(Row::new(all).gap(sys::space::gap::l()).align(Start))
    };

    let single_cols = {
        let sel = single_sel.clone();
        new_columns("sel1", W, single_items, move |l| {
            l.select(sel.clone(), |i, _| Lab::write().single = Some(i))
        })
    };
    let multi_cols = {
        let sel = multi_sel.clone();
        new_columns("sel2", W, multi_items, move |l| {
            l.select(sel.clone(), |i, on| {
                let next = Selection::Multi(Lab::read().multi.clone()).toggled(i, on);
                if let Selection::Multi(v) = next {
                    Lab::write().multi = v;
                }
            })
        })
    };
    let switch_cols = {
        let sel = switch_sel.clone();
        new_columns("sel3", W, switch_items, move |l| {
            l.select(sel.clone(), toggle_switch)
        })
    };

    vec![
        heading(format!(
            "Single (radio): {}",
            single.map_or("none".into(), |i| SOUNDS[i].to_string())
        )),
        row(
            old_single,
            (
                unsupported("form rows choose with a pop-up menu instead"),
                String::new(),
            ),
            single_cols,
        ),
        heading(format!("Multiple (checkbox): {multi:?}")),
        row(
            (unsupported("no checkboxes"), String::new()),
            (unsupported("no checkboxes"), String::new()),
            multi_cols,
        ),
        heading(format!("Switches: {:?}", s.switches)),
        row(
            (
                unsupported("switches only as a trailing widget"),
                String::new(),
            ),
            form_switches,
            switch_cols,
        ),
        boxed(caption(
            "HDMI is disabled. In the M3 styles a click anywhere on the item toggles it; with the \
             keyboard (Interaction page) Space toggles the focused one.",
        )),
    ]
}

fn toggle_switch(i: usize, on: bool) {
    let next = Selection::Multi(Lab::read().switches.clone()).toggled(i, on);
    if let Selection::Multi(v) = next {
        Lab::write().switches = v;
    }
}

fn dividers(s: &Lab) -> Vec<Box<dyn Widget>> {
    let items = |id: &str| -> Vec<ListItem> {
        [
            (ICON_MAIL, "Inbox", "12 new"),
            (ICON_STAR, "Starred", "3"),
            (ICON_SEARCH, "Search", ""),
        ]
        .iter()
        .enumerate()
        .map(|(i, (glyph, name, count))| {
            let item = ListItem::new(format!("{id}.{i}"), name.to_string())
                .supporting("Supporting text")
                .leading(Leading::Icon(*glyph))
                .on_press(|| pressed("divider item"));
            if count.is_empty() {
                item
            } else {
                item.trailing_text(*count)
            }
        })
        .collect()
    };
    let mut out = Vec::new();
    for (name, kind) in [
        ("No dividers", Dividers::None),
        ("Full dividers", Dividers::Full),
        ("Inset dividers (from the text)", Dividers::Inset),
        ("Middle-inset dividers", Dividers::Middle),
    ] {
        out.push(heading(name));
        out.push(boxed(
            Row::new(new_columns(&format!("dv{kind:?}"), WIDE, items, move |l| {
                l.dividers(kind)
            }))
            .gap(sys::space::gap::l())
            .align(Start),
        ));
    }

    // sections that open and close
    let open = s.open.clone();
    let sections = move |id: &str| -> Vec<ListItem> {
        let mut items = Vec::new();
        let groups: [(&str, &[&str]); 2] = [
            ("Audio", &["Speakers", "Headphones"]),
            ("Input", &["Keyboard", "Mouse", "Touchpad"]),
        ];
        for (g, (name, children)) in groups.iter().enumerate() {
            let is_open = open.get(g).copied().unwrap_or(false);
            items.push(
                ListItem::new(format!("{id}.g{g}"), name.to_string())
                    .supporting(format!("{} devices", children.len()))
                    .leading(Leading::Icon(if g == 0 {
                        ICON_SPEAKER
                    } else {
                        ICON_MOUSE
                    }))
                    .trailing(Trailing::Expand(is_open))
                    .on_press(move || {
                        let mut lab = Lab::write();
                        if let Some(o) = lab.open.get_mut(g) {
                            *o = !*o;
                        }
                    }),
            );
            if is_open {
                for (c, child) in children.iter().enumerate() {
                    items.push(
                        ListItem::new(format!("{id}.g{g}.{c}"), child.to_string())
                            .leading(Leading::Icon(ICON_CHECK))
                            .on_press(|| pressed("section child")),
                    );
                }
            }
        }
        items
    };
    out.push(heading("Subheader and sections that open and close"));
    out.push(boxed(
        Row::new(new_columns("sect", WIDE, sections, |l| l.header("Devices")))
            .gap(sys::space::gap::l())
            .align(Start),
    ));
    out.push(boxed(caption(
        "The current kit list has a caption header and no dividers; the Settings form draws its own \
         inset hairline between rows (see the Line types page).",
    )));
    out
}

fn interaction(s: &Lab) -> Vec<Box<dyn Widget>> {
    let mut out = vec![heading(
        "Keyboard: Up, Down, Home, End move; Enter or Space presses; Esc lets go",
    )];
    let mut cols = Vec::new();
    for (k, style) in ListStyle::ALL.into_iter().enumerate() {
        let id = format!("kbd.{k}");
        let items: Vec<ListItem> = ["Wi-Fi", "Bluetooth", "Disabled", "Sound", "Display"]
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let name = name.to_string();
                let said = name.clone();
                ListItem::new(format!("{id}.{i}"), name)
                    .leading(Leading::Icon(ICON_SETTINGS))
                    .disabled(i == 2)
                    .on_press(move || pressed(format!("{said} (keyboard list)")))
            })
            .collect();
        let list = List::new(id.clone(), items)
            .style(style)
            .width(WIDE)
            .keyboard(true)
            .build();
        let focus = lists::focused(&id).map_or("none".into(), |i| i.to_string());
        cols.push(column(
            &format!("New · {}", style.name()),
            boxed(on_backdrop(style, list, WIDE)),
            format!("focused: {focus}"),
            WIDE + 2.0 * INSET,
        ));
    }
    out.push(boxed(Row::new(cols).gap(sys::space::gap::l()).align(Start)));

    // reorder
    const NAMES: [&str; 5] = ["Morning", "Commute", "Focus", "Workout", "Evening"];
    let order = s.order.clone();
    out.push(heading(format!(
        "Drag the handles to reorder: {}",
        order
            .iter()
            .map(|&i| NAMES[i])
            .collect::<Vec<_>>()
            .join(", ")
    )));
    let mut cols = Vec::new();
    for (k, style) in ListStyle::ALL.into_iter().enumerate() {
        let items: Vec<ListItem> = order
            .iter()
            .map(|&i| {
                ListItem::new(format!("ro.{k}.{i}"), NAMES[i].to_string())
                    .supporting("Playlist")
                    .leading(Leading::Icon(ICON_SWAP))
            })
            .collect();
        let list = List::new(format!("ro.{k}"), items)
            .style(style)
            .width(WIDE)
            .reorder(|from, to| {
                let mut lab = Lab::write();
                let moved = lab.order.remove(from);
                lab.order.insert(to, moved);
            })
            .build();
        cols.push(column(
            &format!("New · {}", style.name()),
            boxed(on_backdrop(style, list, WIDE)),
            String::new(),
            WIDE + 2.0 * INSET,
        ));
    }
    out.push(boxed(Row::new(cols).gap(sys::space::gap::l()).align(Start)));

    // a long list, only the rows in view built
    const COUNT: usize = 1000;
    const VIEW: f32 = 360.0;
    out.push(heading(format!(
        "{COUNT} items in a kinetic scroll view, only the rows in view built"
    )));
    let mut cols = Vec::new();
    for (k, style) in ListStyle::ALL.into_iter().enumerate() {
        let started = Instant::now();
        let probe = ListItem::new(format!("long.{k}.probe"), "x")
            .supporting("y")
            .render(style, WIDE);
        let gap = style.tokens().item_gap;
        let content = COUNT as f32 * (fixed_h(&probe) + gap) - gap;
        let offset = SCROLLS.with_borrow_mut(|scrolls| {
            let scroll = &mut scrolls[k];
            scroll.set_max((content - VIEW).max(0.0));
            scroll.settle_stale();
            scroll.value()
        });
        let items: Vec<ListItem> = (0..COUNT)
            .map(|i| {
                ListItem::new(format!("long.{k}.{i}"), format!("Item {}", i + 1))
                    .supporting("Supporting text")
                    .leading(Leading::Avatar {
                        name: format!("Person {}", i % 26),
                        photo: None,
                    })
                    .trailing_text(format!("{}", i % 60))
                    .on_press(move || pressed(format!("long item {}", i + 1)))
            })
            .collect();
        let list = List::new(format!("long.{k}"), items)
            .style(style)
            .width(WIDE)
            .window(offset, VIEW)
            .build();
        let view = ScrollView::new(Axis::Vertical, WIDE, VIEW)
            .content(content)
            .offset(offset)
            .scrollbar(1.0)
            .on_scroll(move |scroll: Scroll| {
                SCROLLS.with_borrow_mut(|scrolls| scrolls[k].scroll(&scroll, Axis::Vertical, 72.0));
                request_frame();
            })
            .build(list);
        let micros = started.elapsed().as_micros();
        BUILD_TIMES.with_borrow_mut(|times| times[k] = micros);
        cols.push(column(
            &format!("New · {}", style.name()),
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(WIDE + 2.0 * INSET))
                    .height(Size::Fixed(VIEW + 2.0 * INSET))
                    .fill(match style {
                        ListStyle::Shell => Color::TRANSPARENT,
                        _ => style.tokens().backdrop,
                    })
                    .radius(12.0)
                    .padding(pad(INSET, INSET))
                    .child(view),
            ),
            format!("built in {} µs at {:.0} px", micros, offset),
            WIDE + 2.0 * INSET,
        ));
    }
    out.push(boxed(Row::new(cols).gap(sys::space::gap::l()).align(Start)));
    out
}

// ---- today's lists, copied as they're built in place ----

/// Messages' inbox row, as `apps/messages/src/inbox.rs` builds it.
fn inbox_row(name: &str, time: &str, snippet: &str, unread: bool, width: f32) -> Rectangle {
    let label = sys::r#type::label();
    let mut name_style = label;
    if unread {
        name_style.weight = 700;
    }
    let text = Column::new(vec![
        boxed(
            Row::new(vec![
                boxed(
                    Rectangle::new()
                        .width(Size::Fixed(width - 36.0 - 48.0 - 40.0))
                        .height(Size::Fixed(18.0))
                        .fill(Color::TRANSPARENT)
                        .align_child(Start, Center)
                        .clip()
                        .child(
                            Text::new(name)
                                .style(name_style)
                                .color(material::label(Level::Primary))
                                .elide(),
                        ),
                ),
                boxed(
                    Text::new(time)
                        .style(sys::r#type::caption())
                        .color(if unread {
                            sys::color::accent::bg()
                        } else {
                            material::label(Level::Secondary)
                        }),
                ),
            ])
            .gap(4.0),
        ),
        boxed(
            Rectangle::new()
                .width(Size::Fixed(width - 36.0 - 48.0))
                .height(Size::Fixed(16.0))
                .fill(Color::TRANSPARENT)
                .align_child(Start, Center)
                .clip()
                .child(caption(snippet).elide()),
        ),
    ]);
    let dot = Rectangle::new()
        .width(Size::Fixed(8.0))
        .height(Size::Fixed(8.0))
        .radius(Radius::Full)
        .fill(if unread {
            sys::color::accent::bg()
        } else {
            Color::TRANSPARENT
        });
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(52.0))
        .radius(sys::radius::raised())
        .fill(Color::TRANSPARENT)
        .padding(pad(sys::space::inset::s(), 0.0))
        .align_child(Start, Center)
        .child(
            Row::new(vec![
                boxed(dot),
                boxed(Avatar::new(name, 36.0).build()),
                boxed(text),
            ])
            .gap(sys::space::gap::s())
            .align(Center),
        )
}

/// The launcher's result row, as `src/surfaces/launcher/results.rs` builds it.
fn launcher_row(title: &str, subtitle: &str, chosen: bool, width: f32) -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(52.0))
        .radius(sys::radius::popover())
        .fill(if chosen {
            sys::color::state::selected()
        } else {
            Color::TRANSPARENT
        })
        .padding(pad(sys::space::inset::l(), 0.0))
        .align_child(Start, Center)
        .child(
            Row::new(vec![
                boxed(
                    Rectangle::new()
                        .width(Size::Fixed(40.0))
                        .height(Size::Fixed(40.0))
                        .radius(10.0)
                        .fill(Accent::Blue.color())
                        .align_child(Center, Center)
                        .child(icon(ICON_FOLDER, 24.0, Color::WHITE)),
                ),
                boxed(Column::new(vec![
                    boxed(
                        Text::new(title)
                            .style(sys::r#type::title())
                            .color(material::label(Level::Primary)),
                    ),
                    boxed(caption(subtitle)),
                ])),
            ])
            .gap(sys::space::gap::xl())
            .align(Center),
        )
}

fn current(s: &Lab) -> Vec<Box<dyn Widget>> {
    let mut out = Vec::new();
    let pair = |title: &str,
                current: Box<dyn Widget>,
                note: &str,
                items: fn(&str, Option<usize>) -> Vec<ListItem>,
                chosen: Option<usize>| {
        let mut cols = vec![column(title, current, note.to_string(), COL + 60.0)];
        cols.extend(new_columns(
            &format!("cur.{title}"),
            W + 40.0,
            move |id| items(id, chosen),
            |l| l,
        ));
        boxed(Row::new(cols).gap(sys::space::gap::l()).align(Start))
    };

    // Quick Settings' Bluetooth list
    let single = s.single;
    let devices: [(char, &str, &str); 3] = [
        (ICON_HEADPHONES, "WH-1000XM5", "80%"),
        (ICON_MOUSE, "MX Master 3S", "55%"),
        (ICON_KEYBOARD, "Keychron K3", ""),
    ];
    let old_bt = list::with_row_width(COL + 60.0, || {
        list::List::new(
            devices
                .iter()
                .enumerate()
                .map(|(i, (glyph, name, battery))| {
                    let connected = single == Some(i);
                    let mut item = list::ListItem::new(format!("cur.bt.{i}"), name.to_string())
                        .leading(*glyph)
                        .selected(connected)
                        .on_press(move || Lab::write().single = Some(i));
                    if !battery.is_empty() {
                        item = item.trailing(list::trailing_text(
                            *battery,
                            material::label(Level::Secondary),
                        ));
                    }
                    if connected {
                        item = item.trailing(list::check());
                    }
                    item
                })
                .collect(),
        )
        .header("Devices")
        .build()
    });
    fn bt_items(id: &str, chosen: Option<usize>) -> Vec<ListItem> {
        let devices: [(char, &str, &str); 3] = [
            (ICON_HEADPHONES, "WH-1000XM5", "80%"),
            (ICON_MOUSE, "MX Master 3S", "55%"),
            (ICON_KEYBOARD, "Keychron K3", ""),
        ];
        devices
            .iter()
            .enumerate()
            .map(|(i, (glyph, name, battery))| {
                let connected = chosen == Some(i);
                let mut item = ListItem::new(format!("{id}.{i}"), name.to_string())
                    .supporting(if connected {
                        "Connected"
                    } else {
                        "Not connected"
                    })
                    .leading(Leading::Icon(*glyph))
                    .selected(connected)
                    .on_press(move || Lab::write().single = Some(i));
                if !battery.is_empty() {
                    item = item.trailing_text(*battery);
                }
                if connected {
                    item = item.trailing(Trailing::Check);
                }
                item
            })
            .collect()
    }
    out.push(heading("Quick Settings · Bluetooth devices"));
    out.push(pair(
        "Today · kit list (tiles/bluetooth.rs)",
        boxed(Column::new(vec![boxed(old_bt)]).width(Size::Fixed(COL + 60.0))),
        "a header caption, glyph, battery and check",
        bt_items,
        single,
    ));

    // Messages' inbox
    let inbox = Column::new(vec![
        boxed(inbox_row(
            "Ada Lovelace",
            "09:41",
            "The engine is ready for the first program",
            true,
            COL + 60.0,
        )),
        boxed(inbox_row(
            "Grace Hopper",
            "Tue",
            "Found the bug, it was a moth",
            false,
            COL + 60.0,
        )),
        boxed(inbox_row(
            "+47 123 45 678",
            "Mon",
            "Your code is 1234",
            false,
            COL + 60.0,
        )),
    ])
    .gap(4.0);
    fn inbox_items(id: &str, _: Option<usize>) -> Vec<ListItem> {
        [
            (
                "Ada Lovelace",
                "09:41",
                "The engine is ready for the first program",
            ),
            ("Grace Hopper", "Tue", "Found the bug, it was a moth"),
            ("+47 123 45 678", "Mon", "Your code is 1234"),
        ]
        .iter()
        .enumerate()
        .map(|(i, (name, time, snippet))| {
            ListItem::new(format!("{id}.{i}"), name.to_string())
                .supporting(snippet.to_string())
                .leading(Leading::Avatar {
                    name: name.to_string(),
                    photo: None,
                })
                .trailing_text(*time)
                .on_press(|| pressed("conversation"))
        })
        .collect()
    }
    out.push(heading("Messages · inbox"));
    out.push(pair(
        "Today · by hand (messages/inbox.rs)",
        boxed(inbox),
        "copied here: 52 tall, unread dot, avatar 36",
        inbox_items,
        None,
    ));

    // the launcher's results
    let launcher = Column::new(vec![
        boxed(launcher_row("Files", "Browse your files", true, COL + 60.0)),
        boxed(launcher_row("Firefox", "Web browser", false, COL + 60.0)),
    ])
    .gap(4.0);
    fn launcher_items(id: &str, _: Option<usize>) -> Vec<ListItem> {
        [("Files", "Browse your files"), ("Firefox", "Web browser")]
            .iter()
            .enumerate()
            .map(|(i, (name, sub))| {
                ListItem::new(format!("{id}.{i}"), name.to_string())
                    .supporting(sub.to_string())
                    .leading(Leading::Tile(ICON_FOLDER, Accent::Blue))
                    .selected(i == 0)
                    .on_press(|| pressed("launcher result"))
            })
            .collect()
    }
    out.push(heading("Launcher · results"));
    out.push(pair(
        "Today · by hand (launcher/results.rs)",
        boxed(launcher),
        "copied here: 52 tall, 40 px icon, one sliding highlight",
        launcher_items,
        None,
    ));

    // Settings' Apps page
    let apps = Group::new()
        .width(COL + 60.0)
        .row(
            FormRow::new("cur.apps.0", "GIMP")
                .icon(ICON_IMAGE, Accent::Orange)
                .subtitle("GNU Image Manipulation Program")
                .trailing(
                    PushButton::new("cur.apps.0.add", "Add")
                        .on_press(|| pressed("add gimp"))
                        .build(),
                )
                .on_press(|| pressed("gimp details")),
        )
        .row(
            FormRow::new("cur.apps.1", "Inkscape")
                .icon(ICON_IMAGE, Accent::Purple)
                .subtitle("Vector graphics editor")
                .trailing(
                    PushButton::new("cur.apps.1.add", "Add")
                        .on_press(|| pressed("add inkscape"))
                        .build(),
                )
                .on_press(|| pressed("inkscape details")),
        )
        .build();
    fn app_items(id: &str, _: Option<usize>) -> Vec<ListItem> {
        [
            ("GIMP", "GNU Image Manipulation Program", Accent::Orange),
            ("Inkscape", "Vector graphics editor", Accent::Purple),
        ]
        .iter()
        .enumerate()
        .map(|(i, (name, sub, accent))| {
            let key = format!("{id}.{i}.add");
            let said = name.to_string();
            ListItem::new(format!("{id}.{i}"), name.to_string())
                .supporting(sub.to_string())
                .leading(Leading::Tile(ICON_IMAGE, *accent))
                .trailing(Trailing::Widget(boxed(
                    PushButton::new(key, "Add")
                        .on_press(move || pressed(format!("add {said}")))
                        .build(),
                )))
                .on_press(|| pressed("app details"))
        })
        .collect()
    }
    out.push(heading("Shell Settings · Apps"));
    out.push(pair(
        "Today · Settings form (system/apps.rs)",
        boxed(apps),
        "a FormRow with a tile, subtitle and Add button",
        app_items,
        None,
    ));
    out.push(boxed(caption(
        "Menus (the shell's context menu and the kit's pop-up menus) are left out: they are 24 px \
         rows with shortcuts and submenus, closer to M3's menu component than to its list.",
    )));
    out
}
