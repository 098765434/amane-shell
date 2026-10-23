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
mod power;
mod wifi;

use std::cell::Cell;
use std::path::PathBuf;

use amane::{
    Apps, Audio, Battery, Button, Center, Color, Column, End, Full, Gestures, Image, Keyboard,
    Layer, LayerWindow, Network, Padding, Parent, Point, Pointer, Rectangle, Row, Scroll, Service,
    Size, Stack, Start, Text, Vertical, WarningLevel, Widget, WindowInfo, Windows, Workspace,
    Workspaces, Zone,
};

use crate::components::context_menu::{self, ContextMenu, MenuItem};

use crate::tokens::{Styled, bar, sys};
use clock::Clock;

pub use power::{POWER_NS, panel as power_panel};
pub use wifi::{WIFI_NS, panel as wifi_panel};

// Material Symbols codepoints (the font stack is token `sys.type.icon`).
pub(crate) const ICON_APPS: char = '\u{e5c3}';
pub(crate) const ICON_VOLUME_UP: char = '\u{e050}';
pub(crate) const ICON_VOLUME_OFF: char = '\u{e04f}';
pub(crate) const ICON_WIFI: char = '\u{e63e}';
pub(crate) const ICON_WIFI_OFF: char = '\u{e648}';
pub(crate) const ICON_LOCK: char = '\u{e897}';
pub(crate) const ICON_CHECK: char = '\u{e5ca}';
pub(crate) const ICON_REFRESH: char = '\u{e5d5}';
pub(crate) const ICON_MOUSE: char = '\u{e323}';
pub(crate) const ICON_KEYBOARD: char = '\u{e312}';
pub(crate) const ICON_HEADSET: char = '\u{e310}';
pub(crate) const ICON_GAMEPAD: char = '\u{ea28}';

/// Horizontal batteries (`battery_android_0` … `_6`, `_full`, `_bolt`), the
/// macOS shape; the fill shows the charge.
const ICON_BATTERY_LEVELS: [char; 7] = [
    '\u{f30d}', '\u{f30c}', '\u{f30b}', '\u{f30a}', '\u{f309}', '\u{f308}', '\u{f307}',
];
const ICON_BATTERY_FULL: char = '\u{f304}';
const ICON_BATTERY_BOLT: char = '\u{f305}';

/// The battery glyph for a charge: a bolt while charging, otherwise filled to
/// the nearest of the glyph's six bars (full from 95 %).
pub(crate) fn battery_glyph(percent: u8, charging: bool) -> char {
    if charging {
        ICON_BATTERY_BOLT
    } else if percent >= 95 {
        ICON_BATTERY_FULL
    } else {
        let bars = (usize::from(percent) * 6 + 50) / 100;
        ICON_BATTERY_LEVELS[bars.min(6)]
    }
}

/// A Material-Symbols glyph as a `Text` widget (centered on its ink).
pub(crate) fn icon(glyph: char, size: f32, color: Color) -> Text {
    Text::new(glyph.to_string())
        .font(sys::r#type::icon())
        .size(size)
        .color(color)
        .tight()
}

/// The taskbar's layer-surface namespace. The niri `layer-rule` matches this
/// for blur + corner rounding, and menus opened from a window pill hang under
/// it (see [`open_window_menu`]).
pub const BAR_NS: &str = "amane.taskbar";

/// Registry key for the dynamically-rebuilt per-window menu.
const WINDOW_MENU_KEY: &str = "amane.taskbar.window-menu";

thread_local! {
    // Last pointer x within the bar surface (surface-local, from the panel's
    // on_move). A right-click handler only gets a Button, so it reads this to
    // know where to anchor the window menu.
    static CURSOR_X: Cell<i32> = const { Cell::new(0) };
}

fn set_cursor_x(x: i32) {
    CURSOR_X.with(|c| c.set(x));
}

fn cursor_x() -> i32 {
    CURSOR_X.with(|c| c.get())
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
        boxed(
            Row::new(vec![boxed(left_section())])
                .width(Parent)
                .height(Parent)
                .justify(Start)
                .align(Center),
        ),
        boxed(
            Row::new(vec![center_section(overview)])
                .width(Parent)
                .height(Parent)
                .justify(Center)
                .align(Center),
        ),
        boxed(
            Row::new(vec![boxed(right_section())])
                .width(Parent)
                .height(Parent)
                .justify(End)
                .align(Center),
        ),
    ])
    .width(Parent)
    .height(Parent);

    let panel = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .radius(bar::radius())
        .fill(sys::color::surface::panel::bg())
        .blur(sys::effect::backdrop_blur())
        .padding(pad(sys::space::inset::m(), bar::panel::padding_y()))
        .on_move(|p: Point| set_cursor_x(p.x as i32))
        .child(sections);

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
    let items: Vec<Box<dyn Widget>> = vec![
        volume_module(),
        network_indicator(),
        battery_module(),
        clock_module(),
    ];

    Row::new(items).gap(sys::space::gap::xl()).align(Center)
}

// ───── left: overview button + workspace pips ────────────────────────────────

fn overview_button() -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(bar::indicator::width()))
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(|button| {
            if button == Button::Left {
                Windows::toggle_overview();
            }
        })
        .child(icon(
            ICON_APPS,
            sys::size::icon::m(),
            sys::color::content::secondary(),
        ))
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

    boxed(
        Rectangle::new()
            .width(Size::Fixed(bar::window_item::max_width() * 2.0))
            .height(Parent)
            .fill(Color::TRANSPARENT)
            .align_child(Center, Center)
            .clip()
            .child(
                Text::new(text)
                    .style(sys::r#type::title())
                    .color(sys::color::surface::panel::fg())
                    .elide(),
            ),
    )
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
    let heading = if app.is_empty() {
        title.clone()
    } else {
        app.clone()
    };

    let side = sys::size::icon::m() * 2.0;
    let text_w = (bar::window_item::max_width()
        - side
        - sys::space::gap::s()
        - sys::space::inset::m() * 2.0)
        .max(1.0);

    let label = Column::new(vec![
        boxed(
            Text::new(heading)
                .style(sys::r#type::title())
                .color(sys::color::surface::panel::fg())
                .elide(),
        ),
        boxed(
            Text::new(title.clone())
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
fn app_icon_tile(app: &str, title: &str, side: f32) -> Rectangle {
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
fn icon_for(app_id: &str) -> Option<PathBuf> {
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

    context_menu::register(WINDOW_MENU_KEY, menu);
    context_menu::open_under(WINDOW_MENU_KEY, BAR_NS, cursor_x(), 0, 1, 1);
}

// ───── right: status cluster ─────────────────────────────────────────────────

fn volume_module() -> Box<dyn Widget> {
    let audio = Audio::read();
    let muted = audio.muted();
    let volume = audio.volume();
    drop(audio);

    let glyph = if muted {
        ICON_VOLUME_OFF
    } else {
        ICON_VOLUME_UP
    };

    let content = Row::new(vec![
        boxed(icon(
            glyph,
            sys::size::icon::m(),
            sys::color::content::secondary(),
        )),
        boxed(
            Text::new(format!("{volume}%"))
                .style(sys::r#type::label())
                .color(sys::color::content::secondary()),
        ),
    ])
    .gap(sys::space::gap::xs())
    .align(Center);
    boxed(
        Rectangle::new()
            .width(Size::Fixed(bar::volume::width()))
            .height(Parent)
            .radius(sys::radius::raised())
            .fill(sys::color::surface::raised::bg())
            .padding(pad(sys::space::inset::m(), bar::pill::padding_y()))
            .align_child(Center, Center)
            .cursor(Pointer)
            .on_scroll(|scroll: Scroll| {
                let current = Audio::read().volume();
                // scroll up (negative y) raises, down lowers, in 5% steps
                let next = if scroll.y < 0.0 {
                    current.saturating_add(5).min(100)
                } else {
                    current.saturating_sub(5)
                };
                Audio::set_volume(next);
            })
            .on_click(|button| {
                if button == Button::Left {
                    Audio::toggle_mute();
                }
            })
            .child(content),
    )
}

fn network_indicator() -> Box<dyn Widget> {
    let net = Network::read();
    let connected = net.connected();
    let wifi_on = net.wifi_enabled();
    drop(net);

    let glyph = if connected || wifi_on {
        ICON_WIFI
    } else {
        ICON_WIFI_OFF
    };
    let color = if connected {
        sys::color::accent::bg()
    } else {
        sys::color::content::secondary()
    };

    boxed(
        Rectangle::new()
            .width(Size::Fixed(bar::indicator::width()))
            .height(Parent)
            .fill(Color::TRANSPARENT)
            .align_child(Center, Center)
            .cursor(Pointer)
            .on_click(|button| {
                if button == Button::Left {
                    power::close();
                    wifi::toggle();
                }
            })
            .child(icon(glyph, sys::size::icon::m(), color)),
    )
}

fn battery_module() -> Box<dyn Widget> {
    let battery = Battery::read();

    if !battery.present() {
        return boxed(Row::new(Vec::new()));
    }

    let pct = battery.percent();
    let on_ac = battery.on_ac();
    // UPower's warning level; the sysfs fallback has none, so keep a threshold there
    let low = battery.warning() >= WarningLevel::Low || (pct <= 15 && !on_ac);
    drop(battery);

    let color = if on_ac {
        sys::color::accent::bg()
    } else if low {
        sys::color::status::critical::bg()
    } else {
        sys::color::content::secondary()
    };

    let content: Vec<Box<dyn Widget>> = vec![
        boxed(icon(battery_glyph(pct, on_ac), sys::size::icon::m(), color)),
        boxed(
            Text::new(format!("{pct}%"))
                .style(sys::r#type::label())
                .color(color),
        ),
    ];

    boxed(
        Rectangle::new()
            // room for the battery glyph and "100%"
            .width(Size::Fixed(
                sys::size::icon::m() + sys::r#type::label().size * 2.8,
            ))
            .height(Parent)
            .fill(Color::TRANSPARENT)
            .align_child(Center, Center)
            .cursor(Pointer)
            .on_click(|button| {
                if button == Button::Left {
                    wifi::close();
                    power::toggle();
                }
            })
            .child(Row::new(content).gap(sys::space::gap::_2xs()).align(Center)),
    )
}

fn clock_module() -> Box<dyn Widget> {
    boxed(
        Text::new(Clock::read().text().to_string())
            .style(sys::r#type::numeric())
            .color(sys::color::surface::panel::fg()),
    )
}

// ───── helpers ───────────────────────────────────────────────────────────────

pub(crate) fn boxed(widget: impl Widget + 'static) -> Box<dyn Widget> {
    Box::new(widget)
}

pub(crate) fn pad(x: f32, y: f32) -> Padding {
    Padding {
        top: y,
        right: x,
        bottom: y,
        left: x,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_glyph_follows_the_charge() {
        assert_eq!(battery_glyph(0, false), ICON_BATTERY_LEVELS[0]);
        assert_eq!(battery_glyph(18, false), ICON_BATTERY_LEVELS[1]);
        assert_eq!(battery_glyph(50, false), ICON_BATTERY_LEVELS[3]);
        assert_eq!(battery_glyph(94, false), ICON_BATTERY_LEVELS[6]);
        assert_eq!(battery_glyph(95, false), ICON_BATTERY_FULL);
        assert_eq!(battery_glyph(100, false), ICON_BATTERY_FULL);
        assert_eq!(battery_glyph(18, true), ICON_BATTERY_BOLT);
    }
}
