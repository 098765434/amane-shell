//! A status bar along the bottom (or top) edge, built on the Amane fork.
//!
//! Compact and ambient (matching the previous Waybar's size and palette):
//! workspaces on the left and the status cluster on the right. The open
//! apps are the [`dock`](crate::surfaces::dock)'s, which rises over the
//! bar from the bottom edge and while niri's overview is open.
//!
//! Every item is a lone glyph (or a workspace's pip) that turns into a white
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

mod indicators;

use std::cell::{Cell, RefCell};
use std::time::Duration;

use amane::{
    Animation, Audio, Battery, Bluetooth, BluetoothDevice, Center, Color, Easing, Full, InputArea,
    Justify, Keyboard, Layer, LayerWindow, Link, Network, Parent, PowerProfile, Rectangle, Row,
    Scroll, ScrollSource, Service, Size, Start, Vertical, Widget, Workspace, Workspaces, Zone,
    window_size,
};
use amane_kit::settings::{BarPosition, BarStyle, status_bar};
use amane_kit::theme::{self, Mode};

use crate::components::battery;
use crate::components::buttons::{
    ButtonBuilder, DrawnButton, IconButton, PushButton, Shape, Style,
};
use crate::components::edge_reveal::EdgeReveal;
use crate::components::icons::{
    Glyph, ICON_AIRPLANE, ICON_BLUETOOTH, ICON_BLUETOOTH_CONNECTED, ICON_BLUETOOTH_OFF, ICON_CABLE,
    ICON_CAMERA, ICON_DND, ICON_KEEP_AWAKE, ICON_MIC, ICON_NIGHT_LIGHT, ICON_NOTIFICATIONS,
    ICON_SCREEN_SHARE, ICON_TIMER, ICON_TUNE, ICON_VOLUME_OFF, ICON_VPN, ICON_WIFI, ICON_WIFI_NONE,
    ICON_WIFI_OFF, audio_glyph, battery_glyph, power_profile_glyph, wifi_glyph,
};
use crate::surfaces::launcher;
use crate::components::layout::{boxed, cursor, fixed_width, pad, text_width, track_cursor};
use crate::components::material::{self, Level, Material};
use crate::components::motion;
use crate::surfaces::notifications::{self, DoNotDisturb};
use crate::components::panel::{self, Panel, is_open};
use crate::surfaces::quick_settings::{self, Page};
use crate::components::slider;
use crate::services::launcher::clock::Clock;
use crate::services::network::connections::Connections;
use crate::services::power::keep_awake::KeepAwake;
use crate::services::display::night_light::NightLight;
use crate::services::desktop::privacy::Privacy;
use crate::services::network::rfkill::Rfkill;
use crate::services::network::tailscale::{State as TailscaleState, Tailscale};
use crate::services::launcher::timer::Countdown;

use crate::tokens::{bar, sys};

pub(crate) use indicators::Indicator;

pub use indicators::{ipc, migrate};

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

    // where the hiding bar is, when it hides
    static REVEAL: RefCell<EdgeReveal> = RefCell::new(EdgeReveal::new(
        Animation::new(0.0)
            .duration(if theme::reduced_motion() {
                Duration::ZERO
            } else {
                Duration::from_millis(220)
            })
            .easing(Easing::Out),
        Duration::from_millis(400),
    ));
}

/// The edge the bar runs along; the panels and cards that keep clear of
/// it ask here.
pub fn edge() -> Vertical {
    match status_bar().position {
        BarPosition::Bottom => Vertical::Bottom,
        BarPosition::Top => Vertical::Top,
    }
}

/// The status bar view. Registered with `App::window`.
pub fn bar() -> LayerWindow {
    let left: Box<dyn Widget> = if status_bar().show_workspaces {
        boxed(workspaces())
    } else {
        boxed(Row::new(Vec::new()))
    };
    let sections = Row::new(vec![left, boxed(status())])
        .width(Parent)
        .height(Parent)
        .justify(Justify::SpaceBetween)
        .align(Center);

    let style = status_bar().style;
    let height = sys::size::panel::height();
    let (width, _) = window_size();

    // how far it's slid past its edge: 0 in place, `height` gone
    let hiding = status_bar().auto_hide;
    let (offset, input) = REVEAL.with_borrow_mut(|reveal| {
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
        .padding(pad(sys::space::inset::m(), bar::panel::padding_y()))
        .translate(0.0, offset);
    let panel = track_cursor(panel).child(sections);
    let panel = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .on_hover(|over| REVEAL.with_borrow_mut(|reveal| reveal.hover(over)))
        // a pointer that arrived without an enter still counts
        .on_move(|_| REVEAL.with_borrow_mut(|reveal| reveal.hover(true)))
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
        // (macOS menu-bar behavior); the panels take keys when clicked into.
        .keyboard(Keyboard::None)
        .namespace(BAR_NS)
        .visible(true)
        .child(panel)
}

/// What the strip is painted with: the bar's tint over niri's frost, the
/// glass's own (none where niri draws it), or black or white.
fn fill(style: BarStyle) -> Color {
    match style {
        BarStyle::Frosted => Material::Bar.tint(),
        BarStyle::Glass => Material::Control.fill(),
        BarStyle::Solid => match theme::mode() {
            Mode::Dark => Color::BLACK,
            Mode::Light => Color::WHITE,
        },
    }
}

// ───── left: workspaces ──────────────────────────────────────────────────────

/// One pip per workspace; scrolling over them moves between workspaces.
fn workspaces() -> Rectangle {
    let pips: Vec<Box<dyn Widget>> = Workspaces::read()
        .list()
        .iter()
        .map(|workspace| boxed(workspace_pip(workspace)))
        .collect();
    let pips = Row::new(pips).gap(sys::space::gap::_2xs()).align(Center);

    Rectangle::new()
        .width(Size::Fixed(fixed_width(&pips)))
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .on_scroll(|scroll| {
            if status_bar().scroll_workspaces {
                scroll_workspaces(scroll);
            }
        })
        .child(pips)
}

/// A dot, widened while its workspace is active or focused, in a bar item's
/// hit area; a click focuses it.
fn workspace_pip(workspace: &Workspace) -> Rectangle {
    let id = workspace.id();
    let lit = workspace.active() || workspace.focused();

    let color = if workspace.urgent() {
        sys::color::status::critical::bg()
    } else if lit {
        material::label(Level::Primary)
    } else {
        material::label(Level::Secondary)
    };
    let width = if lit {
        bar::workspace_dot::active_size()
    } else {
        bar::workspace_dot::size()
    };
    let tip = workspace
        .name()
        .map(String::from)
        .unwrap_or_else(|| format!("Workspace {}", workspace.index()));

    item(DrawnButton::new(
        format!("amane.bar.workspace.{id}"),
        move |color| {
            Rectangle::new()
                .width(Size::Fixed(width))
                .height(Size::Fixed(bar::workspace_dot::size()))
                .radius(bar::workspace_dot::size() / 2.0)
                .fill(color)
        },
    ))
    .width(bar::indicator::width())
    .color(color)
    .on_press(move || Workspaces::focus(id))
    .tooltip(BAR_NS, tip)
    .build()
}

/// A wheel notch, or [`SCROLL_STEP`] lines of touchpad, focuses the next
/// workspace down or the previous one up.
fn scroll_workspaces(scroll: Scroll) {
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
    let list = workspaces.list();
    let focused = list.iter().position(Workspace::focused);
    if let Some(next) = stepped(focused, list.len(), step) {
        let id = list[next].id();
        drop(workspaces);
        Workspaces::focus(id);
    }
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

fn status() -> Row {
    let mut items = Vec::new();
    if Indicator::Privacy.shown() {
        items.extend(privacy_icons());
    }
    if Indicator::StatusIcons.shown() {
        items.extend(on_icons());
    }
    items.extend(
        [
            optional(Indicator::Volume, volume),
            optional(Indicator::Bluetooth, bluetooth),
            optional(Indicator::Network, network),
            optional(Indicator::Tailscale, tailscale),
            optional(Indicator::KeepAwake, keep_awake),
            optional(Indicator::Battery, battery),
            Some(quick_settings_button()),
            optional(Indicator::Notifications, notification_icon),
            timer(),
            Some(clock()),
        ]
        .into_iter()
        .flatten(),
    );

    Row::new(items).gap(sys::space::gap::_2xs()).align(Center)
}

/// `build`'s icon, unless its Quick Settings page hides it.
fn optional(
    indicator: Indicator,
    build: fn() -> Option<Box<dyn Widget>>,
) -> Option<Box<dyn Widget>> {
    indicator.shown().then(build).flatten()
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

/// Quick Settings' button, the macOS Control Center's place in the bar.
fn quick_settings_button() -> Box<dyn Widget> {
    boxed(
        bar_icon(
            "amane.bar.quick-settings",
            ICON_TUNE,
            material::label(Level::Secondary),
        )
        .active(is_open(Panel::QuickSettings))
        .on_press(|| panel::toggle(Panel::QuickSettings))
        .tooltip(BAR_NS, "Quick Settings")
        .build(),
    )
}

/// What's using the camera, the microphone or the screen, first in the
/// row as GNOME has them.
fn privacy_icons() -> Vec<Box<dyn Widget>> {
    let privacy = Privacy::read();
    let mut glyphs = Vec::new();
    if privacy.screen() {
        glyphs.push((
            "amane.bar.privacy.screen",
            ICON_SCREEN_SHARE,
            String::from("Screen is being shared"),
        ));
    }
    if privacy.camera() {
        glyphs.push((
            "amane.bar.privacy.camera",
            ICON_CAMERA,
            String::from("Camera in use"),
        ));
    }
    if privacy.recording() {
        glyphs.push((
            "amane.bar.privacy.microphone",
            ICON_MIC,
            format!("Microphone in use by {}", privacy.microphone().join(", ")),
        ));
    }
    drop(privacy);

    grid_icons(glyphs, sys::color::status::critical::bg())
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
            .on_secondary(|| DoNotDisturb::open_menu(BAR_NS, (cursor().0, menu_edge())))
            .tooltip(BAR_NS, tip)
            .build(),
    ))
}

/// The launcher's timer while it runs, counting down by the second; a
/// click opens the launcher's actions, where it can be cancelled.
fn timer() -> Option<Box<dyn Widget>> {
    let (left, label) = crate::services::launcher::timer::left()?;
    // amane has no timers: a frame comes as each second turns over, asked
    // for once a second however often the bar redraws
    let due = left.as_secs();
    if TIMER_DUE.replace(Some(due)) != Some(due) {
        motion::redraw_after::<Countdown>(
            Duration::from_nanos(u64::from(left.subsec_nanos())) + TICK,
        );
    }
    let text = crate::services::launcher::timer::format(left);
    let width = text_width(&text, sys::r#type::numeric())
        + bar::indicator::width()
        + sys::space::inset::s() * 2.0;
    Some(boxed(
        item(PushButton::new("amane.bar.timer", text))
            .icon(ICON_TIMER)
            .text(sys::r#type::numeric())
            .width(width)
            .on_press(|| {
                launcher::ipc(&[String::from("open"), String::from("actions")]);
            })
            .tooltip(BAR_NS, label)
            .build(),
    ))
}

/// A beat past each second turning over, so the frame finds it turned.
const TICK: Duration = Duration::from_millis(20);

thread_local! {
    // the second the timer's next frame was asked for
    static TIMER_DUE: Cell<Option<u64>> = const { Cell::new(None) };
}

/// The clock opens the notification center, like macOS's; its tooltip is
/// the date.
fn clock() -> Box<dyn Widget> {
    let clock = Clock::read();
    let time = clock.text();
    let date = clock.date();
    drop(clock);
    // as wide as the time, with room for the highlight either side
    let width = text_width(&time, sys::r#type::numeric()) + sys::space::inset::s() * 2.0;

    boxed(
        item(PushButton::new("amane.bar.clock", time))
            .text(sys::r#type::numeric())
            .width(width)
            .active(is_open(Panel::Notifications))
            .on_press(|| panel::toggle(Panel::Notifications))
            .tooltip(BAR_NS, date)
            .build(),
    )
}

/// The bar's edge facing the screen, for a menu to open from: its top
/// along the bottom, its bottom along the top.
fn menu_edge() -> i32 {
    match edge() {
        Vertical::Top => sys::size::panel::height() as i32,
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
    item(IconButton::new(id, glyph))
        .color(color)
        .width(bar::indicator::width())
}

/// A status glyph opening its Quick Settings `page`, and highlighted while
/// that page shows. The grid's are left alone, or opening Quick Settings
/// would light them all.
fn status_icon(
    id: &'static str,
    glyph: impl Into<Glyph>,
    color: Color,
    page: Page,
    tip: impl Into<String>,
) -> IconButton {
    bar_icon(id, glyph, color)
        .active(page != Page::Grid && quick_settings::showing(page))
        .on_press(move || quick_settings::open(page))
        .tooltip(BAR_NS, tip)
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
