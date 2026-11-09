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
//! are `~/.config/emaki/status-bar.json` (Emaki Settings), followed
//! live. With `auto_hide` it slides past its edge ([`EdgeReveal`]) and
//! keeps no room from windows, coming back when the pointer meets the edge
//! or Quick Settings or the notification center is open, which spares an
//! OLED panel a strip that never changes.

mod activities;
mod apps;
mod chips;
mod drag;
mod indicators;
mod items;
mod size;
mod tray;
mod workspace_shape;
mod workspaces;

use items::*;
use workspaces::*;

use std::cell::Cell;
use std::time::Duration;

use amane::{
    Audio, Battery, Bluetooth, BluetoothDevice, Center, Color, Easing, Full, InputArea, Justify,
    Keyboard, Layer, LayerWindow, Link, Monitor, Network, Parent, PowerProfile, Rectangle, Row,
    Scroll, ScrollSource, Service, Size, Start, Vertical, Widget, Workspace, Workspaces, Zone,
    window_size,
};
use emaki_kit::settings::{AutoHide, BarStyle, ClockStyle, ShowDate, status_bar};
use emaki_kit::theme::{self, Mode};
use emaki_phone::KdeConnect;

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
use emaki_kit::buttons::{ButtonBuilder, DrawnButton, IconButton, PushButton, Shape, Style};
use emaki_kit::edge_reveal::EdgeReveal;
use emaki_kit::icons::{
    Glyph, ICON_AIRPLANE, ICON_BLUETOOTH, ICON_BLUETOOTH_CONNECTED, ICON_BLUETOOTH_OFF, ICON_CABLE,
    ICON_DND, ICON_KEEP_AWAKE, ICON_LAN, ICON_NIGHT_LIGHT, ICON_NOTIFICATIONS, ICON_PHONE,
    ICON_TUNE, ICON_UNDO, ICON_VOLUME_OFF, ICON_VPN, ICON_WIFI_NONE, ICON_WIFI_OFF, audio_glyph,
    battery_glyph, power_profile_glyph, wifi_glyph,
};
use emaki_kit::layout::{boxed, cursor, pad, text_width, track_cursor};
use emaki_kit::material::{self, Level, Material};
use emaki_kit::slider;

use crate::tokens::{bar, sys};

pub(crate) use indicators::{Indicator, Item};

use crate::chrome::bar::{edge, height};
use crate::chrome::per_output::PerOutput;
pub use activities::popup as activity_popup;
pub use indicators::run;
pub use size::metrics;
pub use workspace_shape::switch_position;

/// The status bar's layer-surface namespace. The niri `layer-rule` matches
/// this to frost its popups, and tooltips hang under it.
pub const BAR_NS: &str = "emaki.status-bar";

/// How many lines of touchpad scroll move one workspace along.
fn scroll_step() -> f32 {
    bar::scroll::step()
}

/// How tall the strip along the edge is that brings a hidden bar back.
const STRIP: f32 = 2.0;

thread_local! {
    // touchpad scroll over the workspaces not yet made into a step
    static SCROLLED: Cell<f32> = const { Cell::new(0.0) };

    // and over the keyboard layout
    static LAYOUT_SCROLLED: Cell<f32> = const { Cell::new(0.0) };

    // where each monitor's hiding bar is, when it hides
    static REVEALS: PerOutput<EdgeReveal> = const { PerOutput::new() };

    // the menu last opened from the bar, by its key, and the item that opened it
    static MENU_OF: std::cell::RefCell<Option<(&'static str, String)>> =
        const { std::cell::RefCell::new(None) };
}

fn new_reveal() -> EdgeReveal {
    EdgeReveal::new(
        emaki_kit::motion::eased(0.0, Duration::from_millis(220), Easing::Out),
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
    emaki_kit::glass_region::shape_id(&format!("emaki.status-bar.{output}"))
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

// ───── right: status cluster ─────────────────────────────────────────────────

/// The items on the right that have something to show, in their order,
/// draggable into another.
/// Apps' items that don't fit in `room` go behind a » button. An item just
/// dragged off the bar leaves an undo button at its left for a while.
fn status(output: &str, room: f32) -> Row {
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
    let gap = metrics().gap;
    let width_of_all = |built: &[(Item, Vec<Box<dyn Widget>>)]| -> f32 {
        built
            .iter()
            .flat_map(|(_, widgets)| widgets.iter())
            .map(|widget| width_of(widget.as_ref()) + gap)
            .sum()
    };
    let undo = drag::just_hidden().map(undo_button);
    let room = room
        - undo
            .as_ref()
            .map_or(0.0, |undo| width_of(undo.as_ref()) + gap);
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

    let cluster = boxed(drag::cluster(output, built, height()));
    Row::new(undo.into_iter().chain([cluster]).collect())
        .gap(gap)
        .align(Center)
        .height(Parent)
}

/// Brings back `item`, just dragged off the bar.
fn undo_button(item: Item) -> Box<dyn Widget> {
    boxed(
        bar_icon("emaki.bar.undo-hide", ICON_UNDO, sys::color::accent::bg())
            .on_press(drag::undo_hide)
            .tooltip(BAR_NS, format!("Undo · put {} back", item.label()))
            .build(),
    )
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

/// Opens `menu` as `key` under the bar at `point`, for `item` (its button's
/// id), which stays lit while it's open.
fn show_menu(item: &str, key: &'static str, menu: emaki_kit::menu::ContextMenu, point: (i32, i32)) {
    opened_menu(item, key);
    emaki_kit::menu::layer::show_under(key, menu, BAR_NS, point);
}

/// Notes that `item` opened the menu `key`, opened some other way.
fn opened_menu(item: &str, key: &'static str) {
    MENU_OF.set(Some((key, item.to_string())));
}

/// Whether the menu `item` opened is still open, so it stays lit as the
/// panels' items do.
fn menu_showing(item: &str) -> bool {
    use emaki_kit::menu::layer;
    MENU_OF.with_borrow(|menu| {
        menu.as_ref()
            .is_some_and(|(key, by)| by == item && layer::is_open(key))
    }) && emaki_kit::menu::layer::is_open_under(BAR_NS)
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
