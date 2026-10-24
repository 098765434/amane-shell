//! A bottom taskbar built on the Amane fork.
//!
//! Compact and ambient in normal use (matching the previous Waybar's size
//! and palette), it **expands into an interactive dock while niri's overview
//! is open** — the fork's `Windows::overview()` drives the switch. It wires
//! in the fork's smart features: the `Windows` service (list + focused +
//! overview), window actions (focus/close/fullscreen/float/move/overview),
//! `Workspaces`, and the reusable [`context_menu`] (right-click a window).
//!
//! Visual language is macOS: a translucent frosted panel (backdrop blur on
//! the root rectangle), rounded pills, and generous spacing. Everything is
//! driven by the Penpot tokens in `design-tokens/tokens.json` via [`tokens`].

mod clock;
mod indicators;
mod panel;

use std::path::PathBuf;

use amane::{
    Apps, Audio, Battery, Bluetooth, BluetoothDevice, Button, Center, Color, Column, End, Full,
    Gestures, Image, Justify, Keyboard, Layer, LayerWindow, Link, Network, Parent, Pointer,
    PowerProfile, Rectangle, Row, Scroll, Service, Size, Stack, Start, Text, Vertical,
    WarningLevel, Widget, WindowInfo, Windows, Workspace, Workspaces, Zone,
};

use crate::components::buttons::{IconButton, PushButton, Shape, Style};
use crate::components::context_menu::{self, ContextMenu, MenuItem};
use crate::components::icons::{
    Glyph, ICON_AIRPLANE, ICON_APPS, ICON_BLUETOOTH, ICON_BLUETOOTH_CONNECTED, ICON_BLUETOOTH_OFF,
    ICON_CABLE, ICON_CAMERA, ICON_DND, ICON_MIC, ICON_NIGHT_LIGHT, ICON_NOTIFICATIONS,
    ICON_SCREEN_SHARE, ICON_TUNE, ICON_VOLUME_OFF, ICON_VPN, ICON_WIFI, ICON_WIFI_NONE,
    ICON_WIFI_OFF, audio_glyph, battery_glyph, power_profile_glyph, wifi_glyph,
};
use crate::components::layout::{
    boxed, clipped_label, cursor, nothing, pad, text_width, track_cursor,
};
use crate::components::notifications::{self, DoNotDisturb};
use crate::components::quick_settings::{self, Page};
use crate::components::slider;
use crate::services::connections::Connections;
use crate::services::night_light::NightLight;
use crate::services::privacy::Privacy;
use crate::services::rfkill::Rfkill;
use crate::services::tailscale::{State as TailscaleState, Tailscale};

use crate::tokens::{Styled, bar, sys};

pub(crate) use clock::Clock;
pub(crate) use indicators::Indicator;
pub(crate) use panel::{Panel, close, dismissable, is_open, overlay, popover_margin, toggle};

pub use indicators::ipc;

/// The taskbar's layer-surface namespace. The niri `layer-rule` matches this
/// for blur + corner rounding, and menus opened from a window pill hang under
/// it (see [`open_window_menu`]).
pub const BAR_NS: &str = "amane.taskbar";

/// Registry key for the dynamically-rebuilt per-window menu.
const WINDOW_MENU_KEY: &str = "amane.taskbar.window-menu";

/// At or under this charge a battery reads low, where UPower gives no
/// warning level of its own (sysfs, peripherals).
pub(crate) const LOW_BATTERY: u8 = 15;

/// Whether the machine's battery is low: UPower's warning level, or the
/// threshold where the sysfs fallback has none.
pub(crate) fn battery_low(battery: &Battery) -> bool {
    battery.warning() >= WarningLevel::Low || (battery.percent() <= LOW_BATTERY && !battery.on_ac())
}

/// `normal`, or the critical colour while a battery at `percent` is low and
/// not charging.
pub(crate) fn battery_color(percent: u8, charging: bool, normal: Color) -> Color {
    if percent <= LOW_BATTERY && !charging {
        sys::color::status::critical::bg()
    } else {
        normal
    }
}

/// The accent while something is connected, else secondary.
fn status_color(connected: bool) -> Color {
    if connected {
        sys::color::accent::bg()
    } else {
        sys::color::content::secondary()
    }
}

/// The taskbar view. Registered with `App::window`.
pub fn bar() -> LayerWindow {
    let overview = Windows::read().overview();

    // The overview open/close progress (0..1) from niri drives a smooth height
    // grow, in lockstep with the compositor's own overview zoom. The `overview`
    // bool still decides the center content swap — it flips when the gesture
    // commits, so the dock appears/disappears cleanly while the height eases.
    let progress = Gestures::read().overview_progress().clamp(0.0, 1.0) as f32;
    let height = sys::size::panel::height()
        + (bar::overview::height() - sys::size::panel::height()) * progress;

    // Three zones via a Stack: each layer spans the full width and justifies
    // its own content, so the center never shifts when the sides differ.
    let sections = Stack::new(vec![
        zone(boxed(left_section()), Start.into()),
        zone(center_section(overview), Center.into()),
        zone(boxed(right_section()), End.into()),
    ])
    .width(Parent)
    .height(Parent);

    let panel = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .radius(bar::radius())
        .fill(sys::color::surface::panel::bg())
        .blur(sys::effect::backdrop_blur())
        .padding(pad(sys::space::inset::m(), bar::panel::padding_y()));
    let panel = track_cursor(panel).child(sections);

    LayerWindow::new()
        .width(Full)
        .height(height)
        .anchor_vertical(Vertical::Bottom)
        .layer(Layer::Top)
        .space(Zone::Reserve)
        // None so bar clicks never steal keyboard focus from the active app
        // (macOS menu-bar behavior). Window menus opened from a pill are
        // mouse-driven; the popup's pointer grab still dismisses on
        // outside-click.
        .keyboard(Keyboard::None)
        .namespace(BAR_NS)
        .visible(true)
        .child(panel)
}

// ───── sections ─────────────────────────────────────────────────────────────

/// One of the bar's three full-width layers, its content justified.
fn zone(content: Box<dyn Widget>, justify: Justify) -> Box<dyn Widget> {
    boxed(
        Row::new(vec![content])
            .width(Parent)
            .height(Parent)
            .justify(justify)
            .align(Center),
    )
}

fn left_section() -> Row {
    let mut items: Vec<Box<dyn Widget>> = vec![boxed(overview_button())];

    let workspaces = Workspaces::read();
    for workspace in workspaces.list() {
        items.push(boxed(workspace_pip(workspace)));
    }

    Row::new(items).gap(sys::space::gap::s()).align(Center)
}

fn center_section(overview: bool) -> Box<dyn Widget> {
    if overview {
        window_dock()
    } else {
        focused_title()
    }
}

fn right_section() -> Row {
    let mut items: Vec<Box<dyn Widget>> = privacy_indicators();
    items.extend(status_indicators());
    items.extend([
        volume_module(),
        bluetooth_indicator(),
        network_indicator(),
        tailscale_indicator(),
        battery_module(),
        quick_settings_button(),
        notification_indicator(),
        clock_module(),
    ]);

    Row::new(items).gap(sys::space::gap::xl()).align(Center)
}

// ───── left: overview button + workspace pips ────────────────────────────────

fn overview_button() -> Rectangle {
    bar_icon(
        "amane.bar.overview",
        ICON_APPS,
        sys::color::content::secondary(),
    )
    .on_press(Windows::toggle_overview)
    .build()
}

fn workspace_pip(ws: &Workspace) -> Rectangle {
    let id = ws.id();
    let lit = ws.active() || ws.focused();

    let color = if ws.urgent() {
        sys::color::status::critical::bg()
    } else if lit {
        sys::color::surface::panel::fg()
    } else {
        sys::color::content::secondary()
    };

    let width = if lit {
        bar::workspace_dot::active_size()
    } else {
        bar::workspace_dot::size()
    };

    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(bar::workspace_dot::size()))
        .radius(bar::workspace_dot::size() / 2.0)
        .fill(color)
        .cursor(Pointer)
        .on_click(move |button| {
            if button == Button::Left {
                Workspaces::focus(id);
            }
        })
}

// ───── center: focused title (compact) / window dock (overview) ──────────────

fn focused_title() -> Box<dyn Widget> {
    let windows = Windows::read();
    let text = windows
        .focused()
        .map(|win| win.title().or(win.app_id()).unwrap_or("").to_string())
        .unwrap_or_default();

    boxed(clipped_label(
        text,
        sys::r#type::title(),
        sys::color::surface::panel::fg(),
        bar::window_item::max_width() * 2.0,
        Center,
    ))
}

fn window_dock() -> Box<dyn Widget> {
    let windows = Windows::read();
    let tiles: Vec<Box<dyn Widget>> = windows
        .list()
        .iter()
        .map(|win| boxed(window_tile(win)))
        .collect();

    boxed(Row::new(tiles).gap(sys::space::gap::s()).align(Center))
}

fn window_tile(win: &WindowInfo) -> Rectangle {
    let id = win.id();
    let floating = win.floating();

    let fill = if win.focused() {
        sys::color::state::selected()
    } else {
        sys::color::surface::raised::bg()
    };

    let app = win.app_id().unwrap_or("").to_string();
    let title = win.title().unwrap_or("").to_string();
    let heading = if app.is_empty() { &title } else { &app };

    let side = sys::size::icon::m() * 2.0;
    let text_w = (bar::window_item::max_width()
        - side
        - sys::space::gap::s()
        - sys::space::inset::m() * 2.0)
        .max(1.0);

    let label = Column::new(vec![
        boxed(
            Text::new(heading.as_str())
                .style(sys::r#type::title())
                .color(sys::color::surface::panel::fg())
                .elide(),
        ),
        boxed(
            Text::new(title.as_str())
                .style(sys::r#type::caption())
                .color(sys::color::content::secondary())
                .elide(),
        ),
    ])
    .gap(sys::space::gap::_2xs())
    .width(Size::Fixed(text_w));

    let row = Row::new(vec![boxed(app_icon_tile(&app, &title, side)), boxed(label)])
        .gap(sys::space::gap::s())
        .align(Center);

    Rectangle::new()
        .width(Size::Fixed(bar::window_item::max_width()))
        .height(Parent)
        .radius(bar::window_item::radius())
        .fill(fill)
        .padding(sys::space::inset::m())
        .align_child(Start, Center)
        .clip()
        .cursor(Pointer)
        .on_click(move |button| match button {
            Button::Left => Windows::focus(id),
            Button::Right => open_window_menu(id, floating),
            _ => {}
        })
        .child(row)
}

/// A window's app icon, or a first-letter placeholder when none resolves.
pub(crate) fn app_icon_tile(app: &str, title: &str, side: f32) -> Rectangle {
    if let Some(path) = icon_for(app) {
        // `loaded` kicks off the async decode and wakes us when ready; until
        // then fall through to the placeholder for this frame
        if Image::loaded(&path) {
            return Rectangle::new()
                .width(Size::Fixed(side))
                .height(Size::Fixed(side))
                .fill(Image::contain(path));
        }
    }

    Rectangle::new()
        .width(Size::Fixed(side))
        .height(Size::Fixed(side))
        .radius(sys::radius::raised())
        .fill(sys::color::surface::raised::bg())
        .align_child(Center, Center)
        .child(
            Text::new(first_letter(app, title))
                .style(sys::r#type::title())
                .color(sys::color::surface::panel::fg())
                .tight(),
        )
}

/// Resolve a niri window app_id to a desktop-entry icon file, matching the
/// id (and its last dotted segment) against each app's icon name, exec
/// basename, or name — case-insensitively.
pub(crate) fn icon_for(app_id: &str) -> Option<PathBuf> {
    if app_id.is_empty() {
        return None;
    }

    let apps = Apps::read();
    let needle = app_id.to_lowercase();
    let last = needle.rsplit('.').next().unwrap_or(&needle).to_string();

    let matched = apps.list().iter().find(|app| {
        let icon = app.icon().unwrap_or("").to_lowercase();
        let name = app.name().to_lowercase();
        let exec_base = app
            .exec()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_lowercase();

        [icon, name, exec_base]
            .iter()
            .any(|field| *field == needle || *field == last)
    });

    matched.and_then(|app| app.icon_path().map(|path| path.to_path_buf()))
}

fn first_letter(app: &str, title: &str) -> String {
    let base = app
        .rsplit('.')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(app);
    let source = if base.is_empty() { title } else { base };

    source
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// Build a per-window menu bound to `id` and open it anchored to the bar at
/// the last cursor x. The bottom-anchored parent makes it open upward.
fn open_window_menu(id: u64, floating: bool) {
    let mut move_to = ContextMenu::new();
    {
        let workspaces = Workspaces::read();
        for workspace in workspaces.list() {
            let target = workspace.id();
            let name = workspace
                .name()
                .map(String::from)
                .unwrap_or_else(|| format!("Workspace {}", workspace.index()));
            move_to = move_to.item(
                MenuItem::label(name).on_select(move || Windows::move_to_workspace(id, target)),
            );
        }
    }

    let float_label = if floating {
        "Move to Tiling"
    } else {
        "Move to Floating"
    };

    let menu = ContextMenu::new()
        .item(MenuItem::label("Focus").on_select(move || Windows::focus(id)))
        .item(MenuItem::label("Toggle Fullscreen").on_select(move || Windows::fullscreen(id)))
        .item(MenuItem::label(float_label).on_select(move || Windows::toggle_floating(id)))
        .item(MenuItem::label("Move to Workspace").submenu(move_to))
        .separator()
        .item(
            MenuItem::label("Close")
                .destructive()
                .on_select(move || Windows::close(id)),
        );

    context_menu::show_under(WINDOW_MENU_KEY, menu, BAR_NS, (cursor().0, 0));
}

// ───── right: status cluster ─────────────────────────────────────────────────

fn volume_module() -> Box<dyn Widget> {
    if !Indicator::Volume.shown() {
        return nothing();
    }
    let audio = Audio::read();
    let muted = audio.muted();
    let volume = audio.volume();
    let kind = audio.output_kind();
    drop(audio);

    // what is playing through: speakers, headphones, a headset, the TV …
    let glyph = if muted {
        ICON_VOLUME_OFF
    } else {
        audio_glyph(kind, false)
    };
    let tip = if muted {
        String::from("Muted")
    } else {
        format!("Volume {volume}%")
    };

    boxed(
        bar_icon("amane.bar.volume", glyph, sys::color::content::secondary())
            .on_scroll(move |scroll: Scroll| {
                Audio::set_volume(slider::scrolled(volume, scroll, slider::STEP))
            })
            .on_press(|| quick_settings::open(Page::Sound))
            .tooltip(BAR_NS, tip)
            .build(),
    )
}

fn bluetooth_indicator() -> Box<dyn Widget> {
    let bluetooth = Bluetooth::read();

    // no adapter, nothing to control
    if !bluetooth.available() || !Indicator::Bluetooth.shown() {
        return nothing();
    }

    let powered = bluetooth.powered();
    let connected = bluetooth.devices().iter().any(BluetoothDevice::connected);
    drop(bluetooth);

    let glyph = match (powered, connected) {
        (false, _) => ICON_BLUETOOTH_OFF,
        (true, true) => ICON_BLUETOOTH_CONNECTED,
        (true, false) => ICON_BLUETOOTH,
    };
    boxed(
        bar_icon("amane.bar.bluetooth", glyph, status_color(connected))
            .on_press(|| quick_settings::open(Page::Bluetooth))
            .build(),
    )
}

fn network_indicator() -> Box<dyn Widget> {
    if !Indicator::Network.shown() {
        return nothing();
    }
    let net = Network::read();
    let link = net.link();
    let strength = net.strength();
    let wifi_on = net.wifi_enabled();
    drop(net);

    let glyph = match link {
        Link::Wifi => wifi_glyph(strength),
        Link::Wired => ICON_CABLE,
        // vpn and the rest carry no strength to show
        Link::Other => ICON_WIFI,
        Link::Offline if wifi_on => ICON_WIFI_NONE,
        Link::Offline => ICON_WIFI_OFF,
    };
    boxed(
        // grey like its neighbours: the glyph already shows the link
        bar_icon("amane.bar.network", glyph, sys::color::content::secondary())
            .on_press(|| quick_settings::open(Page::Wifi))
            .build(),
    )
}

/// Tailscale's mark while connected; accent while routing through an
/// exit node.
fn tailscale_indicator() -> Box<dyn Widget> {
    let tailscale = Tailscale::read();
    if tailscale.state() != TailscaleState::Running || !Indicator::Tailscale.shown() {
        return nothing();
    }
    let routed = tailscale.exit_node().is_some();
    let tip = format!("Tailscale · {}", tailscale.describe());
    drop(tailscale);

    boxed(
        bar_icon(
            "amane.bar.tailscale",
            Glyph::Tailscale,
            status_color(routed),
        )
        .on_press(|| quick_settings::open(Page::Tailscale))
        .tooltip(BAR_NS, tip)
        .build(),
    )
}

fn battery_module() -> Box<dyn Widget> {
    let battery = Battery::read();

    if !battery.present() || !Indicator::Battery.shown() {
        return nothing();
    }

    let pct = battery.percent();
    let on_ac = battery.on_ac();
    let low = battery_low(&battery);
    drop(battery);

    let color = if on_ac {
        sys::color::accent::bg()
    } else if low {
        sys::color::status::critical::bg()
    } else {
        sys::color::content::secondary()
    };
    let tip = if on_ac {
        format!("Battery {pct}% · charging")
    } else {
        format!("Battery {pct}%")
    };

    boxed(
        bar_icon("amane.bar.battery", battery_glyph(pct, on_ac), color)
            .on_press(|| quick_settings::open(Page::Power))
            .tooltip(BAR_NS, tip)
            .build(),
    )
}

/// Quick Settings' button, the macOS Control Center's place in the bar.
fn quick_settings_button() -> Box<dyn Widget> {
    let open = is_open(Panel::QuickSettings);
    boxed(
        bar_icon(
            "amane.bar.quick-settings",
            ICON_TUNE,
            sys::color::content::secondary(),
        )
        .selected(open)
        .on_press(|| panel::toggle(Panel::QuickSettings))
        .tooltip(BAR_NS, "Quick Settings")
        .build(),
    )
}

/// What's using the camera, the microphone or the screen, first in the
/// row as GNOME has them; each opens Quick Settings.
fn privacy_indicators() -> Vec<Box<dyn Widget>> {
    let privacy = Privacy::read();
    let mut items = Vec::new();

    if privacy.screen() {
        items.push(privacy_icon(
            "amane.bar.privacy.screen",
            ICON_SCREEN_SHARE,
            String::from("Screen is being shared"),
        ));
    }
    if privacy.camera() {
        items.push(privacy_icon(
            "amane.bar.privacy.camera",
            ICON_CAMERA,
            String::from("Camera in use"),
        ));
    }
    if privacy.recording() {
        items.push(privacy_icon(
            "amane.bar.privacy.microphone",
            ICON_MIC,
            format!("Microphone in use by {}", privacy.microphone().join(", ")),
        ));
    }
    items
}

fn privacy_icon(id: &'static str, glyph: char, tip: String) -> Box<dyn Widget> {
    boxed(
        bar_icon(id, glyph, sys::color::status::critical::bg())
            .on_press(|| quick_settings::open(Page::Grid))
            .tooltip(BAR_NS, tip)
            .build(),
    )
}

/// Settings GNOME shows in the bar only while they're on: night light, a
/// VPN, a power mode other than Balanced, airplane mode.
fn status_indicators() -> Vec<Box<dyn Widget>> {
    let mut glyphs: Vec<(&'static str, char, &'static str)> = Vec::new();

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

    glyphs
        .into_iter()
        .map(|(id, glyph, tip)| {
            boxed(
                bar_icon(id, glyph, sys::color::content::secondary())
                    .on_press(|| quick_settings::open(Page::Grid))
                    .tooltip(BAR_NS, tip)
                    .build(),
            )
        })
        .collect()
}

/// A moon while Do Not Disturb is on, else a bell while something arrived
/// unseen; nothing otherwise. Click opens the center, right-click picks
/// how long Do Not Disturb lasts.
fn notification_indicator() -> Box<dyn Widget> {
    let dnd = DoNotDisturb::read().on();
    let unseen = notifications::unseen();

    let (glyph, color) = match (dnd, unseen) {
        (true, _) => (ICON_DND, sys::color::surface::panel::fg()),
        (false, true) => (ICON_NOTIFICATIONS, sys::color::accent::bg()),
        (false, false) => return nothing(),
    };

    boxed(
        bar_icon("amane.bar.notifications", glyph, color)
            .on_press(|| panel::toggle(Panel::Notifications))
            .on_secondary(|| DoNotDisturb::open_menu(BAR_NS, (cursor().0, 0)))
            .build(),
    )
}

/// The clock opens the notification center, like macOS's.
fn clock_module() -> Box<dyn Widget> {
    let time = Clock::read().text().to_string();
    // as wide as the time, so the bar's spacing stays as it was
    let width = text_width(&time, sys::r#type::numeric());

    boxed(
        PushButton::new("amane.bar.clock", time)
            .style(Style::Plain)
            .text(sys::r#type::numeric())
            .width(width)
            .height(Parent)
            .on_press(|| panel::toggle(Panel::Notifications))
            .build(),
    )
}

// ───── helpers ───────────────────────────────────────────────────────────────

/// A bar item that's a lone glyph: unfilled until hovered, the bar's height.
fn bar_icon(id: &'static str, glyph: impl Into<Glyph>, color: Color) -> IconButton {
    IconButton::new(id, glyph)
        .shape(Shape::Square)
        .color(color)
        .width(bar::indicator::width())
        .height(Parent)
}
