//! A status bar along the bottom (or top) edge, built on the Amane fork.
//!
//! Ambient, and one of three sizes ([`size`]): Compact (the previous
//! Waybar's), Regular and Comfortable. The workspace's shape is on the left
//! ([`workspace_shape`]) and the status cluster on the right. The open
//! apps are the [`dock`](crate::surfaces::dock)'s, which rises over the
//! bar from the bottom edge and while niri's overview is open.
//!
//! Every item is a lone glyph (or the workspace's shape) that turns into a white
//! rounded tile with a black glyph while hovered, and stays one while what
//! it opens is open. Everything is driven by the Penpot tokens in
//! `design-tokens/tokens.json` via [`tokens`](crate::tokens).
//!
//! Where it sits, what it's made of, its clock and which icons it shows
//! are `~/.config/amane-shell/status-bar.json` (Shell Settings), followed
//! live. With `auto_hide` it slides past its edge ([`EdgeReveal`]) and
//! keeps no room from windows, coming back when the pointer meets the edge
//! or Quick Settings or the notification center is open, which spares an
//! OLED panel a strip that never changes.

mod activities;
mod apps;
mod chips;
mod drag;
mod indicators;
mod size;
mod tray;
mod workspace_shape;

use std::cell::Cell;
use std::time::Duration;

use amane::{
    Animation, Audio, Battery, Bluetooth, BluetoothDevice, Center, Color, Easing, Full, InputArea,
    Justify, Keyboard, Layer, LayerWindow, Link, Monitor, Network, Parent, PowerProfile, Rectangle,
    Row, Scroll, ScrollSource, Service, Size, Start, Vertical, Widget, Workspace, Workspaces, Zone,
    window_size,
};
use amane_kit::settings::{AutoHide, BarStyle, ClockStyle, ShowDate, status_bar};
use amane_kit::theme::{self, Mode};
use amane_phone::KdeConnect;

use crate::chrome::battery;
use crate::chrome::panel::{self, Panel, is_open};
use crate::services::clock::Clock;
use crate::services::display::night_light::NightLight;
use crate::services::do_not_disturb::DoNotDisturb;
use crate::services::network::connections::Connections;
use crate::services::network::rfkill::Rfkill;
use crate::services::network::tailscale::{State as TailscaleState, Tailscale};
use crate::services::power::keep_awake::KeepAwake;
use crate::surfaces::notifications;
use crate::surfaces::quick_settings::{self, Page};
use amane_kit::buttons::{ButtonBuilder, DrawnButton, IconButton, PushButton, Shape, Style};
use amane_kit::edge_reveal::EdgeReveal;
use amane_kit::icons::{
    Glyph, ICON_AIRPLANE, ICON_BLUETOOTH, ICON_BLUETOOTH_CONNECTED, ICON_BLUETOOTH_OFF, ICON_CABLE,
    ICON_DND, ICON_KEEP_AWAKE, ICON_NIGHT_LIGHT, ICON_NOTIFICATIONS, ICON_PHONE, ICON_TUNE,
    ICON_VOLUME_OFF, ICON_VPN, ICON_WIFI, ICON_WIFI_NONE, ICON_WIFI_OFF, audio_glyph,
    battery_glyph, power_profile_glyph, wifi_glyph,
};
use amane_kit::layout::{boxed, cursor, pad, text_width, track_cursor};
use amane_kit::material::{self, Level, Material};
use amane_kit::slider;

use crate::tokens::{bar, sys};

pub(crate) use indicators::{Indicator, Item};

use crate::chrome::bar::{edge, height};
use crate::chrome::per_output::PerOutput;
pub use activities::popup as activity_popup;
pub use indicators::{ipc, migrate};
pub use size::metrics;
pub use workspace_shape::switch_position;

/// The status bar's layer-surface namespace. The niri `layer-rule` matches
/// this to frost its popups, and tooltips hang under it.
pub const BAR_NS: &str = "amane.status-bar";

/// How many lines of touchpad scroll move one workspace along.
/// TODO(token): `bar.scroll.step` once Penpot has it.
const SCROLL_STEP: f32 = 3.0;

/// How tall the strip along the edge is that brings a hidden bar back.
const STRIP: f32 = 2.0;

thread_local! {
    // touchpad scroll over the workspaces not yet made into a step
    static SCROLLED: Cell<f32> = const { Cell::new(0.0) };

    // where each monitor's hiding bar is, when it hides
    static REVEALS: PerOutput<EdgeReveal> = const { PerOutput::new() };
}

fn new_reveal() -> EdgeReveal {
    EdgeReveal::new(
        Animation::new(0.0)
            .duration(if theme::reduced_motion() {
                Duration::ZERO
            } else {
                Duration::from_millis(220)
            })
            .easing(Easing::Out),
        Duration::from_millis(400),
    )
}

/// Runs `f` on monitor `output`'s bar's reveal.
fn with_reveal<R>(output: &str, f: impl FnOnce(&mut EdgeReveal) -> R) -> R {
    REVEALS.with(|reveals| reveals.with(output, new_reveal, f))
}

/// The status bar on `monitor`, one on each display. Registered with
/// `App::window_per_monitor`.
pub fn bar(monitor: &Monitor) -> LayerWindow {
    let output = monitor.name.clone();
    let style = status_bar().style;

    // a clear bar's items are light or dark to suit the wallpaper under them
    let sections = if style == BarStyle::Clear {
        Material::Control.adapted(clear_id(&output), || sections(&output))
    } else {
        sections(&output)
    };

    let metrics = metrics();
    let height = metrics.height;
    let (width, _) = window_size();

    // how far it's slid past its edge: 0 in place, `height` gone
    let hiding = hides(&output);
    let (offset, input) = with_reveal(&output, |reveal| {
        reveal.set_top(edge() == Vertical::Top);
        reveal.keep_out(!hiding || is_open(Panel::QuickSettings) || is_open(Panel::Notifications));
        let progress = reveal.progress(0.0);
        let shown = InputArea {
            x: 0,
            y: 0,
            width: width.max(1.0) as i32,
            height: height as i32,
        };
        let input = reveal.input_region(width, height, STRIP, progress, shown);
        let away = (1.0 - progress) * height;
        let offset = match edge() {
            Vertical::Top => -away,
            _ => away,
        };
        (offset, input)
    });

    let panel = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .radius(bar::radius())
        .fill(fill(style))
        .padding(pad(sys::space::inset::m(), metrics.padding_y))
        .translate(0.0, offset);
    let panel = track_cursor(panel).child(sections);
    let (hovered, moved) = (output.clone(), output.clone());
    let panel = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .on_hover(move |over| with_reveal(&hovered, |reveal| reveal.hover(over)))
        // a pointer that arrived without an enter still counts
        .on_move(move |_| with_reveal(&moved, |reveal| reveal.hover(true)))
        .child(panel);

    let window = LayerWindow::new()
        .width(Full)
        .height(height)
        .vibrant(style != BarStyle::Solid && material::vibrant());

    // Frosted leaves niri's layer rule to frost the whole strip in the bar's
    // material; the others say where themselves. A hiding bar says where too,
    // so the frost slides away with it.
    let window = match style {
        BarStyle::Frosted if !hiding => window,
        BarStyle::Frosted => window.blur_shapes(vec![
            Material::Bar.shape(0.0, offset, width, height, 0.0, 1.0),
        ]),
        BarStyle::Glass => window.blur_shapes(vec![
            Material::Control.shape(0.0, offset, width, height, 0.0, 1.0),
        ]),
        BarStyle::Solid => window.blur_shapes(Vec::new()),
        // all but invisible, so niri still reads the wallpaper under it
        BarStyle::Clear => window.blur_shapes(vec![
            Material::Media
                .shape(0.0, offset, width, height, 0.0, 0.02)
                .id(clear_id(&output)),
        ]),
    };
    let window = if hiding {
        window.input_region(input)
    } else {
        window
    };

    window
        .anchor_vertical(edge())
        .layer(Layer::Top)
        .space(if hiding { Zone::Ignore } else { Zone::Reserve })
        // None so bar clicks never steal keyboard focus from the active app
        // (a menu bar's behaviour); the panels take keys when clicked into.
        .keyboard(Keyboard::None)
        .namespace(BAR_NS)
        .visible(true)
        .child(panel)
}

/// The workspaces and the items, side by side.
fn sections(output: &str) -> Row {
    let left: Box<dyn Widget> = if status_bar().show_workspaces {
        boxed(workspaces(output))
    } else {
        boxed(Row::new(Vec::new()))
    };
    // what the right has, clear of the left and the bar's insets
    let room = window_size().0
        - width_of(left.as_ref())
        - 2.0 * sys::space::inset::m()
        - sys::space::gap::l();
    Row::new(vec![left, boxed(status(output, room))])
        .width(Parent)
        .height(Parent)
        .justify(Justify::SpaceBetween)
        .align(Center)
}

/// The id niri's readings of the wallpaper under monitor `output`'s clear
/// bar come by.
fn clear_id(output: &str) -> u32 {
    amane_kit::glass_region::shape_id(&format!("amane.status-bar.{output}"))
}

/// Whether the bar on monitor `output` hides now, as `auto_hide` says.
fn hides(output: &str) -> bool {
    match status_bar().auto_hide {
        AutoHide::Never => false,
        AutoHide::Always => true,
        AutoHide::Desktop => shown_workspace(output).is_some_and(|(_, windows)| windows == 0),
        AutoHide::Fullscreen => shown_workspace(output).is_some_and(|(id, _)| {
            amane::Windows::read()
                .list()
                .iter()
                .any(|window| window.workspace() == Some(id) && window.fullscreen())
        }),
    }
}

/// The workspace monitor `output` shows, and how many windows it has.
fn shown_workspace(output: &str) -> Option<(i64, u32)> {
    Workspaces::read()
        .list()
        .iter()
        .find(|workspace| workspace.output() == Some(output) && workspace.active())
        .map(|workspace| (workspace.id(), workspace.windows()))
}

/// What the strip is painted with: the bar's tint over niri's frost, the
/// glass's own (none where niri draws it), black or white, or nothing.
fn fill(style: BarStyle) -> Color {
    match style {
        BarStyle::Clear => Color::TRANSPARENT,
        BarStyle::Frosted => Material::Bar.tint(),
        BarStyle::Glass => Material::Control.fill(),
        BarStyle::Solid => match theme::mode() {
            Mode::Dark => Color::BLACK,
            Mode::Light => Color::WHITE,
        },
    }
}

// ───── left: workspaces ──────────────────────────────────────────────────────

/// The one shape that says which of monitor `output`'s workspaces shows
/// ([`workspace_shape`]); scrolling over it moves between them.
fn workspaces(output: &str) -> Rectangle {
    let width = metrics().item_width;
    let indicator = {
        let workspaces = Workspaces::read();
        let here: Vec<&Workspace> = workspaces
            .list()
            .iter()
            .filter(|workspace| on_output(workspace, output))
            .collect();
        workspace_shape::indicator(output, &here, width)
    };
    let scrolled_on = output.to_string();

    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .on_scroll(move |scroll| {
            if status_bar().scroll_workspaces {
                scroll_workspaces(&scrolled_on, scroll);
            }
        })
        .child(indicator)
}

/// A wheel notch, or [`SCROLL_STEP`] lines of touchpad, focuses the next
/// workspace down or the previous one up.
fn scroll_workspaces(output: &str, scroll: Scroll) {
    let delta = if scroll.y != 0.0 { scroll.y } else { scroll.x };
    let step = match scroll.source {
        ScrollSource::Wheel | ScrollSource::Tilt => direction(delta),
        ScrollSource::Finger | ScrollSource::Continuous => SCROLLED.with(|scrolled| {
            let total = if scroll.stop {
                0.0
            } else {
                scrolled.get() + delta
            };
            let step = if total.abs() >= SCROLL_STEP {
                direction(total)
            } else {
                0
            };
            scrolled.set(if step == 0 { total } else { 0.0 });
            step
        }),
    };
    if step == 0 {
        return;
    }

    let workspaces = Workspaces::read();
    let list: Vec<&Workspace> = workspaces
        .list()
        .iter()
        .filter(|workspace| on_output(workspace, output))
        .collect();
    // the one shown on this monitor, focused or not
    let current = list
        .iter()
        .position(|workspace| workspace.focused())
        .or_else(|| list.iter().position(|workspace| workspace.active()));
    if let Some(next) = stepped(current, list.len(), step) {
        let id = list[next].id();
        drop(workspaces);
        Workspaces::focus(id);
    }
}

/// Whether `workspace` is on monitor `output`; one niri hasn't placed
/// shows on every bar.
fn on_output(workspace: &Workspace, output: &str) -> bool {
    workspace.output().is_none_or(|on| on == output)
}

fn direction(delta: f32) -> i32 {
    if delta > 0.0 {
        1
    } else if delta < 0.0 {
        -1
    } else {
        0
    }
}

/// The index `step` along from `current` among `len`, if that's another
/// one: it stops at either end.
fn stepped(current: Option<usize>, len: usize, step: i32) -> Option<usize> {
    let current = current?;
    let next = current.checked_add_signed(step as isize)?;
    (next < len && next != current).then_some(next)
}

// ───── right: status cluster ─────────────────────────────────────────────────

/// The items on the right that have something to show, in their order,
/// draggable into another.
/// Apps' items that don't fit in `room` go behind a » button.
fn status(output: &str, room: f32) -> amane::Stack {
    let apps = if Item::Tray.shown() {
        tray::shown()
    } else {
        Vec::new()
    };
    let build_all = |hidden: usize, dated: bool| -> Vec<(Item, Vec<Box<dyn Widget>>)> {
        Item::ordered()
            .into_iter()
            .filter(|item| item.shown())
            .map(|item| match item {
                Item::Tray => (item, tray::icons(&apps, hidden)),
                Item::Clock => (item, vec![clock(dated)]),
                Item::App(name) => (item, apps::widgets(name, output)),
                _ => (item, build(item)),
            })
            .filter(|(_, widgets)| !widgets.is_empty())
            .collect()
    };
    let gap = sys::space::gap::_2xs();
    let width_of_all = |built: &[(Item, Vec<Box<dyn Widget>>)]| -> f32 {
        built
            .iter()
            .flat_map(|(_, widgets)| widgets.iter())
            .map(|widget| width_of(widget.as_ref()) + gap)
            .sum()
    };
    let mut dated = true;
    let mut built = build_all(0, dated);
    let mut width = width_of_all(&built);
    // a date shown "when space allows" goes first, before any app's icon
    if width > room && status_bar().show_date == ShowDate::WhenSpace {
        dated = false;
        built = build_all(0, dated);
        width = width_of_all(&built);
    }
    if width > room && !apps.is_empty() {
        let over = width - room;
        // one for each icon's width, and room for the » button
        let mut hidden = 1;
        while hidden < apps.len() && tray::width_of(hidden - 1) < over {
            hidden += 1;
        }
        built = build_all(hidden, dated);
    }

    drag::cluster(output, built, height())
}

/// What `item` shows now: nothing, one widget, or a few for the privacy
/// and "on" icons.
fn build(item: Item) -> Vec<Box<dyn Widget>> {
    match item {
        Item::Tray | Item::App(_) => Vec::new(),
        Item::Privacy => chips::privacy(),
        Item::Activities => activities::widgets(),
        Item::StatusIcons => on_icons(),
        Item::Volume => volume().into_iter().collect(),
        Item::Bluetooth => bluetooth().into_iter().collect(),
        Item::Network => network().into_iter().collect(),
        Item::Tailscale => tailscale().into_iter().collect(),
        Item::KeepAwake => keep_awake().into_iter().collect(),
        Item::Phone => phone().into_iter().collect(),
        Item::Battery => battery().into_iter().collect(),
        Item::QuickSettings => vec![quick_settings_button()],
        Item::Notifications => notification_icon().into_iter().collect(),
        Item::Clock => vec![clock(true)],
        Item::Keyboard => keyboard().into_iter().collect(),
        Item::Weather => weather().into_iter().collect(),
    }
}

fn volume() -> Option<Box<dyn Widget>> {
    let audio = Audio::read();
    let muted = audio.muted();
    let volume = audio.volume();
    let kind = audio.output_kind();
    drop(audio);

    // what is playing through: speakers, headphones, a headset, the TV …
    let (glyph, tip) = if muted {
        (ICON_VOLUME_OFF, String::from("Muted"))
    } else {
        (audio_glyph(kind, false), format!("Volume {volume}%"))
    };

    let icon = status_icon(
        "amane.bar.volume",
        glyph,
        material::label(Level::Secondary),
        Page::Sound,
        tip,
    )
    .on_scroll(|scroll: Scroll| {
        // the volume as it is now: a burst of scroll steps comes before the
        // bar draws again, and each should step on from the last
        let now = Audio::read().volume();
        let next = slider::scrolled(now, scroll, slider::STEP);
        if next != now {
            Audio::set_volume(next);
        }
    })
    .on_middle(Audio::toggle_mute);
    Some(boxed(icon.build()))
}

/// Only with an adapter: off, on, or connected (in the accent).
fn bluetooth() -> Option<Box<dyn Widget>> {
    let bluetooth = Bluetooth::read();
    if !bluetooth.available() {
        return None;
    }
    let powered = bluetooth.powered();
    let connected: Vec<String> = bluetooth
        .devices()
        .iter()
        .filter(|device| device.connected())
        .map(|device| BluetoothDevice::name(device).to_string())
        .collect();
    drop(bluetooth);

    let (glyph, tip) = match (powered, connected.is_empty()) {
        (false, _) => (ICON_BLUETOOTH_OFF, String::from("Bluetooth off")),
        (true, true) => (ICON_BLUETOOTH, String::from("Bluetooth on")),
        (true, false) => (ICON_BLUETOOTH_CONNECTED, connected.join(", ")),
    };
    let color = status_color(!connected.is_empty());
    Some(boxed(
        status_icon("amane.bar.bluetooth", glyph, color, Page::Bluetooth, tip).build(),
    ))
}

fn network() -> Option<Box<dyn Widget>> {
    let net = Network::read();
    let link = net.link();
    let strength = net.strength();
    let wifi_on = net.wifi_enabled();
    let ssid = net.ssid().to_string();
    drop(net);

    let (glyph, tip) = match link {
        Link::Wifi => (wifi_glyph(strength), ssid),
        Link::Wired => (ICON_CABLE, String::from("Wired")),
        // vpn and the rest carry no strength to show
        Link::Other => (ICON_WIFI, String::from("Connected")),
        Link::Offline if wifi_on => (ICON_WIFI_NONE, String::from("Offline")),
        Link::Offline => (ICON_WIFI_OFF, String::from("Wi‑Fi off")),
    };
    // grey like its neighbours: the glyph already shows the link
    Some(boxed(
        status_icon(
            "amane.bar.network",
            glyph,
            material::label(Level::Secondary),
            Page::Wifi,
            tip,
        )
        .build(),
    ))
}

/// Tailscale's mark while connected; accent while routing through an
/// exit node.
fn tailscale() -> Option<Box<dyn Widget>> {
    let tailscale = Tailscale::read();
    if tailscale.state() != TailscaleState::Running {
        return None;
    }
    let routed = tailscale.exit_node().is_some();
    let tip = format!("Tailscale · {}", tailscale.describe());
    drop(tailscale);

    Some(boxed(
        status_icon(
            "amane.bar.tailscale",
            Glyph::Tailscale,
            status_color(routed),
            Page::Tailscale,
            tip,
        )
        .build(),
    ))
}

/// While Keep Awake holds anything; accent, since the machine isn't
/// behaving as it normally would.
fn keep_awake() -> Option<Box<dyn Widget>> {
    if !KeepAwake::read().on() {
        return None;
    }
    let tip = format!("Keep Awake · {}", quick_settings::keep_awake_status());

    Some(boxed(
        status_icon(
            "amane.bar.keep-awake",
            ICON_KEEP_AWAKE,
            status_color(true),
            Page::KeepAwake,
            tip,
        )
        .build(),
    ))
}

fn battery() -> Option<Box<dyn Widget>> {
    let charge = Battery::read();
    if !charge.present() {
        return None;
    }
    let pct = charge.percent();
    let on_ac = charge.on_ac();
    let low = battery::low(&charge);
    drop(charge);

    let (color, tip) = if on_ac {
        (
            sys::color::accent::bg(),
            format!("Battery {pct}% · charging"),
        )
    } else {
        (
            battery::tint(low, material::label(Level::Secondary)),
            format!("Battery {pct}%"),
        )
    };

    Some(boxed(
        status_icon(
            "amane.bar.battery",
            battery_glyph(pct, on_ac),
            color,
            Page::Power,
            tip,
        )
        .build(),
    ))
}

/// The phone KDE Connect has, with its charge, while it's connected:
/// critical while low, accent while charging, like the battery beside it.
fn phone() -> Option<Box<dyn Widget>> {
    let kdeconnect = KdeConnect::read();
    let phone = kdeconnect.primary()?;
    let name = phone.name().to_string();
    let charge = phone.battery()?;
    drop(kdeconnect);

    let (color, tip) = if charge.charging {
        (
            sys::color::accent::bg(),
            format!("{name} {}% · charging", charge.percent),
        )
    } else {
        (
            battery::color(charge.percent, false, material::label(Level::Secondary)),
            format!("{name} {}%", charge.percent),
        )
    };

    Some(boxed(
        status_icon("amane.bar.phone", ICON_PHONE, color, Page::Phone, tip).build(),
    ))
}

/// Quick Settings' button in the bar.
fn quick_settings_button() -> Box<dyn Widget> {
    boxed(
        bar_icon(
            "amane.bar.quick-settings",
            ICON_TUNE,
            material::label(Level::Secondary),
        )
        .active(is_open(Panel::QuickSettings) && !quick_settings::alone())
        // over a status icon's page, it brings the whole card instead
        .on_press(|| {
            if quick_settings::alone() {
                quick_settings::open(Page::Grid);
            } else {
                panel::toggle(Panel::QuickSettings);
            }
        })
        .tooltip(BAR_NS, "Quick Settings")
        .build(),
    )
}

/// Settings GNOME shows in the bar only while they're on: night light, a
/// VPN, a power mode other than Balanced, airplane mode.
fn on_icons() -> Vec<Box<dyn Widget>> {
    let mut glyphs = Vec::new();

    if NightLight::read().on() {
        glyphs.push(("amane.bar.night-light", ICON_NIGHT_LIGHT, "Night Light"));
    }
    if Connections::read().vpns().iter().any(|vpn| vpn.active) {
        glyphs.push(("amane.bar.vpn", ICON_VPN, "VPN"));
    }
    let profile = PowerProfile::read();
    if profile.present() && profile.active() != "balanced" {
        let tip = if profile.active() == "performance" {
            "Performance Mode"
        } else {
            "Power Saver"
        };
        glyphs.push((
            "amane.bar.power-mode",
            power_profile_glyph(profile.active()),
            tip,
        ));
    }
    drop(profile);
    if Rfkill::read().airplane() {
        glyphs.push(("amane.bar.airplane", ICON_AIRPLANE, "Airplane Mode"));
    }

    grid_icons(glyphs, material::label(Level::Secondary))
}

/// Status glyphs in `color`, each opening Quick Settings' grid.
fn grid_icons(
    glyphs: Vec<(&'static str, char, impl Into<String>)>,
    color: Color,
) -> Vec<Box<dyn Widget>> {
    glyphs
        .into_iter()
        .map(|(id, glyph, tip)| boxed(status_icon(id, glyph, color, Page::Grid, tip).build()))
        .collect()
}

/// A moon while Do Not Disturb is on, else a bell while something arrived
/// unseen; nothing otherwise. Click opens the center, right-click picks
/// how long Do Not Disturb lasts.
fn notification_icon() -> Option<Box<dyn Widget>> {
    let dnd = DoNotDisturb::read().on();
    let unseen = notifications::unseen();

    let (glyph, color, tip) = match (dnd, unseen) {
        (true, _) => (ICON_DND, material::label(Level::Primary), "Do Not Disturb"),
        (false, true) => (
            ICON_NOTIFICATIONS,
            sys::color::accent::bg(),
            "New notifications",
        ),
        (false, false) => return None,
    };

    Some(boxed(
        bar_icon("amane.bar.notifications", glyph, color)
            .active(is_open(Panel::Notifications))
            .on_press(|| panel::toggle(Panel::Notifications))
            .on_secondary(|| notifications::dnd_menu(BAR_NS, (cursor().0, menu_edge())))
            .tooltip(BAR_NS, tip)
            .build(),
    ))
}

/// The clock opens the notification center; its tooltip is
/// the date. `dated` says whether there's room for a date shown "when space
/// allows".
fn clock(dated: bool) -> Box<dyn Widget> {
    let clock = Clock::read();
    let time = clock.bar_text(dated);
    let date = clock.date();
    let (hour, minute) = clock.hands();
    let full_time = clock.text();
    drop(clock);

    if status_bar().clock_style == ClockStyle::Analog {
        return boxed(
            item(DrawnButton::new("amane.bar.clock", move |color| {
                analog(hour, minute, color)
            }))
            .width(metrics().item_width)
            .color(material::label(Level::Primary))
            .active(is_open(Panel::Notifications))
            .on_press(|| panel::toggle(Panel::Notifications))
            .tooltip(BAR_NS, format!("{full_time} · {date}"))
            .build(),
        );
    }

    // as wide as the time with its colons, so flashing them never moves it
    let widest = clock_width(&time);
    let width = widest + sys::space::inset::s() * 2.0;

    boxed(
        item(PushButton::new("amane.bar.clock", time))
            .text(metrics().numeric())
            .width(width)
            .active(is_open(Panel::Notifications))
            .on_press(|| panel::toggle(Panel::Notifications))
            .tooltip(BAR_NS, date)
            .build(),
    )
}

/// How wide the clock's text is with its colons, whether or not they show.
fn clock_width(time: &str) -> f32 {
    text_width(&time.replace('\u{2008}', ":"), metrics().numeric())
}

/// A small clock face with its two hands, the bar's analog clock.
fn analog(hour: u32, minute: u32, color: Color) -> Rectangle {
    let face = metrics().icon;
    let hand = |length: f32, thickness: f32, degrees: f32| -> Box<dyn Widget> {
        // a hand from the middle up, in a frame the size of the face that turns
        boxed(
            Rectangle::new()
                .width(Size::Fixed(face))
                .height(Size::Fixed(face))
                .fill(Color::TRANSPARENT)
                .rotate(degrees)
                .align_child(Center, Start)
                .padding(amane::Padding {
                    top: face / 2.0 - length,
                    ..amane::Padding::default()
                })
                .child(
                    Rectangle::new()
                        .width(Size::Fixed(thickness))
                        .height(Size::Fixed(length))
                        .radius(thickness / 2.0)
                        .fill(color),
                ),
        )
    };
    let minutes = minute as f32 * 6.0;
    let hours = (hour % 12) as f32 * 30.0 + minute as f32 * 0.5;
    Rectangle::new()
        .width(Size::Fixed(metrics().item_width))
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .child(
            Rectangle::new()
                .width(Size::Fixed(face))
                .height(Size::Fixed(face))
                .radius(face / 2.0)
                .border(1.5, color)
                .child(amane::Stack::new(vec![
                    // drawn for a 16 px face, and grown with a bigger one
                    hand(4.0 * face / 16.0, 1.5, hours),
                    hand(6.0 * face / 16.0, 1.2, minutes),
                ])),
        )
}

/// The keyboard layout as an input menu shows it, a short code, with
/// two or more; a click lists them to switch.
fn keyboard() -> Option<Box<dyn Widget>> {
    use crate::services::desktop::keyboard_layout::{self, KeyboardLayout, short};
    let layouts = KeyboardLayout::read();
    if layouts.names().len() < 2 {
        return None;
    }
    let name = layouts.current().unwrap_or_default().to_string();
    drop(layouts);
    let code = short(&name);
    let width = text_width(&code, metrics().label()) + sys::space::inset::s() * 2.0;
    Some(boxed(
        item(PushButton::new("amane.bar.keyboard", code))
            .text(metrics().label())
            .width(width.max(metrics().item_width))
            .on_press(|| {
                let layouts = KeyboardLayout::read();
                let current = layouts.current_index();
                let mut menu = amane_kit::menu::ContextMenu::new();
                for (index, name) in layouts.names().iter().enumerate() {
                    let row = amane_kit::menu::MenuItem::label(name.clone())
                        .check(amane_kit::menu::Check::Radio(index == current))
                        .on_select(move || keyboard_layout::switch(index));
                    menu = menu.item(row);
                }
                drop(layouts);
                amane_kit::menu::layer::show_under(
                    "amane.bar.keyboard",
                    menu,
                    BAR_NS,
                    (cursor().0 as i32, menu_edge()),
                );
            })
            .tooltip(BAR_NS, name)
            .build(),
    ))
}

/// The weather where `weather_place` is, once it's been read: its glyph
/// and temperature; the tooltip says the place, the day's high and low.
fn weather() -> Option<Box<dyn Widget>> {
    use crate::services::desktop::weather::{Weather, describe};
    if !status_bar().weather {
        return None;
    }
    let weather = Weather::read();
    let now = weather.now()?.clone();
    let place = weather.place().to_string();
    drop(weather);
    let (glyph, words) = describe(now.code, now.day);
    let text = format!("{:.0}°", now.temperature);
    let width =
        text_width(&text, metrics().numeric()) + metrics().item_width + sys::space::inset::s();
    Some(boxed(
        item(PushButton::new("amane.bar.weather", text))
            .icon(glyph)
            .icon_size(metrics().icon)
            .text(metrics().numeric())
            .width(width)
            .tooltip(
                BAR_NS,
                format!("{place} · {words} · H:{:.0}° L:{:.0}°", now.high, now.low),
            )
            .build(),
    ))
}

/// The bar's edge facing the screen, for a menu to open from: its top
/// along the bottom, its bottom along the top.
fn menu_edge() -> i32 {
    match edge() {
        Vertical::Top => height() as i32,
        _ => 0,
    }
}

// ───── helpers ───────────────────────────────────────────────────────────────

/// Any bar item: unfilled until hovered, then a white rounded tile with
/// its content black, the bar's height.
fn item<K>(button: ButtonBuilder<K>) -> ButtonBuilder<K> {
    button
        .style(Style::Plain)
        .shape(Shape::Square)
        .highlight(bar::indicator::active::bg(), bar::indicator::active::fg())
        .height(Parent)
}

/// A bar item that's a lone glyph.
fn bar_icon(id: &'static str, glyph: impl Into<Glyph>, color: Color) -> IconButton {
    let metrics = metrics();
    item(IconButton::new(id, glyph))
        .size(metrics.icon)
        .color(color)
        .width(metrics.item_width)
}

/// A status glyph opening its Quick Settings `page` on its own, closing it
/// again, and highlighted while that page shows. The grid's open the whole
/// card and are left alone, or opening Quick Settings would light them all.
fn status_icon(
    id: &'static str,
    glyph: impl Into<Glyph>,
    color: Color,
    page: Page,
    tip: impl Into<String>,
) -> IconButton {
    bar_icon(id, glyph, color)
        .active(page != Page::Grid && quick_settings::showing(page))
        .on_press(move || {
            if page == Page::Grid {
                quick_settings::open(page);
            } else {
                quick_settings::toggle_alone(page);
            }
        })
        .tooltip(BAR_NS, tip)
}

/// How wide a boxed widget is, when it says.
fn width_of(widget: &dyn Widget) -> f32 {
    match widget.width() {
        Size::Fixed(width) => width,
        Size::Parent => 0.0,
    }
}

/// The accent while something is connected, else secondary.
fn status_color(connected: bool) -> Color {
    if connected {
        sys::color::accent::bg()
    } else {
        material::label(Level::Secondary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolling_steps_between_workspaces_and_stops_at_the_ends() {
        assert_eq!(stepped(Some(1), 3, 1), Some(2));
        assert_eq!(stepped(Some(1), 3, -1), Some(0));
        assert_eq!(stepped(Some(2), 3, 1), None);
        assert_eq!(stepped(Some(0), 3, -1), None);
        assert_eq!(stepped(None, 3, 1), None);
    }
}
