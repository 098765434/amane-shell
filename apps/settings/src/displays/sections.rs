//! The page's sections, top to bottom, each a function of the screen
//! shown ([`Ctx`]). A section that doesn't apply to it (brightness on a
//! monitor, OLED care on an LCD) is left out.

use std::cell::Cell;
use std::thread;

use amane::{
    Brightness, Center, Color, Column, Rectangle, Row, Service, Size, Text, TextInput, Widget,
};
use amane_kit::app::{Dialog, field_box, text_field};
use amane_kit::arrangement::{Arrangement, Screen};
use amane_kit::buttons::{PushButton, Segment, SegmentedButton, Style, Switch};
use amane_kit::card::Card;
use amane_kit::dropdown::Dropdown;
use amane_kit::layout::{boxed, text_height};
use amane_kit::list::{List, ListItem, with_row_width};
use amane_kit::material::{self, Level};
use amane_kit::settings::{appearance, change_appearance, change_dock, dock};
use amane_kit::slider::Slider;
use amane_kit::tag::{Tag, tags};
use amane_kit::theme::Mode as Scheme;
use amane_kit::tokens::{Styled, sys};

use super::caps::Caps;
use super::config::{self, Displays as Saved, Setup, Vrr};
use super::niri::{self, Change, Mode, Output, Transform};
use super::trial::try_change;
use super::{Screens, block_name, save, save_all, saved_block};
use crate::{PAGE_WIDTH, labelled, note, setting};

/// What every section reads.
pub struct Ctx {
    pub output: Output,
    pub outputs: Vec<Output>,
    pub caps: Caps,
    /// The saved file; none until `config.kdl`'s blocks are moved, when
    /// nothing can be changed.
    pub saved: Option<Saved>,
}

impl Ctx {
    fn editable(&self) -> bool {
        self.saved.is_some()
    }

    /// The variable refresh the file sets, else what niri shows.
    fn vrr(&self) -> Vrr {
        self.saved
            .as_ref()
            .and_then(|saved| saved_block(saved, &self.output))
            .map(|block| block.vrr)
            .unwrap_or(if self.output.vrr_enabled {
                Vrr::Always
            } else {
                Vrr::Off
            })
    }

    fn name(&self) -> String {
        display_name(&self.output)
    }
}

fn display_name(output: &Output) -> String {
    if output.built_in() {
        String::from("Built-in Display")
    } else {
        format!("{} {}", output.make, output.model.trim())
    }
}

fn hz(rate: f64) -> String {
    if (rate - rate.round()).abs() < 0.05 {
        format!("{} Hz", rate.round())
    } else {
        format!("{rate:.2} Hz")
    }
}

// ───── moving the config's own blocks ───────────────────────────────────────

pub fn setup_banner(setup: &Setup) -> Option<Box<dyn Widget>> {
    match setup {
        Setup::Ready(_) => None,
        Setup::Missing => Some(boxed(note(
            "niri's config.kdl wasn't found, so changes here can't be kept.",
        ))),
        Setup::Unmoved(blocks) => {
            let names: Vec<String> = blocks.iter().map(|b| b.output.name.clone()).collect();
            let what = if names.is_empty() {
                String::from("niri's config.kdl sets no displays yet.")
            } else {
                format!("niri's config.kdl sets {} itself.", names.join(", "))
            };
            Some(boxed(
                Column::new(vec![
                    boxed(note(format!(
                        "{what} Move the display settings to Shell Settings to change them here; \
                         until then this page only shows them."
                    ))),
                    boxed(
                        PushButton::new("settings.displays.move", "Move to Settings…")
                            .on_press(confirm_move)
                            .build(),
                    ),
                ])
                .gap(sys::space::gap::s()),
            ))
        }
    }
}

fn confirm_move() {
    let file = config::displays_file();
    Dialog::new(
        "settings.displays.move-dialog",
        "Move display settings to Settings?",
    )
    .message(format!(
        "The output blocks in {} become comments, and an include of {} takes their \
             place with the same settings, as a commit of its own.",
        config::niri_config().display(),
        file.display()
    ))
    .button("Cancel", || {})
    .main_button("Move", || {
        thread::spawn(|| {
            if let Err(error) = config::migrate() {
                Screens::fail(error);
            }
            Screens::refresh();
        });
    })
    .open("main");
}

pub fn error_line(error: String) -> Text {
    Text::new(error)
        .style(sys::r#type::caption())
        .color(sys::color::status::critical::text())
        .wrap()
}

// ───── which screen ─────────────────────────────────────────────────────────

/// The screens laid out to drag, with two or more, and a picker that
/// also reaches the ones switched off.
pub fn arrangement(ctx: &Ctx) -> Vec<Box<dyn Widget>> {
    if ctx.outputs.len() < 2 {
        return Vec::new();
    }
    let placed: Vec<&Output> = ctx.outputs.iter().filter(|o| o.logical.is_some()).collect();
    let screens: Vec<Screen> = placed
        .iter()
        .map(|output| {
            let logical = output.logical.unwrap_or_else(|| unreachable!());
            let label = if output.built_in() {
                String::from("Built-in")
            } else {
                output.model.trim().to_string()
            };
            Screen::new(
                label,
                logical.x,
                logical.y,
                logical.width as i32,
                logical.height as i32,
            )
        })
        .collect();
    let selected = placed.iter().position(|o| o.name == ctx.output.name);
    let names: Vec<String> = placed.iter().map(|o| o.name.clone()).collect();
    let moved_names = names.clone();
    let movable: Vec<Output> = placed.iter().map(|o| (*o).clone()).collect();
    let editable = ctx.editable();

    let area = Arrangement::new("settings.displays.arrangement", screens, PAGE_WIDTH, 180.0)
        .selected(selected)
        .on_select(move |index| {
            if let Some(name) = names.get(index) {
                Screens::write().selected = Some(name.clone());
            }
        })
        .on_move(move |index, x, y| {
            let (Some(output), Some(name)) = (movable.get(index), moved_names.get(index)) else {
                return;
            };
            if !editable {
                return;
            }
            let (output, name) = (output.clone(), name.clone());
            let what = format!("{name} at {x}, {y}");
            thread::spawn(move || {
                if let Err(error) = niri::apply(&name, &Change::Position(x, y)) {
                    Screens::fail(error);
                }
            });
            save(&output, what, move |block| {
                block.position = Some((x, y));
            });
        })
        .build();

    let labels: Vec<String> = ctx
        .outputs
        .iter()
        .map(|o| format!("{} ({})", display_name(o), o.name))
        .collect();
    let all: Vec<String> = ctx.outputs.iter().map(|o| o.name.clone()).collect();
    let at = all.iter().position(|n| *n == ctx.output.name).unwrap_or(0);
    let picker = Dropdown::new("settings.displays.picker", labels, at)
        .width(280.0)
        .on_select(move |index| {
            if let Some(name) = all.get(index) {
                Screens::write().selected = Some(name.clone());
            }
        })
        .build();

    vec![
        boxed(setting(
            "Arrangement",
            Column::new(vec![
                boxed(area),
                boxed(note(
                    "Drag a display to where it sits beside the others; it snaps to their edges.",
                )),
            ])
            .gap(sys::space::gap::xs()),
        )),
        boxed(setting("Display", picker)),
    ]
}

// ───── what it is ───────────────────────────────────────────────────────────

/// The screen's facts as tags, found out from it.
fn facts(output: &Output, caps: &Caps) -> Vec<Tag> {
    let mut found = Vec::new();
    if caps.oled() {
        found.push(Tag::new("OLED").accent(true));
    }
    if caps.hdr10() {
        found.push(Tag::new(match caps.peak_nits() {
            Some(nits) => format!("HDR10 · {} nits", nits.round()),
            None => String::from("HDR10"),
        }));
    }
    if output.vrr_supported {
        let kind = if caps.freesync() {
            "FreeSync"
        } else {
            "Variable refresh"
        };
        found.push(Tag::new(match caps.refresh_range() {
            Some((low, high)) => format!("{kind} {low}–{high} Hz"),
            None => kind.to_string(),
        }));
    }
    let fastest = output.modes.iter().map(|m| m.hz()).fold(0.0, f64::max);
    if fastest > 0.0 {
        found.push(Tag::new(format!("Up to {}", hz(fastest))));
    }
    if let Some(gamut) = caps.gamut().filter(|g| *g != super::edid::Gamut::Srgb) {
        found.push(Tag::new(
            format!("{} color", gamut.label()).replace("color color", "color"),
        ));
    }
    if let Some(bits) = caps.bits().filter(|bits| *bits >= 10) {
        found.push(Tag::new(format!("{bits}-bit")));
    }
    found
}

pub fn header(ctx: &Ctx) -> Box<dyn Widget> {
    let output = &ctx.output;
    let mut details = Vec::new();
    if let Some(inches) = output.inches() {
        details.push(format!("{inches:.1}″"));
    }
    if let Some(edid) = &ctx.caps.edid
        && let Some(name) = &edid.name
        && output.built_in()
    {
        details.push(format!("{} {name}", output.make));
    }
    details.push(output.name.clone());
    match output.mode() {
        Some(mode) => details.push(format!(
            "{} × {} at {}",
            mode.width,
            mode.height,
            hz(mode.hz())
        )),
        None => details.push(String::from("Off")),
    }

    let padding = sys::space::inset::l();
    let inner = PAGE_WIDTH - padding * 2.0;
    let (facts, facts_h) = tags(inner, facts(output, &ctx.caps));
    let gap = sys::space::gap::s();
    let height = padding * 2.0
        + text_height(sys::r#type::title())
        + gap
        + text_height(sys::r#type::caption())
        + if facts_h > 0.0 { gap + facts_h } else { 0.0 };

    let card = Card::new(vec![boxed(
        Column::new(vec![
            boxed(
                Text::new(ctx.name())
                    .style(sys::r#type::title())
                    .color(material::label(Level::Primary)),
            ),
            boxed(note(details.join(" · "))),
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(inner))
                    .height(Size::Fixed(facts_h))
                    .fill(Color::TRANSPARENT)
                    .child(facts),
            ),
        ])
        .gap(gap),
    )])
    .padding(padding)
    .build();

    // a card fills its parent
    boxed(
        Rectangle::new()
            .width(Size::Fixed(PAGE_WIDTH))
            .height(Size::Fixed(height))
            .fill(Color::TRANSPARENT)
            .child(card),
    )
}

// ───── its picture ──────────────────────────────────────────────────────────

pub fn brightness(ctx: &Ctx) -> Option<Box<dyn Widget>> {
    if !ctx.output.built_in() {
        return None;
    }
    let light = Brightness::read();
    if !light.present() {
        return None;
    }
    let percent = light.percent();
    drop(light);
    Some(boxed(setting(
        "Brightness",
        Row::new(vec![
            boxed(
                Slider::new(percent, 240.0)
                    .wheel(false)
                    .on_value_change(Brightness::set)
                    .build(),
            ),
            boxed(note(format!("{percent} %"))),
        ])
        .gap(sys::space::gap::m())
        .align(Center),
    )))
}

/// The resolutions it lists, largest first, each once.
fn sizes(output: &Output) -> Vec<(u16, u16)> {
    let mut sizes: Vec<(u16, u16)> = Vec::new();
    for mode in &output.modes {
        if !sizes.contains(&(mode.width, mode.height)) {
            sizes.push((mode.width, mode.height));
        }
    }
    sizes.sort_by_key(|(w, h)| std::cmp::Reverse(u32::from(*w) * u32::from(*h)));
    sizes
}

/// The modes at `size`, slowest first.
fn rates(output: &Output, size: (u16, u16)) -> Vec<Mode> {
    let mut rates: Vec<Mode> = output
        .modes
        .iter()
        .copied()
        .filter(|m| (m.width, m.height) == size)
        .collect();
    rates.sort_by_key(|m| m.refresh_rate);
    rates.dedup_by_key(|m| m.refresh_rate);
    rates
}

fn mode_change(output: &Output, to: Mode) {
    let Some(from) = output.mode() else {
        return;
    };
    let name = output.name.clone();
    try_change(
        output,
        Change::Mode(to),
        Change::Mode(from),
        format!("{name} at {}x{}@{}", to.width, to.height, hz(to.hz())),
        move |block| {
            block.mode = Some((to.width, to.height, to.hz()));
            block.custom = false;
        },
    );
}

pub fn resolution(ctx: &Ctx) -> Box<dyn Widget> {
    let output = ctx.output.clone();
    let sizes = sizes(&output);
    let preferred = output
        .modes
        .iter()
        .find(|m| m.is_preferred)
        .map(|m| (m.width, m.height));
    let labels: Vec<String> = sizes
        .iter()
        .map(|&(w, h)| {
            if Some((w, h)) == preferred {
                format!("{w} × {h} (Default)")
            } else {
                format!("{w} × {h}")
            }
        })
        .collect();
    let current = output.mode().map(|m| (m.width, m.height));
    let at = sizes.iter().position(|s| Some(*s) == current).unwrap_or(0);

    let looks = output
        .logical
        .map(|l| format!("Looks like {} × {}.", l.width, l.height));
    let ctx_output = output.clone();
    let dropdown = Dropdown::new("settings.displays.resolution", labels, at)
        .width(240.0)
        .disabled(!ctx.editable() || !output.on())
        .on_select(move |index| {
            let Some(&size) = sizes.get(index) else {
                return;
            };
            let Some(now) = ctx_output.mode() else {
                return;
            };
            // the same refresh rate there if it has it, else the fastest
            let there = rates(&ctx_output, size);
            let Some(to) = there
                .iter()
                .find(|m| m.refresh_rate == now.refresh_rate)
                .or_else(|| there.last())
                .copied()
            else {
                return;
            };
            if to != now {
                mode_change(&ctx_output, to);
            }
        })
        .build();

    let mut column: Vec<Box<dyn Widget>> = vec![boxed(dropdown)];
    if let Some(looks) = looks {
        column.push(boxed(note(looks)));
    }
    boxed(setting(
        "Resolution",
        Column::new(column).gap(sys::space::gap::xs()),
    ))
}

pub fn refresh_rate(ctx: &Ctx) -> Option<Box<dyn Widget>> {
    let now = ctx.output.mode()?;
    let rates = rates(&ctx.output, (now.width, now.height));
    if rates.len() < 2 {
        return Some(boxed(setting("Refresh rate", note(hz(now.hz())))));
    }
    let at = rates
        .iter()
        .position(|m| m.refresh_rate == now.refresh_rate);
    let labels: Vec<String> = rates.iter().map(|m| hz(m.hz())).collect();
    let output = ctx.output.clone();
    let pick = move |index: usize| {
        if let Some(to) = rates.get(index).copied()
            && to != now
        {
            mode_change(&output, to);
        }
    };
    let control: Rectangle = if !ctx.editable() {
        Rectangle::new()
            .width(Size::Fixed(240.0))
            .height(Size::Fixed(text_height(sys::r#type::caption())))
            .fill(Color::TRANSPARENT)
            .child(note(hz(now.hz())))
    } else if labels.len() <= 5 {
        SegmentedButton::new(
            "settings.displays.refresh",
            labels.into_iter().map(Segment::new).collect(),
            at,
        )
        .on_select(move |index, _| pick(index))
        .build()
    } else {
        Dropdown::new("settings.displays.refresh", labels, at.unwrap_or(0))
            .width(240.0)
            .on_select(pick)
            .build()
    };
    Some(boxed(setting("Refresh rate", control)))
}

const VRR_APP: &str = "settings.displays.vrr-app";

pub fn variable_refresh(ctx: &Ctx) -> Option<Box<dyn Widget>> {
    if !ctx.output.vrr_supported {
        return Some(boxed(setting(
            "Variable refresh rate",
            note("This display can't vary its refresh rate."),
        )));
    }
    let now = ctx.vrr();
    let output = ctx.output.clone();
    let choices = [Vrr::Off, Vrr::OnDemand, Vrr::Always];
    let control = SegmentedButton::new(
        "settings.displays.vrr",
        ["Off", "On demand", "Always"]
            .into_iter()
            .map(|label| {
                let segment = Segment::new(label);
                if ctx.editable() {
                    segment
                } else {
                    segment.disabled()
                }
            })
            .collect(),
        choices.iter().position(|c| *c == now),
    )
    .on_select(move |index, _| {
        let vrr = choices[index];
        let (name, change) = (
            output.name.clone(),
            Change::Vrr {
                on: vrr != Vrr::Off,
                on_demand: vrr == Vrr::OnDemand,
            },
        );
        thread::spawn(move || {
            if let Err(error) = niri::apply(&name, &change) {
                Screens::fail(error);
            }
        });
        let label = match vrr {
            Vrr::Off => "off",
            Vrr::OnDemand => "on demand",
            Vrr::Always => "always",
        };
        save(
            &output,
            format!("{} variable refresh {label}", output.name),
            move |block| block.vrr = vrr,
        );
    })
    .build();

    let range = match ctx.caps.refresh_range() {
        Some((low, high)) => format!("It follows a game or video between {low} and {high} Hz. "),
        None => String::new(),
    };
    let flicker = if ctx.caps.oled() {
        "OLED panels can flicker at low frame rates while it's on, so On demand suits them best."
    } else {
        "Some panels flicker at low frame rates while it's on."
    };
    let mut column: Vec<Box<dyn Widget>> = vec![
        boxed(control),
        boxed(note(format!(
            "{range}On demand turns it on only while one of the apps below shows. {flicker}"
        ))),
    ];
    if now == Vrr::OnDemand {
        column.push(vrr_apps(ctx));
    }
    Some(boxed(setting(
        "Variable refresh rate",
        Column::new(column).gap(sys::space::gap::xs()),
    )))
}

/// The apps that turn on-demand variable refresh on, with a field to add
/// one by its app id.
fn vrr_apps(ctx: &Ctx) -> Box<dyn Widget> {
    let apps = ctx
        .saved
        .as_ref()
        .map(|saved| saved.vrr_apps.clone())
        .unwrap_or_default();
    let items: Vec<ListItem> = apps
        .iter()
        .map(|app| {
            let gone = app.clone();
            ListItem::new(format!("settings.displays.vrr-app.{app}"), app.clone())
                .bare(true)
                .trailing(
                    PushButton::new(format!("settings.displays.vrr-remove.{app}"), "Remove")
                        .style(Style::Plain)
                        .on_press(move || {
                            let gone = gone.clone();
                            save_all(
                                format!("on-demand variable refresh without {gone}"),
                                move |saved| {
                                    saved.vrr_apps.retain(|app| *app != gone);
                                },
                            );
                        })
                        .build(),
                )
        })
        .collect();
    let list = with_row_width(PAGE_WIDTH, || {
        List::new(items)
            .empty("No apps yet: add one by its app id (niri msg windows shows them).")
            .build()
    });

    let add = |text: &str| {
        let app = text.trim().to_string();
        if app.is_empty() {
            return;
        }
        TextInput::set_text(VRR_APP, "");
        save_all(
            format!("on-demand variable refresh for {app}"),
            move |saved| {
                if !saved.vrr_apps.contains(&app) {
                    saved.vrr_apps.push(app);
                }
            },
        );
    };
    let field =
        text_field(VRR_APP, "App id, like steam_app_1091500").on_submit(move |text| add(&text));
    let row = Row::new(vec![
        boxed(field_box(VRR_APP, field, 280.0)),
        boxed(
            PushButton::new("settings.displays.vrr-add", "Add")
                .on_press(move || add(&TextInput::text(VRR_APP)))
                .build(),
        ),
    ])
    .gap(sys::space::gap::s())
    .align(Center);

    boxed(Column::new(vec![boxed(list), boxed(row)]).gap(sys::space::gap::s()))
}

/// The scales offered, as macOS's "Looks like" choices.
const SCALES: [f64; 9] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0];

pub fn scale(ctx: &Ctx) -> Box<dyn Widget> {
    let output = ctx.output.clone();
    let Some(logical) = output.logical else {
        return boxed(setting("Scale", note("Off")));
    };
    let mode = output.mode();
    // the physical size a scale divides, turned as shown
    let (pw, ph) = match mode {
        Some(m) if logical.transform.sideways() => (f64::from(m.height), f64::from(m.width)),
        Some(m) => (f64::from(m.width), f64::from(m.height)),
        None => (f64::from(logical.width), f64::from(logical.height)),
    };
    let mut scales: Vec<f64> = SCALES
        .iter()
        .copied()
        .filter(|s| pw / s >= 640.0 && ph / s >= 480.0)
        .collect();
    let custom = !scales.iter().any(|s| (s - logical.scale).abs() < 0.001);
    if custom {
        scales.push(logical.scale);
        scales.sort_by(f64::total_cmp);
    }
    let labels: Vec<String> = scales
        .iter()
        .map(|s| {
            let percent = (s * 100.0).round();
            format!(
                "{percent} % · looks like {} × {}",
                (pw / s).round(),
                (ph / s).round()
            )
        })
        .collect();
    let at = scales
        .iter()
        .position(|s| (s - logical.scale).abs() < 0.001)
        .unwrap_or(0);
    let from = logical.scale;
    let dropdown = Dropdown::new("settings.displays.scale", labels, at)
        .width(300.0)
        .disabled(!ctx.editable())
        .on_select(move |index| {
            if let Some(&to) = scales.get(index)
                && (to - from).abs() > 0.001
            {
                scale_change(&output, from, to);
            }
        })
        .build();
    boxed(setting(
        "Scale",
        Column::new(vec![
            boxed(dropdown),
            boxed(note(
                "Larger makes text and controls bigger, with less room for windows.",
            )),
        ])
        .gap(sys::space::gap::xs()),
    ))
}

fn scale_change(output: &Output, from: f64, to: f64) {
    try_change(
        output,
        Change::Scale(to),
        Change::Scale(from),
        format!("{} scale {to}", output.name),
        move |block| block.scale = Some(to),
    );
}

pub fn rotation(ctx: &Ctx) -> Box<dyn Widget> {
    let output = ctx.output.clone();
    let from = output.logical.map(|l| l.transform).unwrap_or_default();
    let labels: Vec<String> = Transform::ALL
        .iter()
        .map(|t| t.label().to_string())
        .collect();
    let at = Transform::ALL.iter().position(|t| *t == from).unwrap_or(0);
    let dropdown = Dropdown::new("settings.displays.rotation", labels, at)
        .width(240.0)
        .disabled(!ctx.editable() || !output.on())
        .on_select(move |index| {
            let to = Transform::ALL[index];
            if to != from {
                try_change(
                    &output,
                    Change::Transform(to),
                    Change::Transform(from),
                    format!("{} turned {}", output.name, to.config()),
                    move |block| block.transform = to,
                );
            }
        })
        .build();
    boxed(setting(
        "Rotation",
        Column::new(vec![
            boxed(dropdown),
            boxed(note("Turned counter-clockwise, as the screen is.")),
        ])
        .gap(sys::space::gap::xs()),
    ))
}

pub fn hdr(ctx: &Ctx) -> Box<dyn Widget> {
    let caps = &ctx.caps;
    let what = if caps.hdr10() {
        let peak = caps.peak_nits().map_or(String::new(), |nits| {
            format!(", up to {} nits", nits.round())
        });
        let wide = if caps.wide_encoding() {
            " in BT.2020 colour"
        } else {
            ""
        };
        format!(
            "This display shows HDR10{peak}{wide}. niri can't send HDR yet, so it stays off \
             and everything shows in standard range."
        )
    } else {
        String::from("This display doesn't support HDR.")
    };
    boxed(setting(
        "High dynamic range",
        Column::new(vec![
            boxed(labelled(
                "Use HDR",
                Switch::new("settings.displays.hdr", false)
                    .disabled(true)
                    .build(),
            )),
            boxed(note(what)),
        ])
        .gap(sys::space::gap::xs()),
    ))
}

// ───── night light ──────────────────────────────────────────────────────────

pub fn night_light() -> Vec<Box<dyn Widget>> {
    super::night::section()
}

// ───── OLED care ────────────────────────────────────────────────────────────

pub fn oled_care(ctx: &Ctx) -> Option<Box<dyn Widget>> {
    if !ctx.caps.oled() {
        return None;
    }
    let dark = appearance().mode == Some(Scheme::Dark);
    let hidden = dock().auto_hide;
    Some(boxed(setting(
        "OLED care",
        Column::new(vec![
            boxed(note(
                "An OLED's pixels wear with what stays lit for hours. Dark colours, a dock that \
                 hides and a screen that sleeps when idle all spare them.",
            )),
            boxed(labelled(
                "Dark appearance",
                Switch::new("settings.displays.oled.dark", dark)
                    .on_toggle(|on| change_appearance(|now| now.mode = on.then_some(Scheme::Dark)))
                    .build(),
            )),
            boxed(labelled(
                "Hide the dock until the pointer meets the screen's edge",
                Switch::new("settings.displays.oled.dock", hidden)
                    .on_toggle(|on| change_dock(|now| now.auto_hide = on))
                    .build(),
            )),
            boxed(
                PushButton::new("settings.displays.oled.sleep", "Screen Sleep Timing…")
                    .on_press(|| crate::go("power"))
                    .build(),
            ),
        ])
        .gap(sys::space::gap::s()),
    )))
}

// ───── more than one ────────────────────────────────────────────────────────

pub fn multiple(ctx: &Ctx) -> Option<Box<dyn Widget>> {
    if ctx.outputs.len() < 2 {
        return None;
    }
    let output = ctx.output.clone();
    let lit = ctx.outputs.iter().filter(|o| o.on()).count();
    let main = ctx
        .saved
        .as_ref()
        .and_then(|saved| saved_block(saved, &output))
        .is_some_and(|block| block.focus_at_startup);
    let editable = ctx.editable();

    let on_output = output.clone();
    let use_it = Switch::new("settings.displays.enabled", output.on())
        // the last screen lit stays lit
        .disabled(!editable || (output.on() && lit < 2))
        .on_toggle(move |on| {
            let (change, undo) = if on {
                (Change::On, Change::Off)
            } else {
                (Change::Off, Change::On)
            };
            try_change(
                &on_output,
                change,
                undo,
                format!("{} {}", on_output.name, if on { "on" } else { "off" }),
                move |block| block.off = !on,
            );
        })
        .build();

    let main_output = output.clone();
    let main_name = block_name(&output);
    let main_switch = Switch::new("settings.displays.main", main)
        .disabled(!editable)
        .on_toggle(move |on| {
            let name = main_name.clone();
            let output = main_output.clone();
            save_all(
                format!(
                    "{} main display {}",
                    output.name,
                    if on { "on" } else { "off" }
                ),
                move |saved| {
                    // one main display at most
                    for block in &mut saved.outputs {
                        block.focus_at_startup = false;
                    }
                    if on {
                        saved.output_mut(&name).focus_at_startup = true;
                    }
                },
            );
        })
        .build();

    Some(boxed(setting(
        "This display",
        Column::new(vec![
            boxed(labelled("Use this display", use_it)),
            boxed(labelled(
                "Main display: niri starts with it focused",
                main_switch,
            )),
        ])
        .gap(sys::space::gap::s()),
    )))
}

// ───── advanced ─────────────────────────────────────────────────────────────

thread_local! {
    static ADVANCED: Cell<bool> = const { Cell::new(false) };
}

const CUSTOM_SCALE: &str = "settings.displays.custom-scale";
const CUSTOM_MODE: &str = "settings.displays.custom-mode";
const BACKDROP: &str = "settings.displays.backdrop";

pub fn advanced(ctx: &Ctx) -> Box<dyn Widget> {
    let open = ADVANCED.get();
    let toggle = PushButton::new(
        "settings.displays.advanced",
        if open { "Hide Advanced" } else { "Advanced…" },
    )
    .style(Style::Plain)
    .on_press(move || ADVANCED.set(!open))
    .build();
    if !open {
        return boxed(toggle);
    }

    let editable = ctx.editable();
    let output = ctx.output.clone();
    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(toggle)];

    // any scale niri takes
    let scale_output = output.clone();
    let from = output.logical.map_or(1.0, |l| l.scale);
    rows.push(boxed(setting(
        "Custom scale",
        field_row(CUSTOM_SCALE, "1.6", editable, move |text| {
            match text.trim().trim_end_matches('%').trim().parse::<f64>() {
                Ok(value) => {
                    // a percentage, or a factor
                    let to = if value > 10.0 { value / 100.0 } else { value };
                    if (0.5..=4.0).contains(&to) {
                        scale_change(&scale_output, from, to);
                    } else {
                        Screens::fail(String::from("A scale goes from 0.5 to 4."));
                    }
                }
                Err(_) => Screens::fail(String::from("A scale is a number, like 1.6.")),
            }
        }),
    )));

    // a mode the monitor doesn't list
    let mode_output = output.clone();
    rows.push(boxed(setting(
        "Custom mode",
        Column::new(vec![
            boxed(field_row(
                CUSTOM_MODE,
                "2560x1600@90",
                editable,
                move |text| {
                    let Some((w, h, rate)) = parse_mode(text) else {
                        Screens::fail(String::from("A mode reads like 2560x1600@90."));
                        return;
                    };
                    let Some(from) = mode_output.mode() else {
                        return;
                    };
                    try_change(
                        &mode_output,
                        Change::CustomMode(w, h, rate),
                        Change::Mode(from),
                        format!("{} custom mode {w}x{h}@{rate}", mode_output.name),
                        move |block| {
                            block.mode = Some((w, h, rate));
                            block.custom = true;
                        },
                    );
                },
            )),
            boxed(note(
                "A mode the display doesn't list can leave it blank; it goes back by itself \
                 unless kept.",
            )),
        ])
        .gap(sys::space::gap::xs()),
    )));

    // behind the workspaces in the overview
    let backdrop_output = output.clone();
    rows.push(boxed(setting(
        "Overview backdrop",
        field_row(BACKDROP, "#1e1e2e", editable, move |text| {
            let color = text.trim().to_string();
            let valid = color.len() == 7
                && color.starts_with('#')
                && color[1..].chars().all(|c| c.is_ascii_hexdigit());
            if !valid && !color.is_empty() {
                Screens::fail(String::from("A colour reads like #1e1e2e."));
                return;
            }
            save(
                &backdrop_output,
                format!("{} backdrop {color}", backdrop_output.name),
                move |block| block.backdrop = (!color.is_empty()).then_some(color),
            );
        }),
    )));

    // what was read from the screen itself
    rows.push(boxed(setting(
        "Read from the display",
        Column::new(
            raw_facts(ctx)
                .into_iter()
                .map(|line| boxed(note(line)))
                .collect(),
        )
        .gap(sys::space::gap::_2xs()),
    )));

    boxed(Column::new(rows).gap(sys::space::gap::l()))
}

/// A text field and its Set button.
fn field_row(
    id: &'static str,
    placeholder: &str,
    editable: bool,
    set: impl Fn(&str) + Clone + 'static,
) -> Row {
    let submit = set.clone();
    let field = text_field(id, placeholder).on_submit(move |text| submit(&text));
    Row::new(vec![
        boxed(field_box(id, field, 200.0)),
        boxed(
            PushButton::new(format!("{id}.set"), "Set")
                .disabled(!editable)
                .on_press(move || set(&TextInput::text(id)))
                .build(),
        ),
    ])
    .gap(sys::space::gap::s())
    .align(Center)
}

fn parse_mode(text: &str) -> Option<(u16, u16, f64)> {
    let (size, rate) = text.trim().split_once('@')?;
    let (w, h) = size.split_once(['x', '×'])?;
    Some((
        w.trim().parse().ok()?,
        h.trim().parse().ok()?,
        rate.trim().trim_end_matches("Hz").trim().parse().ok()?,
    ))
}

fn raw_facts(ctx: &Ctx) -> Vec<String> {
    let mut lines = Vec::new();
    let output = &ctx.output;
    lines.push(format!(
        "niri: {} {}{}, {} modes",
        output.make,
        output.model.trim(),
        output
            .serial
            .as_deref()
            .map_or(String::new(), |serial| format!(", serial {serial}")),
        output.modes.len()
    ));
    if let Some(edid) = &ctx.caps.edid {
        let mut parts = vec![format!("EDID: {}", edid.vendor)];
        if let Some(name) = &edid.name {
            parts.push(name.clone());
        }
        if let (Some(bits), Some(interface)) = (edid.bits, edid.interface) {
            parts.push(format!("{bits}-bit {interface}"));
        }
        if !edid.colorimetry.is_empty() {
            parts.push(edid.colorimetry.join(", "));
        }
        if let Some(hdr) = &edid.hdr {
            let mut curves = vec!["SDR"];
            if hdr.pq {
                curves.push("PQ");
            }
            if hdr.hlg {
                curves.push("HLG");
            }
            parts.push(curves.join("/"));
            if let (Some(avg), Some(min)) = (hdr.average, hdr.min) {
                parts.push(format!("{} nits full frame, {min:.4} black", avg.round()));
            }
        }
        lines.push(parts.join(" · "));
    }
    if let Some(connector) = &ctx.caps.connector {
        let mut parts = vec![String::from("Kernel:")];
        if let Some(panel) = &connector.panel {
            parts.push(format!("{panel} panel"));
        }
        if let Some(bpc) = connector.max_bpc {
            parts.push(format!("up to {bpc} bpc"));
        }
        if connector.vrr_capable == Some(true) {
            parts.push(String::from("VRR capable"));
        }
        if !connector.colorspaces.is_empty() {
            parts.push(connector.colorspaces.join(", "));
        }
        lines.push(parts.join(" "));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_modes_read_loosely() {
        assert_eq!(parse_mode("2560x1600@90"), Some((2560, 1600, 90.0)));
        assert_eq!(
            parse_mode(" 1920 × 1080 @ 59.94 Hz"),
            Some((1920, 1080, 59.94))
        );
        assert_eq!(parse_mode("1920x1080"), None);
    }

    #[test]
    fn rates_print_whole_when_whole() {
        assert_eq!(hz(120.0), "120 Hz");
        assert_eq!(hz(59.997), "60 Hz");
        assert_eq!(hz(59.94), "59.94 Hz");
    }
}
