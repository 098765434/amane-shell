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
mod theme;
pub(crate) mod tokens;
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

use clock::Clock;
use theme::Theme;
use tokens::{Tokens, bar_tokens, tokens};

pub use power::panel as power_panel;
pub use wifi::panel as wifi_panel;

// Material Symbols codepoints (the font stack is token `sys.type.icon`).
pub(crate) const ICON_APPS: char = '\u{e5c3}';
pub(crate) const ICON_VOLUME_UP: char = '\u{e050}';
pub(crate) const ICON_VOLUME_OFF: char = '\u{e04f}';
pub(crate) const ICON_WIFI: char = '\u{e63e}';
pub(crate) const ICON_WIFI_OFF: char = '\u{e648}';
pub(crate) const ICON_LOCK: char = '\u{e897}';
pub(crate) const ICON_CHECK: char = '\u{e5ca}';
pub(crate) const ICON_REFRESH: char = '\u{e5d5}';
pub(crate) const ICON_BATTERY: char = '\u{e1a4}';
pub(crate) const ICON_BATTERY_CHARGING: char = '\u{e1a3}';
pub(crate) const ICON_MOUSE: char = '\u{e323}';
pub(crate) const ICON_KEYBOARD: char = '\u{e312}';
pub(crate) const ICON_HEADSET: char = '\u{e310}';
pub(crate) const ICON_GAMEPAD: char = '\u{ea28}';

/// A Material-Symbols glyph as a `Text` widget (centered on its ink).
pub(crate) fn icon(glyph: char, size: f32, color: Color, t: &Tokens) -> Text {
    Text::new(glyph.to_string())
        .font(&t.font_icon)
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
    let theme = Theme::auto();
    let t = tokens();
    let b = bar_tokens();
    let overview = Windows::read().overview();

    // The overview open/close progress (0..1) from niri drives a smooth height
    // grow, in lockstep with the compositor's own overview zoom. The `overview`
    // bool still decides the center content swap — it flips when the gesture
    // commits, so the dock appears/disappears cleanly while the height eases.
    let progress = Gestures::read().overview_progress().clamp(0.0, 1.0) as f32;
    let height = t.height_compact + (b.height_overview - t.height_compact) * progress;

    // Three zones via a Stack: each layer spans the full width and justifies
    // its own content, so the center never shifts when the sides differ.
    let sections = Stack::new(vec![
        boxed(
            Row::new(vec![boxed(left_section(theme, t))])
                .width(Parent)
                .height(Parent)
                .justify(Start)
                .align(Center),
        ),
        boxed(
            Row::new(vec![center_section(theme, t, overview)])
                .width(Parent)
                .height(Parent)
                .justify(Center)
                .align(Center),
        ),
        boxed(
            Row::new(vec![boxed(right_section(theme, t))])
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
        .radius(b.radius)
        .fill(theme.surface)
        .blur(t.blur)
        .padding(pad(t.inset_m, b.pad_y))
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

fn left_section(theme: Theme, t: &Tokens) -> Row {
    let mut items: Vec<Box<dyn Widget>> = vec![boxed(overview_button(theme, t))];

    let workspaces = Workspaces::read();
    for workspace in workspaces.list() {
        items.push(boxed(workspace_pip(workspace, theme)));
    }

    Row::new(items).gap(t.gap_s).align(Center)
}

fn center_section(theme: Theme, t: &Tokens, overview: bool) -> Box<dyn Widget> {
    if overview {
        window_dock(theme, t)
    } else {
        focused_title(theme, t)
    }
}

fn right_section(theme: Theme, t: &Tokens) -> Row {
    let items: Vec<Box<dyn Widget>> = vec![
        volume_module(theme, t),
        network_indicator(theme, t),
        battery_module(theme, t),
        clock_module(theme, t),
    ];

    Row::new(items).gap(t.gap_xl).align(Center)
}

// ───── left: overview button + workspace pips ────────────────────────────────

fn overview_button(theme: Theme, t: &Tokens) -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(bar_tokens().indicator_w))
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(|button| {
            if button == Button::Left {
                Windows::toggle_overview();
            }
        })
        .child(icon(ICON_APPS, t.icon_size, theme.fg, t))
}

fn workspace_pip(ws: &Workspace, theme: Theme) -> Rectangle {
    let b = bar_tokens();
    let id = ws.id();
    let lit = ws.active() || ws.focused();

    let color = if ws.urgent() {
        theme.urgent
    } else if lit {
        theme.fg_strong
    } else {
        theme.fg
    };

    let width = if lit { b.dot_active } else { b.dot_size };

    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(b.dot_size))
        .radius(b.dot_size / 2.0)
        .fill(color)
        .cursor(Pointer)
        .on_click(move |button| {
            if button == Button::Left {
                Workspaces::focus(id);
            }
        })
}

// ───── center: focused title (compact) / window dock (overview) ──────────────

fn focused_title(theme: Theme, t: &Tokens) -> Box<dyn Widget> {
    let windows = Windows::read();
    let text = windows
        .focused()
        .map(|win| win.title().or(win.app_id()).unwrap_or("").to_string())
        .unwrap_or_default();

    boxed(
        Rectangle::new()
            .width(Size::Fixed(bar_tokens().window_max * 2.0))
            .height(Parent)
            .fill(Color::TRANSPARENT)
            .align_child(Center, Center)
            .clip()
            .child(
                Text::new(text)
                    .size(t.title_size)
                    .color(theme.fg_strong)
                    .font(&t.font_family)
                    .weight(t.weight_medium)
                    .elide(),
            ),
    )
}

fn window_dock(theme: Theme, t: &Tokens) -> Box<dyn Widget> {
    let windows = Windows::read();
    let tiles: Vec<Box<dyn Widget>> = windows
        .list()
        .iter()
        .map(|win| boxed(window_tile(win, theme, t)))
        .collect();

    boxed(Row::new(tiles).gap(t.gap_s).align(Center))
}

fn window_tile(win: &WindowInfo, theme: Theme, t: &Tokens) -> Rectangle {
    let b = bar_tokens();
    let id = win.id();
    let floating = win.floating();

    let fill = if win.focused() {
        theme.pill_focused
    } else {
        theme.pill
    };

    let app = win.app_id().unwrap_or("").to_string();
    let title = win.title().unwrap_or("").to_string();
    let heading = if app.is_empty() {
        title.clone()
    } else {
        app.clone()
    };

    let side = t.icon_size * 2.0;
    let text_w = (b.window_max - side - t.gap_s - t.inset_m * 2.0).max(1.0);

    let label = Column::new(vec![
        boxed(
            Text::new(heading)
                .size(t.title_size)
                .color(theme.fg_strong)
                .font(&t.font_family)
                .weight(t.weight_medium)
                .elide(),
        ),
        boxed(
            Text::new(title.clone())
                .size(t.caption_size)
                .color(theme.fg)
                .font(&t.font_family)
                .elide(),
        ),
    ])
    .gap(t.gap_2xs)
    .width(Size::Fixed(text_w));

    let row = Row::new(vec![
        boxed(app_icon_tile(&app, &title, side, theme, t)),
        boxed(label),
    ])
    .gap(t.gap_s)
    .align(Center);

    Rectangle::new()
        .width(Size::Fixed(b.window_max))
        .height(Parent)
        .radius(b.window_radius)
        .fill(fill)
        .padding(t.inset_m)
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
fn app_icon_tile(app: &str, title: &str, side: f32, theme: Theme, t: &Tokens) -> Rectangle {
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
        .radius(t.radius_pill)
        .fill(theme.pill)
        .align_child(Center, Center)
        .child(
            Text::new(first_letter(app, title))
                .size(t.title_size)
                .color(theme.fg_strong)
                .font(&t.font_family)
                .weight(t.weight_medium)
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
    let theme = context_menu::Theme::auto();

    let mut move_to = ContextMenu::new(theme);
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

    let menu = ContextMenu::new(theme)
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

fn volume_module(theme: Theme, t: &Tokens) -> Box<dyn Widget> {
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
        boxed(icon(glyph, t.icon_size, theme.fg, t)),
        boxed(
            Text::new(format!("{volume}%"))
                .size(t.label_size)
                .color(theme.fg)
                .font(&t.font_family),
        ),
    ])
    .gap(t.gap_xs)
    .align(Center);

    let b = bar_tokens();
    boxed(
        Rectangle::new()
            .width(Size::Fixed(b.volume_w))
            .height(Parent)
            .radius(t.radius_pill)
            .fill(theme.pill)
            .padding(pad(t.inset_m, b.pill_pad_y))
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

fn network_indicator(theme: Theme, t: &Tokens) -> Box<dyn Widget> {
    let net = Network::read();
    let connected = net.connected();
    let wifi_on = net.wifi_enabled();
    drop(net);

    let glyph = if connected || wifi_on {
        ICON_WIFI
    } else {
        ICON_WIFI_OFF
    };
    let color = if connected { theme.accent } else { theme.fg };

    boxed(
        Rectangle::new()
            .width(Size::Fixed(bar_tokens().indicator_w))
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
            .child(icon(glyph, t.icon_size, color, t)),
    )
}

fn battery_module(theme: Theme, t: &Tokens) -> Box<dyn Widget> {
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
        theme.accent
    } else if low {
        theme.urgent
    } else {
        theme.fg
    };

    let mut content: Vec<Box<dyn Widget>> = Vec::new();
    if on_ac {
        content.push(boxed(icon(ICON_BATTERY_CHARGING, t.icon_size, color, t)));
    }
    content.push(boxed(
        Text::new(format!("{pct}%"))
            .size(t.label_size)
            .color(color)
            .font(&t.font_family),
    ));

    boxed(
        Rectangle::new()
            // room for the AC glyph and "100%"
            .width(Size::Fixed(t.icon_size + t.label_size * 2.8))
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
            .child(Row::new(content).gap(t.gap_2xs).align(Center)),
    )
}

fn clock_module(theme: Theme, t: &Tokens) -> Box<dyn Widget> {
    boxed(
        Text::new(Clock::read().text().to_string())
            .size(t.clock_size)
            .color(theme.fg_strong)
            .font(&t.font_family)
            .weight(t.weight_medium),
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
