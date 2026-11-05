//! The macform controls next to the kit's own, to compare and try:
//! `cargo run -p amane-kit --example macform`.
//!
//! Each row shows one kind of control twice: the kit's on the left,
//! the macform port on the right, sharing the same state, with what it
//! holds written after it. `AMANE_THEME=light|dark` and
//! `AMANE_DENSITY=compact` show the other looks.

use amane::{
    Center, Column, Rectangle, Row, ScrollArea, Service, Size, Start, Text, Widget, Window,
};
use amane_kit::app::{Page, app, app_window, field_box, sidebar, standard_shortcut, text_field};
use amane_kit::buttons::{
    Checkbox, PushButton, Radio, Segment, SegmentedButton, Shape, Style, Switch,
};
use amane_kit::dropdown::Dropdown;
use amane_kit::form::{FormRow, Group, SliderRow};
use amane_kit::icons::{
    ICON_BLUETOOTH, ICON_DARK_MODE, ICON_DISPLAY, ICON_FOLDER, ICON_NOTIFICATIONS, ICON_SEARCH,
    ICON_SETTINGS, ICON_TUNE, ICON_VOLUME_UP, ICON_WIFI,
};
use amane_kit::layout::boxed;
use amane_kit::macform::theme::{self as mac, Accent};
use amane_kit::macform::{
    MacComboBox, MacLeadingIcon, MacPopUpButton, MacPushButton, MacSegmentedNav, MacSlider,
    MacStepper, MacSwitch, MacTextField, MenuItem, Symbol, symbol,
};
use amane_kit::material::{self, Level};
use amane_kit::slider::Slider;
use amane_kit::tokens::{Styled, sys};

struct Lab {
    page: usize,
    wifi: bool,
    bluetooth: bool,
    sounds: bool,
    badges: bool,
    size: usize,
    level: f32,
    accent: usize,
    sort: usize,
    font: String,
    port: f64,
    scale: f64,
    pages_back: usize,
    pressed: String,
    name: String,
    port_text: String,
}

impl Service for Lab {
    fn new() -> Self {
        Self {
            // MACFORM_PAGE=5 opens on a page, for screenshots
            page: std::env::var("MACFORM_PAGE")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(0),
            wifi: true,
            bluetooth: false,
            sounds: true,
            badges: false,
            size: 1,
            level: 0.4,
            accent: 0,
            sort: 0,
            font: String::new(),
            port: 8080.0,
            scale: 1.5,
            pages_back: 2,
            pressed: String::from("nothing yet"),
            name: String::new(),
            port_text: String::new(),
        }
    }

    fn listen() {}
}

const SIZES: [&str; 3] = ["Small", "Medium", "Large"];
const ACCENTS: [&str; 4] = ["Blue", "Purple", "Green", "Orange"];
const FONTS: [&str; 5] = [
    "Atkinson Hyperlegible",
    "Inter",
    "Iosevka",
    "JetBrains Mono",
    "Noto Sans",
];

fn main() {
    app("amane-macform-gallery")
        .popup(amane_kit::tooltip::popup)
        .normal_window("macform", window)
        .run();
}

fn window() -> Window {
    let state = Lab::read();

    let pages = [
        Page::new(ICON_TUNE, "Toggles"),
        Page::new(ICON_VOLUME_UP, "Sliders"),
        Page::new(ICON_NOTIFICATIONS, "Buttons"),
        Page::new(ICON_FOLDER, "Selectors"),
        Page::new(ICON_DARK_MODE, "Text"),
        Page::new(ICON_SETTINGS, "Settings rows"),
    ];

    let nav = sidebar("macform.nav", &pages, state.page, |page| {
        Lab::write().page = page
    });

    let page: Vec<Box<dyn Widget>> = match state.page {
        0 => toggles(&state),
        1 => sliders(&state),
        2 => buttons(&state),
        3 => selectors(&state),
        4 => text(&state),
        _ => settings_rows(&state),
    };

    let title = pages[state.page.min(pages.len() - 1)].name.clone();
    let mut body: Vec<Box<dyn Widget>> = vec![boxed(
        Text::new(title)
            .style(mac::page_title_font())
            .color(mac::label_primary()),
    )];
    if state.page < 5 {
        body.push(boxed(header()));
    }
    body.extend(page);

    let content = Row::new(vec![
        boxed(Column::new(vec![boxed(nav)]).width(Size::Fixed(180.0))),
        boxed(ScrollArea::new(
            "macform.page",
            Column::new(body)
                .gap(sys::space::gap::m())
                .width(Size::Parent),
        )),
    ])
    .gap(sys::space::gap::l())
    .width(Size::Parent)
    .height(Size::Parent);

    app_window(content)
        .title("macform gallery")
        .size(960.0, 640.0)
        .min_size(820.0, 520.0)
        .on_shortcut(standard_shortcut)
}

// ---- layout ----

const LABEL_W: f32 = 100.0;
const CELL_W: f32 = 250.0;
const ROW_H: f32 = 36.0;

fn caption(text: impl Into<String>) -> Text {
    Text::new(text)
        .style(sys::r#type::caption())
        .color(material::label(Level::Secondary))
}

fn cell(width: f32, height: f32, child: impl Widget + 'static) -> Box<dyn Widget> {
    boxed(
        Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height))
            .align_child(Start, Center)
            .child(child),
    )
}

fn header() -> Row {
    Row::new(vec![
        cell(LABEL_W, 20.0, caption("")),
        cell(CELL_W, 20.0, caption("KIT TODAY")),
        cell(CELL_W, 20.0, caption("MACFORM")),
    ])
}

/// One comparison: a name, the kit's control and the port, and a readout.
fn compare(
    name: &str,
    height: f32,
    kit: impl Widget + 'static,
    port: impl Widget + 'static,
    readout: String,
) -> Box<dyn Widget> {
    let line = Row::new(vec![
        cell(LABEL_W, height, caption(name)),
        cell(CELL_W, height, kit),
        cell(CELL_W, height, port),
    ]);
    if readout.is_empty() {
        return boxed(line);
    }
    boxed(Column::new(vec![
        boxed(line),
        boxed(Row::new(vec![
            cell(LABEL_W, 14.0, caption("")),
            cell(CELL_W * 2.0, 14.0, caption(readout)),
        ])),
    ]))
}

fn row(parts: Vec<Box<dyn Widget>>) -> Row {
    Row::new(parts).gap(sys::space::gap::m()).align(Center)
}

fn column(parts: Vec<Box<dyn Widget>>) -> Column {
    Column::new(parts).gap(sys::space::gap::xs())
}

// ---- pages ----

fn toggles(s: &Lab) -> Vec<Box<dyn Widget>> {
    let mut kit_radios = Vec::new();
    let mut mac_radios = Vec::new();
    for (i, size) in SIZES.iter().enumerate() {
        kit_radios.push(boxed(
            Radio::new(format!("kit.size.{i}"), s.size == i)
                .label(*size)
                .on_select(move || Lab::write().size = i)
                .build(),
        ));
        mac_radios.push(boxed(
            Radio::new(format!("mac.size.{i}"), s.size == i)
                .label(*size)
                .on_select(move || Lab::write().size = i)
                .build(),
        ));
    }

    vec![
        compare(
            "Switch",
            ROW_H,
            row(vec![
                boxed(
                    Switch::new("kit.wifi", s.wifi)
                        .on_toggle(|on| Lab::write().wifi = on)
                        .build(),
                ),
                boxed(
                    Switch::new("kit.bt", s.bluetooth)
                        .on_toggle(|on| Lab::write().bluetooth = on)
                        .build(),
                ),
                boxed(Switch::new("kit.off", true).disabled(true).build()),
            ]),
            row(vec![
                boxed(
                    MacSwitch::new("mac.wifi", s.wifi)
                        .on_toggle(|on| Lab::write().wifi = on)
                        .build(),
                ),
                boxed(
                    MacSwitch::new("mac.bt", s.bluetooth)
                        .on_toggle(|on| Lab::write().bluetooth = on)
                        .build(),
                ),
                boxed(MacSwitch::new("mac.off", true).disabled(true).build()),
            ]),
            format!(
                "wifi {}, bluetooth {} (third is disabled)",
                s.wifi, s.bluetooth
            ),
        ),
        compare(
            "Checkbox",
            ROW_H * 2.0,
            column(vec![
                boxed(
                    Checkbox::new("kit.sounds", s.sounds)
                        .label("Play sounds")
                        .on_toggle(|on| Lab::write().sounds = on)
                        .build(),
                ),
                boxed(
                    Checkbox::new("kit.badges", s.badges)
                        .label("Show badges")
                        .on_toggle(|on| Lab::write().badges = on)
                        .build(),
                ),
            ]),
            column(vec![
                boxed(
                    Checkbox::new("mac.sounds", s.sounds)
                        .label("Play sounds")
                        .on_toggle(|on| Lab::write().sounds = on)
                        .build(),
                ),
                boxed(
                    Checkbox::new("mac.badges", s.badges)
                        .label("Show badges")
                        .on_toggle(|on| Lab::write().badges = on)
                        .build(),
                ),
            ]),
            format!("sounds {}, badges {}", s.sounds, s.badges),
        ),
        compare(
            "Radio",
            ROW_H * 3.0,
            column(kit_radios),
            column(mac_radios),
            format!("size {}", SIZES[s.size]),
        ),
    ]
}

fn sliders(s: &Lab) -> Vec<Box<dyn Widget>> {
    let percent = (s.level * 100.0).round() as u8;
    vec![
        compare(
            "Slider",
            ROW_H,
            Slider::new(percent, 200.0)
                .on_value_change(|v| Lab::write().level = f32::from(v) / 100.0)
                .build(),
            MacSlider::new(s.level, 200.0)
                .on_change(|v| Lab::write().level = v)
                .build(),
            format!("level {:.3} (drag, click or scroll either)", s.level),
        ),
        compare(
            "Disabled",
            ROW_H,
            Slider::new(percent, 200.0).inert(true).dimmed(true).build(),
            MacSlider::new(s.level, 200.0).disabled(true).build(),
            String::new(),
        ),
        compare(
            "Large (kit)",
            ROW_H,
            Slider::new(percent, 200.0)
                .large(ICON_VOLUME_UP)
                .on_value_change(|v| Lab::write().level = f32::from(v) / 100.0)
                .build(),
            caption("(no macform counterpart)"),
            String::new(),
        ),
    ]
}

fn buttons(s: &Lab) -> Vec<Box<dyn Widget>> {
    let press = |what: &'static str| move || Lab::write().pressed = what.to_string();

    let segments = vec![
        Segment::new("Day"),
        Segment::new("Week"),
        Segment::new("Month"),
    ];

    let tiles = Accent::ALL
        .iter()
        .enumerate()
        .map(|(i, accent)| {
            let glyphs = [ICON_WIFI, ICON_BLUETOOTH, ICON_DISPLAY, ICON_VOLUME_UP];
            boxed(
                MacLeadingIcon::new(glyphs[i % glyphs.len()])
                    .accent(*accent)
                    .build(),
            )
        })
        .collect();

    let symbols = [
        Symbol::ChevronDown,
        Symbol::ChevronUp,
        Symbol::ChevronLeft,
        Symbol::ChevronRight,
        Symbol::ChevronUpDown,
        Symbol::Checkmark,
        Symbol::Ellipsis,
        Symbol::InfoCircle,
    ]
    .into_iter()
    .map(|sym| boxed(symbol(sym, 16.0, mac::label_primary())))
    .collect();

    vec![
        compare(
            "Push",
            ROW_H,
            row(vec![
                boxed(
                    PushButton::new("kit.ok", "OK")
                        .style(Style::Filled)
                        .on_press(press("kit OK"))
                        .build(),
                ),
                boxed(
                    PushButton::new("kit.cancel", "Cancel")
                        .style(Style::Tonal)
                        .on_press(press("kit Cancel"))
                        .build(),
                ),
                boxed(
                    PushButton::new("kit.search", "Search")
                        .icon(ICON_SEARCH)
                        .style(Style::Tonal)
                        .shape(Shape::Round)
                        .on_press(press("kit Search"))
                        .build(),
                ),
            ]),
            row(vec![
                boxed(
                    MacPushButton::new("mac.ok", "OK")
                        .prominent(true)
                        .on_press(press("macform OK"))
                        .build(),
                ),
                boxed(
                    MacPushButton::new("mac.cancel", "Cancel")
                        .on_press(press("macform Cancel"))
                        .build(),
                ),
                boxed(
                    MacPushButton::new("mac.search", "Search")
                        .icon(ICON_SEARCH)
                        .on_press(press("macform Search"))
                        .build(),
                ),
            ]),
            format!("last pressed: {}", s.pressed),
        ),
        compare(
            "Disabled",
            ROW_H,
            row(vec![
                boxed(PushButton::new("kit.dis", "Apply").disabled(true).build()),
                boxed(
                    PushButton::new("kit.dis2", "Apply")
                        .style(Style::Filled)
                        .disabled(true)
                        .build(),
                ),
            ]),
            row(vec![
                boxed(
                    MacPushButton::new("mac.dis", "Apply")
                        .disabled(true)
                        .build(),
                ),
                boxed(
                    MacPushButton::new("mac.dis2", "Apply")
                        .prominent(true)
                        .disabled(true)
                        .build(),
                ),
            ]),
            String::new(),
        ),
        compare(
            "Navigation",
            ROW_H + 4.0,
            SegmentedButton::new("kit.range", segments, Some(s.pages_back.min(2)))
                .on_select(|i, _| Lab::write().pages_back = i)
                .build(),
            MacSegmentedNav::new("mac.nav")
                .back_enabled(s.pages_back > 0)
                .forward_enabled(s.pages_back < 2)
                .on_back(|| {
                    let mut lab = Lab::write();
                    lab.pages_back = lab.pages_back.saturating_sub(1);
                })
                .on_forward(|| {
                    let mut lab = Lab::write();
                    lab.pages_back = (lab.pages_back + 1).min(2);
                })
                .build(),
            format!(
                "position {} of 3 (segmented button on the kit side, back/forward pill on macform)",
                s.pages_back + 1
            ),
        ),
        compare(
            "Leading icons",
            ROW_H,
            caption("(no kit counterpart)"),
            Row::new(tiles).gap(4.0).align(Center),
            String::new(),
        ),
        compare(
            "Symbols",
            ROW_H,
            caption("(Material Symbols font)"),
            Row::new(symbols).gap(10.0).align(Center),
            String::from("drawn as round-joined strokes, no font"),
        ),
    ]
}

fn selectors(s: &Lab) -> Vec<Box<dyn Widget>> {
    let accents: Vec<MenuItem> = ACCENTS.iter().map(|a| MenuItem::from(*a)).collect();
    let sorts = vec![
        MenuItem::new("Name").icon(ICON_TUNE),
        MenuItem::new("Date modified").icon(ICON_FOLDER),
        MenuItem::new("Size").icon(ICON_DISPLAY),
    ];
    let fonts: Vec<MenuItem> = FONTS.iter().map(|f| MenuItem::from(*f)).collect();

    vec![
        compare(
            "Pop-up / pull-down",
            ROW_H,
            Dropdown::new(
                "kit.accent",
                ACCENTS.iter().map(|a| a.to_string()).collect(),
                s.accent,
            )
            .on_select(|i| Lab::write().accent = i)
            .build(),
            Dropdown::items("mac.accent", accents, Some(s.accent))
                .on_select(|i| Lab::write().accent = i)
                .build(),
            format!(
                "accent {} (both are the kit's Dropdown now)",
                ACCENTS[s.accent]
            ),
        ),
        compare(
            "With icons",
            ROW_H,
            caption(""),
            Dropdown::items("mac.sort", sorts.clone(), Some(s.sort))
                .width(150.0)
                .on_select(|i| Lab::write().sort = i)
                .build(),
            format!(
                "sort by {} (Up, Down, Enter and Escape work while open)",
                sorts[s.sort].text
            ),
        ),
        compare(
            "Pop-up button",
            ROW_H,
            caption(""),
            row(vec![
                boxed(
                    Text::new(sorts[s.sort].text.clone())
                        .style(mac::control_font())
                        .color(mac::label_primary()),
                ),
                boxed(
                    MacPopUpButton::new("mac.popup")
                        .items(sorts, Some(s.sort))
                        .on_select(|i| Lab::write().sort = i)
                        .build(),
                ),
            ]),
            String::new(),
        ),
        compare(
            "Combo box",
            ROW_H,
            caption("(none)"),
            MacComboBox::new("mac.font", fonts)
                .width(200.0)
                .placeholder("Font")
                .on_change(|f| Lab::write().font = f)
                .build(),
            format!(
                "font {:?} (type to narrow the menu, or pick from the chevron)",
                s.font
            ),
        ),
    ]
}

fn text(s: &Lab) -> Vec<Box<dyn Widget>> {
    vec![
        compare(
            "Text field",
            ROW_H,
            field_box(
                "kit.name",
                text_field("kit.name", "Your name").on_change(|t| Lab::write().name = t),
                200.0,
            ),
            MacTextField::new("mac.name")
                .placeholder("Your name")
                .width(200.0)
                .on_change(|t| Lab::write().name = t)
                .build(),
            format!("kit field holds {:?}", s.name),
        ),
        compare(
            "Validated",
            ROW_H,
            caption("(no check in the kit)"),
            MacTextField::new("mac.port")
                .placeholder("Port, digits only")
                .width(200.0)
                .validate(|t| t.chars().all(|c| c.is_ascii_digit()))
                .on_submit(|t| Lab::write().port_text = t)
                .build(),
            format!(
                "press Enter: letters turn the edge red; last accepted {:?}",
                s.port_text
            ),
        ),
        compare(
            "Password",
            ROW_H,
            field_box("kit.pw", text_field("kit.pw", "Password").password(), 200.0),
            MacTextField::new("mac.pw")
                .placeholder("Password")
                .password()
                .width(200.0)
                .build(),
            String::new(),
        ),
        compare(
            "Stepper",
            ROW_H,
            caption("(none)"),
            row(vec![
                boxed(
                    MacStepper::new("mac.port.step", s.port)
                        .range(1.0, 65535.0)
                        .width(90.0)
                        .on_change(|v| Lab::write().port = v)
                        .build(),
                ),
                boxed(
                    MacStepper::new("mac.scale.step", s.scale)
                        .range(0.5, 3.0)
                        .step(0.25)
                        .on_change(|v| Lab::write().scale = v)
                        .build(),
                ),
                boxed(MacStepper::new("mac.dis.step", 3.0).disabled(true).build()),
            ]),
            format!(
                "port {}, scale {} (hold a half to repeat, scroll, or type and Enter)",
                s.port, s.scale
            ),
        ),
    ]
}

/// macOS Settings-style rows, from the kit's `form`.
fn settings_rows(s: &Lab) -> Vec<Box<dyn Widget>> {
    let width = 500.0;
    let accents: Vec<MenuItem> = ACCENTS.iter().map(|a| MenuItem::from(*a)).collect();

    let network = Group::new()
        .width(width)
        .header("Network")
        .row(
            FormRow::new("rows.wifi", "Wi-Fi")
                .icon(ICON_WIFI, Accent::Blue)
                .subtitle(if s.wifi { "Connected to Home" } else { "Off" })
                .trailing(
                    MacSwitch::new("rows.wifi.switch", s.wifi)
                        .on_toggle(|on| Lab::write().wifi = on)
                        .build(),
                ),
        )
        .row(
            FormRow::new("rows.bt", "Bluetooth")
                .icon(ICON_BLUETOOTH, Accent::Indigo)
                .trailing(
                    MacSwitch::new("rows.bt.switch", s.bluetooth)
                        .on_toggle(|on| Lab::write().bluetooth = on)
                        .build(),
                ),
        )
        .row(
            FormRow::new("rows.more", "Network details")
                .icon(ICON_SETTINGS, Accent::Gray)
                .on_press(|| Lab::write().pressed = String::from("Network details")),
        )
        .footer("Turning Wi-Fi off also stops the shell from looking for networks nearby.");

    let mut radios = Vec::new();
    for (i, size) in SIZES.iter().enumerate() {
        radios.push(boxed(
            Radio::new(format!("rows.size.{i}"), s.size == i)
                .label(*size)
                .on_select(move || Lab::write().size = i)
                .build(),
        ));
    }

    let look = Group::new()
        .width(width)
        .header("Look")
        .row(
            FormRow::new("rows.accent", "Accent colour")
                .icon(ICON_DARK_MODE, Accent::Purple)
                .trailing(
                    Dropdown::items("rows.accent.menu", accents, Some(s.accent))
                        .on_select(|i| Lab::write().accent = i)
                        .build(),
                ),
        )
        .row(
            FormRow::new("rows.scale", "Display scale")
                .icon(ICON_DISPLAY, Accent::Teal)
                .trailing(
                    MacStepper::new("rows.scale.step", s.scale)
                        .range(0.5, 3.0)
                        .step(0.25)
                        .on_change(|v| Lab::write().scale = v)
                        .build(),
                ),
        )
        .row(
            FormRow::new("rows.size", "Control size")
                .icon(ICON_TUNE, Accent::Orange)
                .detail(Column::new(radios), 72.0),
        );

    let level = s.level;
    let sound = Group::new()
        .width(width)
        .slider(
            SliderRow::new("Output volume", move |w| {
                MacSlider::new(level, w)
                    .on_change(|v| Lab::write().level = v)
                    .build()
            })
            .glyph(ICON_VOLUME_UP)
            .value(format!("{:.0}%", s.level * 100.0)),
        )
        .row(
            FormRow::new("rows.reset", "Reset notifications")
                .icon(ICON_NOTIFICATIONS, Accent::Red)
                .subtitle("Shows every app's first notification again")
                .trailing(
                    MacPushButton::new("rows.reset.button", "Reset…")
                        .on_press(|| Lab::write().pressed = String::from("Reset"))
                        .build(),
                ),
        );

    vec![
        boxed(
            MacSegmentedNav::new("rows.nav")
                .back_enabled(true)
                .forward_enabled(false)
                .build(),
        ),
        boxed(network.build()),
        boxed(look.build()),
        boxed(sound.build()),
        boxed(caption(format!("last pressed: {}", s.pressed))),
    ]
}
