// This module is a reusable context-menu component. Its public API
// (icon/theme constructors, extra builder methods) is intentionally broader
// than the bundled demo exercises, so dead_code is allowed module-wide.
// `unused_imports` is deliberately NOT allowed — imports must stay honest.
#![allow(dead_code)]

pub mod runtime;
pub mod theme;
pub mod tokens;

use std::path::PathBuf;
use std::rc::Rc;

use amane::{
    Button, Center, Color, Column, ConstraintAdjustment, Gravity, Horizontal, Image, Key, Keyboard,
    Layer, LayerWindow, Padding, Parent, Point, Pointer, Popup, Rectangle, Row, SpaceBetween, Text,
    Vertical, Weight, Widget, WindowSize, request_frame,
};

pub use theme::Theme;
// Full public API of the component. Some entry points aren't exercised by
// the bundled demo, so this re-export block allows unused imports.
#[allow(unused_imports)]
pub use runtime::{arm, close, open_at, open_named, open_under, register, set_cursor};
#[allow(unused_imports)]
pub use theme::Mode;

use runtime::{FocusSource, with, with_mut};
use tokens::{Tokens, tokens};

/// Maximum submenu nesting depth. Each level is a distinct popup slot with
/// its own compile-time namespace; bump this (and [`NS`]) to allow deeper.
pub const MAX_DEPTH: usize = 8;

/// The anchor layer-surface namespace. The niri `layer-rule` matches this and
/// its `popups { }` block styles every descendant popup (all menu levels).
pub const NAMESPACE_ANCHOR: &str = "amane.context-menu-anchor";

/// One namespace per depth level. Level `k`'s popup is parented to level
/// `k-1` (level 0 to the anchor).
const NS: [&str; MAX_DEPTH] = [
    "amane.context-menu.0",
    "amane.context-menu.1",
    "amane.context-menu.2",
    "amane.context-menu.3",
    "amane.context-menu.4",
    "amane.context-menu.5",
    "amane.context-menu.6",
    "amane.context-menu.7",
];

fn parent_ns(k: usize) -> &'static str {
    if k == 0 {
        // level 0 hangs under a caller-supplied surface when one is set
        // (e.g. a taskbar pill via `open_under`), else the desktop anchor
        runtime::root_parent().unwrap_or(NAMESPACE_ANCHOR)
    } else {
        NS[k - 1]
    }
}

// ───── Public types ──────────────────────────────────────────────────────

#[derive(Clone)]
pub enum Icon {
    Glyph {
        ch: String,
        font: Option<&'static str>,
    },
    Image(PathBuf),
}

impl Icon {
    pub fn glyph(ch: impl Into<String>) -> Self {
        Self::Glyph {
            ch: ch.into(),
            font: None,
        }
    }

    pub fn glyph_with_font(ch: impl Into<String>, font: &'static str) -> Self {
        Self::Glyph {
            ch: ch.into(),
            font: Some(font),
        }
    }

    pub fn image(path: impl Into<PathBuf>) -> Self {
        Self::Image(path.into())
    }
}

#[derive(Clone)]
pub struct MenuItem {
    label: String,
    icon: Option<Icon>,
    shortcut: Option<String>,
    destructive: bool,
    enabled: bool,
    on_select: Option<Rc<dyn Fn()>>,
    submenu: Option<Vec<Entry>>,
}

impl MenuItem {
    pub fn label(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            shortcut: None,
            destructive: false,
            enabled: true,
            on_select: None,
            submenu: None,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn on_select(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    pub fn submenu(mut self, menu: ContextMenu) -> Self {
        self.submenu = Some(menu.entries);
        self
    }

    fn has_submenu(&self) -> bool {
        self.submenu.is_some()
    }
}

#[derive(Clone)]
pub(crate) enum Entry {
    Item(MenuItem),
    Separator,
}

#[derive(Clone)]
pub struct ContextMenu {
    pub(crate) theme: Theme,
    pub(crate) entries: Vec<Entry>,
}

impl ContextMenu {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            entries: Vec::new(),
        }
    }

    pub fn item(mut self, item: MenuItem) -> Self {
        self.entries.push(Entry::Item(item));
        self
    }

    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }
}

// ───── View entry points ─────────────────────────────────────────────────

/// Overlay anchor layer-surface. Two modes:
///
/// - **Idle / Open**: 1×1 top-left, click-through (`input_region` empty).
///   Does not block any pointer input to apps.
/// - **Arming**: fullscreen, catches right-click (captures a fresh input
///   serial for `xdg_popup.grab()`), Esc cancels.
///
/// `Keyboard::OnDemand` is required for popup keyboard inheritance —
/// wlr-layer-shell says child popups inherit the parent's keyboard mode,
/// and `xdg_popup.grab()` can only route keyboard to a popup whose
/// inherited mode is not None.
pub fn anchor() -> LayerWindow {
    let arming = runtime::is_arming();

    let size: WindowSize = if arming {
        WindowSize::Full
    } else {
        WindowSize::Fixed(1.0)
    };

    let mut lw = LayerWindow::new()
        .width(size)
        .height(size)
        .layer(Layer::Overlay)
        .keyboard(Keyboard::OnDemand)
        .namespace(NAMESPACE_ANCHOR)
        .visible(true);

    if !arming {
        lw = lw
            .anchor_vertical(Vertical::Top)
            .anchor_horizontal(Horizontal::Left)
            .input_region(Vec::new());
    }

    let mut rect = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT);

    if arming {
        rect = rect
            .on_move(|p: Point| runtime::set_cursor(p.x as i32, p.y as i32))
            .on_click(|btn: Button| {
                if btn == Button::Right {
                    runtime::open_at_cursor();
                } else {
                    runtime::close();
                }
            });
    }

    lw.on_key(|key| {
        if matches!(key, Key::Escape) {
            runtime::close();
        }
    })
    .child(rect)
}

/// The fixed array of per-depth popup view fns. Register all of them on the
/// `App` (`for f in LEVELS { app = app.popup(f); }`). Each renders the menu
/// level at its depth, reading the open stack.
pub const LEVELS: [fn() -> Popup; MAX_DEPTH] = [
    || render(0),
    || render(1),
    || render(2),
    || render(3),
    || render(4),
    || render(5),
    || render(6),
    || render(7),
];

/// Render the popup for depth level `k`. Returns an invisible placeholder
/// when the stack isn't that deep, so the slot stays registered but mapped
/// only while its level is open.
fn render(k: usize) -> Popup {
    let t = tokens();

    let Some((entries, theme, anchor_rect)) =
        runtime::level(k, |l| (l.entries.clone(), l.theme, l.anchor_rect))
    else {
        return Popup::new(NS[k])
            .size(1, 1)
            .anchor_rect(0, 0, 1, 1)
            .parent(parent_ns(k))
            .visible(false);
    };

    let (width, height) = size_for(&entries, t);
    let card = card_widget(&entries, theme.current(), t, k);
    let (ax, ay, aw, ah) = anchor_rect;

    // Level 0 can slide on both axes; deeper levels keep their x (they flip
    // to the parent's other side instead) and sit just left of the row edge.
    let constraint = if k == 0 {
        ConstraintAdjustment::SLIDE_X
            | ConstraintAdjustment::SLIDE_Y
            | ConstraintAdjustment::FLIP_X
            | ConstraintAdjustment::FLIP_Y
    } else {
        ConstraintAdjustment::SLIDE_Y | ConstraintAdjustment::FLIP_X | ConstraintAdjustment::FLIP_Y
    };

    let mut popup = Popup::new(NS[k])
        .size(width as u32, height as u32)
        .anchor_rect(ax, ay, aw, ah)
        .parent(parent_ns(k))
        .gravity(Gravity::BottomRight)
        .constraint_adjustment(constraint)
        .grab()
        .visible(runtime::visible_at(k))
        .on_key(move |key| handle_key(k, key))
        .child(card);

    if k > 0 {
        popup = popup.offset(-t.submenu_offset as i32, 0);
    } else {
        popup = popup.on_dismiss(runtime::close);
    }

    popup
}

// ───── Card / row rendering ──────────────────────────────────────────────

fn card_widget(entries: &[Entry], theme: Theme, t: &Tokens, level: usize) -> Rectangle {
    let any_icon = entries_have_icon(entries);
    let rows: Vec<Box<dyn Widget>> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| build_entry(i, entry, theme, t, level, any_icon))
        .collect();

    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(theme.surface)
        .radius(t.radius)
        .border(t.hairline, theme.border)
        .padding(Padding {
            top: t.card_padding,
            right: t.card_padding,
            bottom: t.card_padding,
            left: t.card_padding,
        })
        .child(Column::new(rows).width(Parent))
}

fn build_entry(
    index: usize,
    entry: &Entry,
    theme: Theme,
    t: &Tokens,
    level: usize,
    any_icon: bool,
) -> Box<dyn Widget> {
    match entry {
        Entry::Separator => Box::new(
            Rectangle::new()
                .width(Parent)
                .height(t.hairline + 2.0 * t.separator_margin)
                .fill(Color::TRANSPARENT)
                .padding(Padding {
                    top: t.separator_margin,
                    right: 0.0,
                    bottom: t.separator_margin,
                    left: 0.0,
                })
                .child(
                    Rectangle::new()
                        .width(Parent)
                        .height(t.hairline)
                        .fill(theme.separator),
                ),
        ),
        Entry::Item(item) => Box::new(row(index, item, theme, t, level, any_icon)),
    }
}

fn row(
    index: usize,
    item: &MenuItem,
    theme: Theme,
    t: &Tokens,
    level: usize,
    any_icon: bool,
) -> Rectangle {
    let (focused, source) = runtime::level(level, |l| (l.focus == Some(index), l.focus_source))
        .unwrap_or((false, None));
    let focused = focused && item.enabled;
    let keyboard = focused && source == Some(FocusSource::Keyboard);

    let fill = if !focused {
        Color::TRANSPARENT
    } else if keyboard && !item.destructive {
        theme.selected_fill
    } else if item.destructive {
        theme.hover_destructive_fill
    } else {
        theme.hover_fill
    };

    let fg = if !item.enabled {
        theme.muted
    } else if keyboard && !item.destructive {
        theme.on_selected_fg
    } else if item.destructive {
        theme.destructive
    } else {
        theme.fg
    };

    let shortcut_color = if keyboard && !item.destructive {
        with_alpha_f(theme.on_selected_fg, t.shortcut_hover_alpha)
    } else if focused {
        with_alpha_f(theme.fg, t.shortcut_hover_alpha)
    } else {
        theme.muted
    };

    let label_widget: Box<dyn Widget> = Box::new(
        Text::new(item.label.clone())
            .size(t.label_size)
            .color(fg)
            .font(&t.font_family)
            .weight(Weight::from(t.font_regular))
            .elide(),
    );

    let shortcut_widget: Box<dyn Widget> = match &item.shortcut {
        Some(s) => Box::new(
            Text::new(s.clone())
                .size(t.shortcut_size)
                .color(shortcut_color)
                .font(&t.font_family)
                .weight(Weight::from(t.font_regular)),
        ),
        None => Box::new(empty()),
    };

    let chevron_widget: Box<dyn Widget> = if item.has_submenu() {
        Box::new(
            Rectangle::new()
                .width(t.chevron_slot)
                .height(t.chevron_slot)
                .fill(Color::TRANSPARENT)
                .align_child(Center, Center)
                .child(
                    Text::new("\u{203A}")
                        .size(t.label_size)
                        .color(fg)
                        .font(&t.font_family)
                        .tight(),
                ),
        )
    } else {
        Box::new(empty())
    };

    // When no row in this menu has an icon, drop the leading icon column
    // entirely so the label hugs the left padding.
    let leading = if any_icon {
        Row::new(vec![icon_widget(item.icon.as_ref(), fg, t), label_widget])
            .gap(t.gap)
            .align(Center)
            .height(Parent)
    } else {
        Row::new(vec![label_widget]).align(Center).height(Parent)
    };
    let trailing = Row::new(vec![shortcut_widget, chevron_widget])
        .gap(t.trailing_gap)
        .align(Center);

    let inner = Row::new(vec![
        Box::new(leading) as Box<dyn Widget>,
        Box::new(trailing) as Box<dyn Widget>,
    ])
    .width(Parent)
    .height(Parent)
    .justify(SpaceBetween)
    .align(Center);

    let has_submenu = item.has_submenu();
    let on_select = item.on_select.clone();
    let enabled = item.enabled;

    let mut rect = Rectangle::new()
        .width(Parent)
        .height(t.row_height)
        .fill(fill)
        .radius(t.radius_row)
        .padding(Padding {
            top: 0.0,
            right: t.row_pad_x,
            bottom: 0.0,
            left: t.row_pad_x,
        })
        .child(inner);

    if enabled {
        rect = rect
            .cursor(Pointer)
            .on_hover(move |inside| set_focus(level, index, inside, has_submenu))
            .on_click(move |_button| {
                if has_submenu {
                    runtime::push_level(level, index);
                } else {
                    if let Some(f) = &on_select {
                        f();
                    }
                    runtime::close();
                }
            });
    }

    rect
}

fn icon_widget(icon: Option<&Icon>, fg: Color, t: &Tokens) -> Box<dyn Widget> {
    match icon {
        Some(Icon::Glyph { ch, font }) => {
            let text = Text::new(ch.clone()).size(t.label_size).color(fg).tight();
            let text = match font {
                Some(f) => text.font(f),
                None => text.font(&t.font_family),
            };
            Box::new(
                Rectangle::new()
                    .width(t.icon_slot)
                    .height(t.icon_slot)
                    .fill(Color::TRANSPARENT)
                    .align_child(Center, Center)
                    .child(text),
            )
        }
        Some(Icon::Image(path)) => Box::new(
            Rectangle::new()
                .width(t.icon_slot)
                .height(t.icon_slot)
                .fill(Image::contain(path.clone())),
        ),
        None => Box::new(
            Rectangle::new()
                .width(t.icon_slot)
                .height(t.icon_slot)
                .fill(Color::TRANSPARENT),
        ),
    }
}

fn empty() -> Rectangle {
    Rectangle::new()
        .width(0.0)
        .height(0.0)
        .fill(Color::TRANSPARENT)
}

// ───── Focus / keyboard ──────────────────────────────────────────────────

/// Pointer hover on a row at `level`. Updates the highlight and, for a
/// submenu row, opens its child level; for a leaf row, closes any deeper
/// levels.
fn set_focus(level: usize, index: usize, inside: bool, has_submenu: bool) {
    if !inside {
        with_mut(|r| {
            let is_deepest = r.stack.len() == level + 1;
            if let Some(l) = r.stack.get_mut(level)
                && l.focus == Some(index)
                && is_deepest
            {
                l.focus = None;
                l.focus_source = None;
            }
        });
        request_frame();
        return;
    }

    let already_open = with(|r| {
        r.stack.len() > level + 1 && r.stack.get(level).map(|l| l.focus) == Some(Some(index))
    });

    with_mut(|r| {
        if let Some(l) = r.stack.get_mut(level) {
            l.focus = Some(index);
            l.focus_source = Some(FocusSource::Pointer);
        }
    });

    if has_submenu {
        if !already_open {
            runtime::push_level(level, index);
        }
    } else {
        runtime::pop_to(level);
    }
    request_frame();
}

/// Keyboard handler for the popup at `level`. Only the deepest (topmost
/// grabbing) popup receives keys, so `level` is the active level.
fn handle_key(level: usize, key: Key) {
    match key {
        Key::Escape => runtime::close(),
        Key::Down => step_focus(level, 1),
        Key::Up => step_focus(level, -1),
        Key::Right => {
            descend(level);
        }
        Key::Left => {
            if level > 0 {
                runtime::pop_to(level - 1);
            }
        }
        Key::Enter if !descend(level) => {
            invoke_focused(level);
        }
        _ => {}
    }
}

/// If the focused row at `level` has a submenu, open it and focus its first
/// item. Returns whether it descended.
fn descend(level: usize) -> bool {
    let focus = runtime::level(level, |l| l.focus).flatten();
    let Some(i) = focus else {
        return false;
    };
    let has = runtime::level(
        level,
        |l| matches!(l.entries.get(i), Some(Entry::Item(it)) if it.has_submenu()),
    )
    .unwrap_or(false);

    if has {
        runtime::push_level(level, i);
        focus_first(level + 1);
        true
    } else {
        false
    }
}

fn step_focus(level: usize, delta: isize) {
    with_mut(|r| {
        let Some(l) = r.stack.get_mut(level) else {
            return;
        };
        let selectable: Vec<usize> = l
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| matches!(e, Entry::Item(it) if it.enabled).then_some(i))
            .collect();
        if selectable.is_empty() {
            return;
        }

        let next = match l
            .focus
            .and_then(|c| selectable.iter().position(|&i| i == c))
        {
            Some(pos) => {
                let n = selectable.len() as isize;
                selectable[(((pos as isize + delta) % n + n) % n) as usize]
            }
            None if delta >= 0 => selectable[0],
            None => *selectable.last().unwrap(),
        };

        l.focus = Some(next);
        l.focus_source = Some(FocusSource::Keyboard);
    });
    request_frame();
}

fn focus_first(level: usize) {
    with_mut(|r| {
        if let Some(l) = r.stack.get_mut(level) {
            l.focus = l
                .entries
                .iter()
                .position(|e| matches!(e, Entry::Item(it) if it.enabled));
            l.focus_source = Some(FocusSource::Keyboard);
        }
    });
    request_frame();
}

fn invoke_focused(level: usize) {
    let on_select = with(|r| {
        let l = r.stack.get(level)?;
        let i = l.focus?;
        match l.entries.get(i)? {
            Entry::Item(it) if it.enabled => it.on_select.clone(),
            _ => None,
        }
    });

    if let Some(f) = on_select {
        f();
        runtime::close();
    }
}

// ───── Measurement ───────────────────────────────────────────────────────

/// An estimate: amane doesn't expose text measurement.
fn text_width(s: &str, size: f32, t: &Tokens) -> f32 {
    s.chars().count() as f32 * size * t.advance_ratio
}

fn item_width(item: &MenuItem, t: &Tokens, any_icon: bool) -> f32 {
    let label = text_width(&item.label, t.label_size, t);
    let shortcut = item
        .shortcut
        .as_ref()
        .map(|s| text_width(s, t.shortcut_size, t) + t.shortcut_gap)
        .unwrap_or(0.0);
    let chevron = if item.has_submenu() {
        t.chevron_slot + t.trailing_gap
    } else {
        0.0
    };
    let leading = if any_icon { t.icon_slot + t.gap } else { 0.0 };

    leading + label + shortcut + chevron + 2.0 * t.row_pad_x
}

pub(crate) fn entries_have_icon(entries: &[Entry]) -> bool {
    entries
        .iter()
        .any(|e| matches!(e, Entry::Item(it) if it.icon.is_some()))
}

pub(crate) fn size_for(entries: &[Entry], t: &Tokens) -> (f32, f32) {
    let any_icon = entries_have_icon(entries);
    let widest = entries
        .iter()
        .filter_map(|e| match e {
            Entry::Item(it) => Some(item_width(it, t, any_icon)),
            _ => None,
        })
        .fold(0.0_f32, f32::max);

    let width = (widest + 2.0 * t.card_padding).clamp(t.min_width, t.max_width);

    let mut h = 2.0 * t.card_padding;
    for e in entries {
        match e {
            Entry::Item(_) => h += t.row_height,
            Entry::Separator => h += t.hairline + 2.0 * t.separator_margin,
        }
    }

    (width, h)
}

pub(crate) fn row_offset(entries: &[Entry], index: usize, t: &Tokens) -> f32 {
    let mut y = 0.0;
    for (i, e) in entries.iter().enumerate() {
        if i == index {
            break;
        }
        match e {
            Entry::Item(_) => y += t.row_height,
            Entry::Separator => y += t.hairline + 2.0 * t.separator_margin,
        }
    }
    y
}

fn with_alpha_f(color: Color, alpha: f32) -> Color {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::rgba(color.red(), color.green(), color.blue(), a)
}
