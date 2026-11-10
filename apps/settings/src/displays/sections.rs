//! The page's sections, top to bottom, each a function of the screen
//! shown ([`Ctx`]). A section that doesn't apply to it (brightness on a
//! monitor, OLED care on an LCD) is left out.

use std::thread;

use amane::{Battery, Brightness, Center, Rectangle, Row, Service, Size, TextInput};
use emaki_kit::app::{Dialog, field_box, text_field};
use emaki_kit::arrangement::{Arrangement, Screen};
use emaki_kit::buttons::{IconButton, PushButton, Switch};
use emaki_kit::color::Accent;
use emaki_kit::dropdown::Dropdown;
use emaki_kit::form::{FormRow, Group, SliderRow};
use emaki_kit::icons::{ICON_BRIGHTNESS_HIGH, ICON_BRIGHTNESS_LOW, ICON_DISPLAY, ICON_INFO};
use emaki_kit::layout::{boxed, frame};
use emaki_kit::leading_icon::LeadingIcon;
use emaki_kit::settings::{
    BatteryRefresh, appearance, change_appearance, change_displays, change_dock, change_status_bar,
    displays, dock, output_key, status_bar,
};
use emaki_kit::slider::Slider;
use emaki_kit::style;
use emaki_kit::tag::{Tag, tags};
use emaki_kit::theme::Mode as Scheme;
use emaki_kit::tokens::sys;

use super::caps::Caps;
use super::ddc::{self, Ddc, Feature, Monitor};
use super::kdl::{self, Displays as Saved, OutputConfig, Setup, Vrr};
use super::outputs::{self, Change, Mode, Output, OutputExt, Transform, TransformExt};
use super::trial::try_change;
use super::{Screens, block_name, save, save_all, saved_block};
use crate::{group, page_width, switch_row};

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

    pub fn name(&self) -> String {
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

pub fn setup_banner(setup: &Setup) -> Option<Group> {
    match setup {
        Setup::Ready(_) => None,
        Setup::Missing => Some(
            group().row(
                FormRow::new("settings.displays.missing", "Changes here can't be kept")
                    .icon(emaki_kit::icons::ICON_WARNING, Accent::Orange)
                    .subtitle("niri's config.kdl wasn't found"),
            ),
        ),
        Setup::Unmoved(blocks) => {
            let names: Vec<String> = blocks.iter().map(|b| b.output.name.clone()).collect();
            let what = if names.is_empty() {
                String::from("niri's config.kdl sets no displays yet.")
            } else {
                format!("niri's config.kdl sets {} itself.", names.join(", "))
            };
            Some(
                group()
                    .row(
                        FormRow::new("settings.displays.unmoved", "Shown, not changed here")
                            .icon(ICON_INFO, Accent::Gray)
                            .subtitle(what)
                            .trailing(
                                PushButton::new("settings.displays.move", "Move to Settings…")
                                    .on_press(confirm_move)
                                    .build(),
                            ),
                    )
                    .footer(
                        "Move the display settings to Emaki Settings to change them here; \
                         until then this page only shows them.",
                    ),
            )
        }
    }
}

fn confirm_move() {
    let file = kdl::displays_file();
    Dialog::new(
        "settings.displays.move-dialog",
        "Move display settings to Settings?",
    )
    .message(format!(
        "The output blocks in {} become comments, and an include of {} takes their \
             place with the same settings, as a commit of its own.",
        kdl::niri_config().display(),
        file.display()
    ))
    .button("Cancel", || {})
    .main_button("Move", || {
        thread::spawn(|| {
            if let Err(error) = kdl::migrate() {
                Screens::fail(error);
            }
            Screens::refresh();
        });
    })
    .open("main");
}

// ───── which screen ─────────────────────────────────────────────────────────

/// The screens laid out to drag, with two or more, and a picker that
/// also reaches the ones switched off.
pub fn arrangement(ctx: &Ctx) -> Option<Group> {
    if ctx.outputs.len() < 2 {
        return None;
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

    let inset = sys::space::inset::m();
    let area_height = 180.0;
    let area = Arrangement::new(
        "settings.displays.arrangement",
        screens,
        page_width() - inset * 2.0,
        area_height,
    )
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
        // kept only when confirmed, as a new mode is: a screen moved
        // somewhere unreachable comes back by itself
        let (output, name) = (output.clone(), name.clone());
        let (from_x, from_y) = output.logical.map_or((0, 0), |l| (l.x, l.y));
        try_change(
            &output,
            Change::Position { x, y },
            Change::Position {
                x: from_x,
                y: from_y,
            },
            format!("{name} at {x}, {y}"),
            move |block| block.position = Some((x, y)),
        );
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
        .width(crate::MENU_W_WIDE)
        .on_select(move |index| {
            if let Some(name) = all.get(index) {
                Screens::write().selected = Some(name.clone());
            }
        })
        .build();

    Some(
        group()
            .header("Arrangement")
            .custom(
                Rectangle::new()
                    .width(Size::Parent)
                    .height(area_height + inset * 2.0)
                    .padding(inset)
                    .child(area),
                area_height + inset * 2.0,
            )
            .row(FormRow::new("settings.displays.picker-row", "Display").trailing(picker))
            .footer("Drag a display to where it sits beside the others; it snaps to their edges."),
    )
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

pub fn header(ctx: &Ctx) -> Group {
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
    // a port's name says something only for a display plugged in
    if !output.built_in() {
        details.push(output.name.clone());
    }
    match output.mode().copied() {
        Some(mode) => details.push(format!(
            "{} × {} at {}",
            mode.width,
            mode.height,
            hz(mode.hz())
        )),
        None => details.push(String::from("Off")),
    }

    // the tags line up under the title, past the tile
    let tile = 28.0 * sys::density::factor();
    let inner = page_width() - style::row_pad_h() * 2.0 - tile - 10.0;
    let (facts, facts_h) = tags(inner, facts(output, &ctx.caps));
    let row = FormRow::new("settings.displays.header", ctx.name())
        .leading(
            LeadingIcon::new(ICON_DISPLAY)
                .accent(Accent::Teal)
                .large(true)
                .build(),
            tile,
        )
        .subtitle(details.join(" · "));
    let row = if facts_h > 0.0 {
        row.detail(frame(inner, facts_h).child(facts), facts_h)
    } else {
        row
    };
    group().row(row)
}

// ───── its picture ──────────────────────────────────────────────────────────

/// The brightness rows: the backlight's, or a monitor's own over DDC/CI.
pub fn brightness(ctx: &Ctx, group: Group) -> Group {
    if !ctx.output.built_in() {
        return external_brightness(&ctx.output, group);
    }
    let light = Brightness::read();
    if !light.present() {
        return group;
    }
    let percent = light.percent();
    drop(light);
    group.slider(
        SliderRow::new("Brightness", move |width| {
            Slider::new(percent, width)
                .wheel(false)
                .on_value_change(Brightness::set)
                .build()
        })
        .glyph(ICON_BRIGHTNESS_HIGH)
        .value(format!("{percent} %")),
    )
}

/// A monitor's own brightness and contrast, over DDC/CI.
fn external_brightness(output: &Output, group: Group) -> Group {
    let (brightness, contrast) = match Ddc::monitor(&output.name) {
        Monitor::Reading => {
            return group.row(
                FormRow::new("settings.displays.ddc", "Brightness").value("Asking the display…"),
            );
        }
        Monitor::Silent => {
            return group.row(
                FormRow::new("settings.displays.ddc", "Brightness").subtitle(
                    "This display doesn't take brightness from the computer (DDC/CI). Its own \
                     buttons set it.",
                ),
            );
        }
        Monitor::Levels {
            brightness,
            contrast,
            ..
        } => (brightness, contrast),
    };
    let slider = |title: &str, glyph: char, feature: Feature, level: ddc::Level| {
        let connector = output.name.clone();
        let percent = level.percent();
        SliderRow::new(title, move |width| {
            Slider::new(percent, width)
                .wheel(false)
                .on_value_change(move |percent| Ddc::set(&connector, feature, percent))
                .build()
        })
        .glyph(glyph)
        .value(format!("{percent} %"))
    };
    let mut group = group;
    if let Some(level) = brightness {
        group = group.slider(slider(
            "Brightness",
            ICON_BRIGHTNESS_HIGH,
            Feature::Brightness,
            level,
        ));
    }
    if let Some(level) = contrast {
        group = group.slider(slider(
            "Contrast",
            ICON_BRIGHTNESS_LOW,
            Feature::Contrast,
            level,
        ));
    }
    group
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
    let Some(from) = output.mode().copied() else {
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

pub fn resolution(ctx: &Ctx) -> FormRow {
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
    let current = output.mode().copied().map(|m| (m.width, m.height));
    let at = sizes.iter().position(|s| Some(*s) == current).unwrap_or(0);

    let looks = output
        .logical
        .map(|l| format!("Looks like {} × {}", l.width, l.height));
    let ctx_output = output.clone();
    let dropdown = Dropdown::new("settings.displays.resolution", labels, at)
        .width(crate::MENU_W_WIDE)
        .disabled(!ctx.editable() || !output.on())
        .on_select(move |index| {
            let Some(&size) = sizes.get(index) else {
                return;
            };
            let Some(now) = ctx_output.mode().copied() else {
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

    FormRow::new("settings.displays.resolution-row", "Resolution")
        .subtitle(looks.unwrap_or_default())
        .trailing(dropdown)
}

/// A refresh rate the page offers: a mode, and whether the rate varies up to it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rate {
    mode: Mode,
    variable: bool,
}

/// The rates at `rates`' resolution, fixed and then variable when the
/// display can vary its rate.
fn offered(rates: &[Mode], vrr: bool) -> Vec<Rate> {
    let fixed = rates.iter().map(|&mode| Rate {
        mode,
        variable: false,
    });
    let variable = rates.iter().filter(|_| vrr).map(|&mode| Rate {
        mode,
        variable: true,
    });
    fixed.chain(variable).collect()
}

/// "120 Hz", or for a varying rate its range when the EDID gives one:
/// "Variable, 30–60 Hz".
fn rate_label(rate: Rate, range: Option<(u16, u16)>) -> String {
    if !rate.variable {
        return hz(rate.mode.hz());
    }
    let top = rate.mode.hz().round() as u16;
    match range {
        Some((low, high)) if low < top => format!("Variable, {low}–{} Hz", top.min(high)),
        _ => format!("Variable up to {}", hz(rate.mode.hz())),
    }
}

/// The mode `displays.kdl` keeps for the power adapter: niri's, unless the
/// shell runs the display at its battery choice now.
fn adapter_mode(ctx: &Ctx) -> Option<Mode> {
    let now = ctx.output.mode().copied()?;
    if !super::moved_for_battery(&ctx.output) {
        return Some(now);
    }
    let saved = ctx
        .saved
        .as_ref()
        .and_then(|saved| saved_block(saved, &ctx.output))
        .and_then(|block| block.mode);
    let Some((width, height, rate)) = saved else {
        return Some(now);
    };
    Some(
        ctx.output
            .modes
            .iter()
            .copied()
            .find(|m| (m.width, m.height) == (width, height) && (m.hz() - rate).abs() < 0.5)
            .unwrap_or(now),
    )
}

/// The refresh rate on the power adapter (what `displays.kdl` keeps), and
/// on a laptop what it changes to on battery (`displays.json`, which the
/// shell follows).
pub fn refresh_rate(ctx: &Ctx) -> Vec<FormRow> {
    let Some(now) = adapter_mode(ctx) else {
        return Vec::new();
    };
    let rates = rates(&ctx.output, (now.width, now.height));
    let choices = offered(&rates, ctx.output.vrr_supported);
    let range = ctx.caps.refresh_range();
    let laptop = Battery::read().present();
    let title = "Refresh rate";

    let mut row = FormRow::new("settings.displays.refresh-row", title);
    if laptop {
        row = row.subtitle("On power adapter");
    }
    let variable_now = ctx.vrr() != Vrr::Off;
    let at = choices
        .iter()
        .position(|c| c.mode.refresh_rate == now.refresh_rate && c.variable == variable_now);
    let row = if choices.len() < 2 || !ctx.editable() {
        let shown = Rate {
            mode: now,
            variable: variable_now && ctx.output.vrr_supported,
        };
        row.value(rate_label(shown, range))
    } else {
        let labels: Vec<String> = choices.iter().map(|&c| rate_label(c, range)).collect();
        let output = ctx.output.clone();
        let vrr = ctx.vrr();
        row.trailing(
            Dropdown::new("settings.displays.refresh", labels, at.unwrap_or(0))
                .width(crate::MENU_W)
                .on_select(move |index| {
                    if let Some(&to) = choices.get(index) {
                        adapter_change(&output, now, vrr, to);
                    }
                })
                .build(),
        )
    };

    let mut rows = vec![row];
    if laptop {
        rows.push(battery_refresh(ctx, &rates));
    }
    rows
}

/// Moves the adapter's rate to `to`: at once and asked about, as any mode
/// change, unless the display runs at its battery choice now, when only
/// `displays.kdl` changes and the shell puts it on once plugged in.
fn adapter_change(output: &Output, now: Mode, vrr: Vrr, to: Rate) {
    let vrr_to = match (to.variable, vrr) {
        (false, _) => Vrr::Off,
        (true, Vrr::Off) => Vrr::Always,
        (true, kept) => kept,
    };
    let what = format!(
        "{} at {}{}",
        output.name,
        hz(to.mode.hz()),
        if to.variable { ", variable" } else { "" }
    );
    let keep = move |block: &mut OutputConfig| {
        block.mode = Some((to.mode.width, to.mode.height, to.mode.hz()));
        block.custom = false;
        block.vrr = vrr_to;
    };
    if super::moved_for_battery(output) {
        save(output, what, keep);
        return;
    }
    if vrr_to != vrr {
        let (name, change) = (
            output.name.clone(),
            Change::Vrr {
                on: vrr_to != Vrr::Off,
                on_demand: vrr_to == Vrr::OnDemand,
            },
        );
        thread::spawn(move || {
            if let Err(error) = outputs::apply(&name, &change) {
                Screens::fail(error);
            }
        });
    }
    if to.mode == now {
        save(output, what, keep);
    } else {
        try_change(output, Change::Mode(to.mode), Change::Mode(now), what, keep);
    }
}

/// What the rate does on battery: the same, or one of the rates offered.
fn battery_refresh(ctx: &Ctx, rates: &[Mode]) -> FormRow {
    let key = output_key(&ctx.output);
    let choice = displays().battery(&key);
    let rates_offered = offered(rates, ctx.output.vrr_supported);
    let range = ctx.caps.refresh_range();

    let mut labels = vec![String::from("Same as on adapter")];
    labels.extend(rates_offered.iter().map(|&c| rate_label(c, range)));
    let near = |mhz: u32, variable: bool| {
        // the fastest offered that isn't above it, as the shell picks
        rates_offered
            .iter()
            .enumerate()
            .filter(|(_, c)| c.variable == variable && c.mode.refresh_rate <= mhz + 500)
            .max_by_key(|(_, c)| c.mode.refresh_rate)
            .or_else(|| {
                rates_offered
                    .iter()
                    .enumerate()
                    .find(|(_, c)| c.variable == variable)
            })
            .map(|(index, _)| index + 1)
    };
    let at = match choice {
        BatteryRefresh::Same => Some(0),
        BatteryRefresh::Slowest => near(0, false),
        BatteryRefresh::Rate { mhz } => near(mhz, false),
        BatteryRefresh::Variable { mhz } => near(mhz, true).or_else(|| near(mhz, false)),
    };
    let control = Dropdown::new("settings.displays.battery-refresh", labels, at.unwrap_or(0))
        .width(crate::MENU_W)
        .on_select(move |index| {
            let choice = match index.checked_sub(1).and_then(|i| rates_offered.get(i)) {
                None => BatteryRefresh::Same,
                Some(rate) if rate.variable => BatteryRefresh::Variable {
                    mhz: rate.mode.refresh_rate,
                },
                Some(rate) => BatteryRefresh::Rate {
                    mhz: rate.mode.refresh_rate,
                },
            };
            let key = key.clone();
            change_displays(move |now| {
                now.on_battery.insert(key, choice);
            });
        })
        .build();
    FormRow::new("settings.displays.battery-refresh-row", "On battery")
        .child()
        .trailing(control)
}

const VRR_APP: &str = "settings.displays.vrr-app";

pub fn variable_refresh(ctx: &Ctx) -> Option<Group> {
    // a fixed rate on the adapter: nothing varies to set
    let now = ctx.vrr();
    if !ctx.output.vrr_supported || now == Vrr::Off {
        return None;
    }
    let output = ctx.output.clone();
    let choices = [Vrr::Always, Vrr::OnDemand];
    let control = Dropdown::new(
        "settings.displays.vrr",
        ["Always", "For chosen apps"].map(String::from).to_vec(),
        choices.iter().position(|c| *c == now).unwrap_or(0),
    )
    .width(crate::MENU_W)
    .disabled(!ctx.editable())
    .on_select(move |index| {
        let vrr = choices[index];
        let (name, change) = (
            output.name.clone(),
            Change::Vrr {
                on: true,
                on_demand: vrr == Vrr::OnDemand,
            },
        );
        if !super::moved_for_battery(&output) {
            thread::spawn(move || {
                if let Err(error) = outputs::apply(&name, &change) {
                    Screens::fail(error);
                }
            });
        }
        let label = match vrr {
            Vrr::OnDemand => "for chosen apps",
            _ => "always",
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
        "OLED panels can flicker at low frame rates while it's on, so For chosen apps suits them best."
    } else {
        "Some panels flicker at low frame rates while it's on."
    };
    let on_demand = if now == Vrr::OnDemand {
        "On the power adapter it varies only while one of the apps below shows. "
    } else {
        "On the power adapter it varies all the time; For chosen apps varies it only while one \
         of the apps you choose shows. "
    };
    let mut group = group()
        .row(FormRow::new("settings.displays.vrr-row", "Variable refresh").trailing(control))
        .footer(format!("{range}{on_demand}{flicker}"));
    if now == Vrr::OnDemand {
        group = vrr_apps(ctx, group);
    }
    Some(group)
}

/// The apps that turn on-demand variable refresh on, with a field to add
/// one by its app id.
fn vrr_apps(ctx: &Ctx, group: Group) -> Group {
    let apps = ctx
        .saved
        .as_ref()
        .map(|saved| saved.vrr_apps.clone())
        .unwrap_or_default();
    let rows = apps.iter().map(|app| {
        let gone = app.clone();
        FormRow::new(format!("settings.displays.vrr-app.{app}"), app.clone()).trailing(
            PushButton::new(format!("settings.displays.vrr-remove.{app}"), "Remove")
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
    let field = text_field(VRR_APP, "Add a game or app…").on_submit(move |text| add(&text));
    let add_row = FormRow::new("settings.displays.vrr-add-row", "Add an app")
        .subtitle(if apps.is_empty() {
            "None yet. Pick one that's open, or type its app id"
        } else {
            ""
        })
        .detail(
            Row::new(vec![
                boxed(field_box(VRR_APP, field, crate::FIELD_W)),
                boxed(
                    PushButton::new("settings.displays.vrr-add", "Add")
                        .on_press(move || add(&TextInput::text(VRR_APP)))
                        .build(),
                ),
                boxed(open_apps_menu(&apps, add)),
            ])
            .gap(sys::space::gap::s())
            .align(Center),
            style::control_height(),
        );
    group.rows(rows).row(add_row)
}

/// The app ids of the windows open now, looked up off the UI thread at
/// most every few seconds; the page draws again when they're in.
fn open_app_ids() -> Vec<String> {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;
    static OPEN: Mutex<(Option<Instant>, Vec<String>)> = Mutex::new((None, Vec::new()));
    static LOOKING: AtomicBool = AtomicBool::new(false);
    let Ok(open) = OPEN.lock() else {
        return Vec::new();
    };
    let stale = open
        .0
        .is_none_or(|at| at.elapsed() > std::time::Duration::from_secs(5));
    if stale && !LOOKING.swap(true, std::sync::atomic::Ordering::AcqRel) {
        thread::spawn(|| {
            let ids = outputs::window_app_ids();
            if let Ok(mut open) = OPEN.lock() {
                *open = (Some(Instant::now()), ids);
            }
            LOOKING.store(false, std::sync::atomic::Ordering::Release);
            drop(Screens::write());
        });
    }
    open.1.clone()
}

/// A small menu of the apps open now, to add one without knowing its id.
fn open_apps_menu(added: &[String], add: impl Fn(&str) + Clone + 'static) -> Rectangle {
    let ids: Vec<String> = open_app_ids()
        .into_iter()
        .filter(|id| !added.contains(id))
        .collect();
    let items = ids.iter().map(emaki_kit::menu::MenuItem::new).collect();
    let picked = ids.clone();
    emaki_kit::pop_up_button::PopUpButton::new("settings.displays.vrr-open-apps")
        .items(items, None)
        .disabled(ids.is_empty())
        .on_select(move |index| {
            if let Some(id) = picked.get(index) {
                add(id);
            }
        })
        .build()
}

/// The scales offered.
const SCALES: [f64; 9] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0];

pub fn scale(ctx: &Ctx) -> FormRow {
    let output = ctx.output.clone();
    let Some(logical) = output.logical else {
        return FormRow::new("settings.displays.scale-row", "Scale").value("Off");
    };
    let mode = output.mode().copied();
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
        .width(crate::MENU_W_WIDE)
        .disabled(!ctx.editable())
        .on_select(move |index| {
            if let Some(&to) = scales.get(index)
                && (to - from).abs() > 0.001
            {
                scale_change(&output, from, to);
            }
        })
        .build();
    FormRow::new("settings.displays.scale-row", "Scale")
        .subtitle("Larger makes text and controls bigger")
        .trailing(dropdown)
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

pub fn rotation(ctx: &Ctx) -> FormRow {
    let output = ctx.output.clone();
    let from = output.logical.map(|l| l.transform).unwrap_or_default();
    let labels: Vec<String> = Transform::ALL
        .iter()
        .map(|t| t.label().to_string())
        .collect();
    let at = Transform::ALL.iter().position(|t| *t == from).unwrap_or(0);
    let dropdown = Dropdown::new("settings.displays.rotation", labels, at)
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
    FormRow::new("settings.displays.rotation-row", "Rotation").trailing(dropdown)
}

pub fn hdr(ctx: &Ctx) -> Group {
    let caps = &ctx.caps;
    let output = ctx.output.clone();
    let title = "High dynamic range";
    if !caps.hdr10() {
        return group()
            .row(FormRow::new("settings.displays.hdr-row", title).value("Not supported"));
    }

    let peak = output
        .peak_luminance
        .or(caps.peak_nits().map(f64::from))
        .map_or(String::new(), |nits| {
            format!(" up to {} nits", nits.round())
        });
    if !output.hdr_supported {
        return group().row(
            FormRow::new("settings.displays.hdr-row", title).subtitle(format!(
                "This display shows HDR10{peak}, but the compositor can't drive it here: it \
                 needs to be on the main graphics card."
            )),
        );
    }

    let on = output.hdr_enabled;
    let auto = ctx
        .saved
        .as_ref()
        .and_then(|saved| super::saved_block(saved, &output))
        .is_some_and(|block| block.hdr_auto);
    // Off, Automatic (only while a video or game shows HDR), On
    let at = if auto {
        1
    } else if on {
        2
    } else {
        0
    };
    let labels = ["Off", "Automatic", "On"].map(String::from).to_vec();
    let switched = output.clone();
    let dropdown = Dropdown::new("settings.displays.hdr", labels, at)
        .disabled(!ctx.editable() || !output.on())
        .on_select(move |index| {
            if index == at {
                return;
            }
            let name = switched.name.clone();
            match index {
                // the file alone: niri switches when HDR shows
                1 => super::save(&switched, format!("{name} HDR automatic"), |block| {
                    block.hdr = false;
                    block.hdr_auto = true;
                }),
                _ => {
                    let hdr = index == 2;
                    let keep = move |block: &mut OutputConfig| {
                        block.hdr = hdr;
                        block.hdr_auto = false;
                    };
                    if hdr == on {
                        // already showing as asked (Automatic had it on or off): only the file
                        super::save(
                            &switched,
                            format!("{name} HDR {}", if hdr { "on" } else { "off" }),
                            keep,
                        );
                    } else {
                        try_change(
                            &switched,
                            Change::Hdr(hdr),
                            Change::Hdr(!hdr),
                            format!("{name} HDR {}", if hdr { "on" } else { "off" }),
                            keep,
                        );
                    }
                }
            }
        })
        .build();

    let apps = IconButton::new("settings.displays.hdr-apps", ICON_INFO)
        .tooltip(crate::WINDOW, "About HDR")
        .on_press(|| {
            Dialog::new("settings.displays.hdr-apps-dialog", "About HDR")
                .message(
                    "The desktop looks the same; video and games that support HDR show \
                     brighter highlights: mpv with --vo=gpu-next, Firefox with \
                     gfx.wayland.hdr turned on in about:config, and Vulkan games that ask for \
                     it. Automatic turns HDR on a second after one starts and off ten seconds \
                     after it ends, saving the memory and power HDR takes the rest of the time. \
                     While HDR is on, Brightness sets how bright white windows and text are.",
                )
                .button("OK", || {})
                .open("main");
        })
        .build();
    let group = group().header(title).row(
        FormRow::new("settings.displays.hdr-row", "Use HDR")
            .subtitle(if auto {
                format!("HDR10{peak}, {} now", if on { "on" } else { "off" })
            } else {
                format!("HDR10{peak}")
            })
            .trailing(apps)
            .trailing(dropdown),
    );
    group.footer("The screen blanks for a moment while it switches.")
}

// ───── night light ──────────────────────────────────────────────────────────

pub fn night_light() -> Group {
    super::night::section()
}

// ───── OLED care ────────────────────────────────────────────────────────────

pub fn oled_care(ctx: &Ctx) -> Option<Group> {
    if !ctx.caps.oled() {
        return None;
    }
    let black = {
        let now = appearance();
        now.black && now.mode != Some(Scheme::Light)
    };
    let hidden = dock().auto_hide;
    let bar_hidden = status_bar().auto_hide != emaki_kit::settings::AutoHide::Never;
    Some(
        group()
            .header("OLED care")
            .row(
                switch_row(
                    "settings.displays.oled.dark",
                    "Black appearance",
                    black,
                    |on| {
                        change_appearance(|now| {
                            now.black = on;
                            if on && now.mode == Some(Scheme::Light) {
                                now.mode = Some(Scheme::Dark);
                            }
                        })
                    },
                )
                .subtitle("Black pixels are off, so they use no power and don't wear"),
            )
            .row(switch_row(
                "settings.displays.oled.dock",
                "Hide the dock automatically",
                hidden,
                |on| change_dock(|now| now.auto_hide = on),
            ))
            .row(switch_row(
                "settings.displays.oled.bar",
                "Hide the status bar automatically",
                bar_hidden,
                |on| {
                    // turning it on keeps a mode already chosen
                    change_status_bar(|now| {
                        use emaki_kit::settings::AutoHide;
                        if !on {
                            now.auto_hide = AutoHide::Never;
                        } else if now.auto_hide == AutoHide::Never {
                            now.auto_hide = AutoHide::Always;
                        }
                    })
                },
            ))
            .row(
                FormRow::new("settings.displays.oled.sleep", "Screen sleep timing")
                    .value("In Power & Lock")
                    .on_press(|| crate::go("power")),
            )
            .footer(
                "An OLED's pixels wear with what stays lit for hours, and every lit pixel costs \
                 power. Black surfaces, a dock that hides and a screen that sleeps when idle all \
                 spare them.",
            ),
    )
}

// ───── more than one ────────────────────────────────────────────────────────

pub fn multiple(ctx: &Ctx) -> Option<Group> {
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

    Some(
        group()
            .header("This display")
            .row(FormRow::new("settings.displays.enabled-row", "Use this display").trailing(use_it))
            .row(
                FormRow::new("settings.displays.main-row", "Main display")
                    .subtitle("Where windows open first after you log in")
                    .trailing(main_switch),
            ),
    )
}

// ───── advanced ─────────────────────────────────────────────────────────────

const CUSTOM_SCALE: &str = "settings.displays.custom-scale";
const CUSTOM_MODE: &str = "settings.displays.custom-mode";
const BACKDROP: &str = "settings.displays.backdrop";

/// A row that opens the Advanced sub-page.
pub fn advanced_link() -> Group {
    group().row(
        FormRow::new("settings.displays.advanced-row", "Advanced")
            .subtitle("Custom scale and mode, the overview's backdrop, what the display reports")
            .on_press(|| crate::open(crate::View::Sub("advanced"))),
    )
}

/// The Advanced sub-page: a custom scale or mode, the overview's backdrop,
/// and what the display says of itself.
pub fn advanced(ctx: &Ctx) -> Group {
    let group = group();
    let editable = ctx.editable();
    let output = ctx.output.clone();

    // any scale niri takes
    let scale_output = output.clone();
    let from = output.logical.map_or(1.0, |l| l.scale);
    let custom_scale = FormRow::new("settings.displays.custom-scale-row", "Custom scale")
        .subtitle("A factor or a percentage, from 0.5 to 4")
        .detail(
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
            style::control_height(),
        );

    // a mode the monitor doesn't list
    let mode_output = output.clone();
    let custom_mode = FormRow::new("settings.displays.custom-mode-row", "Custom mode")
        .subtitle(
            "A mode the display doesn't list can leave it blank; it goes back by itself unless kept",
        )
        .detail(
            field_row(CUSTOM_MODE, "2560x1600@90", editable, move |text| {
                let Some((w, h, rate)) = parse_mode(text) else {
                    Screens::fail(String::from("A mode reads like 2560x1600@90."));
                    return;
                };
                let Some(from) = mode_output.mode().copied() else {
                    return;
                };
                try_change(
                    &mode_output,
                    Change::CustomMode { width: w, height: h, hz: rate },
                    Change::Mode(from),
                    format!("{} custom mode {w}x{h}@{rate}", mode_output.name),
                    move |block| {
                        block.mode = Some((w, h, rate));
                        block.custom = true;
                    },
                );
            }),
            style::control_height(),
        );

    // behind the workspaces in the overview
    let backdrop_output = output.clone();
    let backdrop = FormRow::new("settings.displays.backdrop-row", "Overview backdrop")
        .subtitle("A color like #1e1e2e behind the workspaces")
        .detail(
            field_row(BACKDROP, "#1e1e2e", editable, move |text| {
                let color = text.trim().to_string();
                let valid = color.len() == 7
                    && color.starts_with('#')
                    && color[1..].chars().all(|c| c.is_ascii_hexdigit());
                if !valid && !color.is_empty() {
                    Screens::fail(String::from("A color reads like #1e1e2e."));
                    return;
                }
                save(
                    &backdrop_output,
                    format!("{} backdrop {color}", backdrop_output.name),
                    move |block| block.backdrop = (!color.is_empty()).then_some(color),
                );
            }),
            style::control_height(),
        );

    // what was read from the screen itself
    let read = FormRow::new("settings.displays.raw-row", "Read from the display")
        .subtitle(raw_facts(ctx).join("\n"));

    group
        .row(custom_scale)
        .row(custom_mode)
        .row(backdrop)
        .row(read)
}

/// A text field and its Set button.
fn field_row(
    id: &'static str,
    placeholder: &str,
    editable: bool,
    set: impl Fn(&str) + Clone + 'static,
) -> Row {
    let submit = set.clone();
    let field = text_field(id, placeholder).on_submit(move |text| {
        if editable {
            submit(&text);
        }
    });
    Row::new(vec![
        boxed(field_box(id, field, crate::FIELD_W)),
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
    fn variable_rates_follow_the_fixed_ones_with_their_range() {
        let mode = |hz: u32| Mode {
            width: 2880,
            height: 1800,
            refresh_rate: hz * 1000,
            is_preferred: false,
        };
        let rates = [mode(60), mode(120)];
        let offered = offered(&rates, true);
        let range = Some((30, 120));
        let labels: Vec<String> = offered.iter().map(|&r| rate_label(r, range)).collect();
        assert_eq!(
            labels,
            [
                "60 Hz",
                "120 Hz",
                "Variable, 30–60 Hz",
                "Variable, 30–120 Hz"
            ]
        );
        assert_eq!(rate_label(offered[3], None), "Variable up to 120 Hz");
        // without variable refresh, only the fixed ones
        assert_eq!(super::offered(&rates, false).len(), 2);
    }

    #[test]
    fn rates_print_whole_when_whole() {
        assert_eq!(hz(120.0), "120 Hz");
        assert_eq!(hz(59.997), "60 Hz");
        assert_eq!(hz(59.94), "59.94 Hz");
    }
}
