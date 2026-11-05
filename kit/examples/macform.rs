//! The macform controls next to the kit's own, to compare and try:
//! `cargo run -p amane-kit --example macform`.
//!
//! Each row shows one kind of control twice: the kit's today on the left,
//! the macform port on the right, sharing the same state, with what it
//! holds written after it. `AMANE_THEME=light|dark` and
//! `AMANE_DENSITY=compact` show the other looks.

use amane::{Center, Column, Rectangle, Row, Service, Size, Start, Text, Widget, Window};
use amane_kit::app::{Page, app, app_window, field_box, sidebar, standard_shortcut, text_field};
use amane_kit::buttons::{
    Checkbox, PushButton, Radio, Segment, SegmentedButton, Shape, Style, Switch,
};
use amane_kit::dropdown::Dropdown;
use amane_kit::icons::{
    ICON_BLUETOOTH, ICON_DARK_MODE, ICON_DISPLAY, ICON_FOLDER, ICON_NOTIFICATIONS, ICON_SEARCH,
    ICON_SETTINGS, ICON_TUNE, ICON_VOLUME_UP, ICON_WIFI,
};
use amane_kit::layout::boxed;
use amane_kit::macform::theme::{self as mac, Accent};
use amane_kit::macform::{
    MacCheckbox, MacComboBox, MacLeadingIcon, MacPopUpButton, MacPullDown, MacPushButton,
    MacRadioButton, MacSegmentedNav, MacSlider, MacStepper, MacSwitch, MacTextField, MenuItem,
    Symbol, symbol,
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
            page: 0,
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
        boxed(
            Column::new(body)
                .gap(sys::space::gap::m())
                .width(Size::Parent),
        ),
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
            MacRadioButton::new(format!("mac.size.{i}"), s.size == i)
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
                    MacCheckbox::new("mac.sounds", s.sounds)
                        .label("Play sounds")
                        .on_toggle(|on| Lab::write().sounds = on)
                        .build(),
                ),
                boxed(
                    MacCheckbox::new("mac.badges", s.badges)
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
            MacPullDown::new("mac.accent", accents, Some(s.accent))
                .on_select(|i| Lab::write().accent = i)
                .build(),
            format!(
                "accent {} (kit menu opens at the pointer, macform under the button)",
                ACCENTS[s.accent]
            ),
        ),
        compare(
            "With icons",
            ROW_H,
            caption(""),
            MacPullDown::new("mac.sort", sorts.clone(), Some(s.sort))
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

/// macOS Settings-style rows built from the ports, as the old nexus pages
/// composed them.
fn settings_rows(s: &Lab) -> Vec<Box<dyn Widget>> {
    let width = 500.0;
    let divider = || {
        boxed(
            Rectangle::new()
                .width(Size::Parent)
                .height(Size::Fixed(1.0))
                .fill(mac::divider()),
        )
    };
    let line = |glyph: char, accent: Accent, title: &str, control: Box<dyn Widget>| {
        boxed(
            Rectangle::new()
                .width(Size::Parent)
                .height(Size::Fixed(mac::row_min_height()))
                .padding(amane_kit::layout::pad(mac::row_pad_h(), 0.0))
                .align_child(Start, Center)
                .child(amane_kit::layout::split_row(
                    Row::new(vec![
                        boxed(MacLeadingIcon::new(glyph).accent(accent).build()),
                        boxed(
                            Text::new(title)
                                .style(mac::control_font())
                                .color(mac::label_primary()),
                        ),
                    ])
                    .gap(10.0)
                    .align(Center),
                    vec![control],
                )),
        )
    };

    let group = |rows: Vec<Box<dyn Widget>>, height: f32| {
        boxed(
            Rectangle::new()
                .width(Size::Fixed(width))
                .height(Size::Fixed(height))
                .radius(mac::group_radius())
                .fill(mac::group_fill())
                .clip()
                .child(Column::new(rows).width(Size::Parent)),
        )
    };

    let accents: Vec<MenuItem> = ACCENTS.iter().map(|a| MenuItem::from(*a)).collect();
    let rh = mac::row_min_height();

    // the SliderRow: a glyph, then the label and value over a slider
    let slider_row = boxed(
        Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(76.0))
            .radius(mac::group_radius())
            .fill(mac::group_fill())
            .padding(amane::Padding {
                top: 14.0,
                bottom: 10.0,
                left: 20.0,
                right: 20.0,
            })
            .child(
                Row::new(vec![
                    boxed(amane_kit::layout::icon(
                        ICON_VOLUME_UP,
                        18.0,
                        mac::label_secondary(),
                    )),
                    boxed(
                        Column::new(vec![
                            boxed(amane_kit::layout::split_row(
                                Text::new("Output volume")
                                    .style(mac::control_font())
                                    .color(mac::label_primary()),
                                vec![boxed(
                                    Text::new(format!("{:.0}%", s.level * 100.0))
                                        .style(mac::control_font())
                                        .color(mac::label_secondary()),
                                )],
                            )),
                            boxed(
                                MacSlider::new(s.level, width - 40.0 - 30.0)
                                    .on_change(|v| Lab::write().level = v)
                                    .build(),
                            ),
                        ])
                        .gap(8.0)
                        .width(Size::Parent),
                    ),
                ])
                .gap(12.0),
            ),
    );

    vec![
        boxed(
            MacSegmentedNav::new("rows.nav")
                .back_enabled(true)
                .forward_enabled(false)
                .build(),
        ),
        group(
            vec![
                line(
                    ICON_WIFI,
                    Accent::Blue,
                    "Wi-Fi",
                    boxed(
                        MacSwitch::new("rows.wifi", s.wifi)
                            .on_toggle(|on| Lab::write().wifi = on)
                            .build(),
                    ),
                ),
                divider(),
                line(
                    ICON_BLUETOOTH,
                    Accent::Indigo,
                    "Bluetooth",
                    boxed(
                        MacSwitch::new("rows.bt", s.bluetooth)
                            .on_toggle(|on| Lab::write().bluetooth = on)
                            .build(),
                    ),
                ),
                divider(),
                line(
                    ICON_DARK_MODE,
                    Accent::Purple,
                    "Accent colour",
                    boxed(
                        MacPullDown::new("rows.accent", accents, Some(s.accent))
                            .on_select(|i| Lab::write().accent = i)
                            .build(),
                    ),
                ),
                divider(),
                line(
                    ICON_DISPLAY,
                    Accent::Teal,
                    "Display scale",
                    boxed(
                        MacStepper::new("rows.scale", s.scale)
                            .range(0.5, 3.0)
                            .step(0.25)
                            .on_change(|v| Lab::write().scale = v)
                            .build(),
                    ),
                ),
            ],
            rh * 4.0 + 3.0,
        ),
        slider_row,
        group(
            vec![line(
                ICON_NOTIFICATIONS,
                Accent::Red,
                "Reset notifications",
                boxed(
                    MacPushButton::new("rows.reset", "Reset…")
                        .on_press(|| Lab::write().pressed = String::from("Reset"))
                        .build(),
                ),
            )],
            rh,
        ),
    ]
}
