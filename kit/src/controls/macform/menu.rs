//! The menu the pull-down, pop-up button and combo box open: a `menuFill`
//! panel, radius 8, 4 px in, 24 px rows, the active one filled with the
//! accent.
//!
//! It's an xdg-popup lined up under the control that opened it: its right
//! edge on the control's, 4 px below (flipped above when there's no room
//! under it). It fades in over 200 ms with a 4 px slide, where the
//! original scaled from 10% height (scaling would leave the quad pipeline).
//! The pointer, or Up, Down and Enter, picks a row; Escape or a click
//! outside closes it.
//!
//! The control finds where it is on its window with `on_track`: amane
//! reports the pointer on the control and on the window, and the
//! difference is the control's corner.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use amane::{
    Center, Color, Column, ConstraintAdjustment, Gravity, Key, Point, Popup, PopupAnchor,
    Rectangle, Row, Shadow, Size, Text, Widget, current_window, pointer_position, request_frame,
};

use super::theme::{self, px};
use super::{hover_fill, on_press};
use crate::buttons::interactive;
use crate::layout::{boxed, icon, pad, text_width};
use crate::tokens::Styled;

/// One row of a menu: its text, and optionally a glyph before it and one
/// after it (Material Symbols).
#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    pub text: String,
    pub icon: Option<char>,
    pub trailing: Option<char>,
}

impl MenuItem {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            icon: None,
            trailing: None,
        }
    }

    pub fn icon(mut self, glyph: char) -> Self {
        self.icon = Some(glyph);
        self
    }

    pub fn trailing(mut self, glyph: char) -> Self {
        self.trailing = Some(glyph);
        self
    }
}

impl<S: Into<String>> From<S> for MenuItem {
    fn from(text: S) -> Self {
        Self::new(text)
    }
}

/// What an open menu needs after its control has been rebuilt away.
struct Menu {
    items: Vec<MenuItem>,
    /// The rows shown, as indices into `items` (a combo box filters).
    shown: Vec<usize>,
    active: Option<usize>,
    on_select: Rc<dyn Fn(usize)>,
    open: bool,
    opened: Instant,
    /// The row Up and Down have reached.
    keyed: Option<usize>,
    /// The control's corner on its window, and its size.
    origin: Point,
    size: (f32, f32),
    parent: Parent,
    /// Opened by a right click: its corner at the pointer, not under a control.
    at_pointer: bool,
}

/// The surface a menu opens over: a normal window by name, or a layer
/// window by namespace.
#[derive(Clone, Debug, PartialEq)]
pub enum Parent {
    Window(String),
    Layer(&'static str),
}

thread_local! {
    static MENUS: RefCell<HashMap<String, Menu>> = RefCell::new(HashMap::new());
}

/// Room around the panel for its shadow.
const MARGIN: f32 = 10.0;
/// The gap between the control and the panel.
const GAP: f32 = 4.0;
const FADE: Duration = Duration::from_millis(200);

/// Records `id`'s items and handler for a menu that's open or may open,
/// keeping its open state and position. Call on every build.
pub(crate) fn register(
    id: &str,
    items: Vec<MenuItem>,
    shown: Vec<usize>,
    active: Option<usize>,
    size: (f32, f32),
    on_select: Rc<dyn Fn(usize)>,
) {
    register_on(id, items, shown, active, size, on_select, None);
}

/// [`register`], over the layer window `layer` when one is given.
pub(crate) fn register_on(
    id: &str,
    items: Vec<MenuItem>,
    shown: Vec<usize>,
    active: Option<usize>,
    size: (f32, f32),
    on_select: Rc<dyn Fn(usize)>,
    layer: Option<&'static str>,
) {
    let parent = match layer {
        Some(namespace) => Parent::Layer(namespace),
        None => Parent::Window(current_window()),
    };
    MENUS.with_borrow_mut(|menus| {
        let menu = menus.entry(id.to_string()).or_insert_with(|| Menu {
            items: Vec::new(),
            shown: Vec::new(),
            active: None,
            on_select: on_select.clone(),
            open: false,
            opened: Instant::now(),
            keyed: None,
            origin: Point { x: 0.0, y: 0.0 },
            size,
            parent: parent.clone(),
            at_pointer: false,
        });
        if menu.shown != shown {
            menu.keyed = None;
        }
        menu.items = items;
        menu.shown = shown;
        menu.active = active;
        menu.on_select = on_select;
        menu.size = size;
        menu.parent = parent;
    });
}

/// `control` with its corner tracked, for the menu to line up with it.
pub(crate) fn track(id: &str, control: Rectangle) -> Rectangle {
    let id = id.to_string();
    control.on_track(move |at: Point| {
        let window = pointer_position();
        MENUS.with_borrow_mut(|menus| {
            if let Some(menu) = menus.get_mut(&id) {
                menu.origin = Point {
                    x: window.x - at.x,
                    y: window.y - at.y,
                };
            }
        });
    })
}

/// A context menu: opens `items` with its top-left corner at the pointer,
/// over the current window, and calls `on_select` with the row picked.
/// Call it from a right click.
pub fn context_menu(id: &str, items: Vec<MenuItem>, on_select: impl Fn(usize) + 'static) {
    // a right click while it's open only closes it
    if is_open(id) {
        close(id);
        return;
    }
    let shown = (0..items.len()).collect();
    register(id, items, shown, None, (1.0, 1.0), Rc::new(on_select));
    let at = pointer_position();
    MENUS.with_borrow_mut(|menus| {
        if let Some(menu) = menus.get_mut(id) {
            menu.origin = at;
            menu.at_pointer = true;
        }
    });
    open(id);
}

pub(crate) fn is_open(id: &str) -> bool {
    MENUS.with_borrow(|menus| menus.get(id).is_some_and(|menu| menu.open))
}

/// Opens `id`'s menu under its control, or closes it if it's open.
pub(crate) fn toggle(id: &str) {
    if is_open(id) {
        close(id);
    } else {
        open(id);
    }
}

pub(crate) fn open(id: &str) {
    let opened = MENUS.with_borrow_mut(|menus| {
        let menu = menus.get_mut(id)?;
        if menu.open {
            return None;
        }
        menu.open = true;
        menu.opened = Instant::now();
        menu.keyed = None;
        Some(())
    });
    if opened.is_none() {
        return;
    }
    let id = id.to_string();
    amane::open_popup(move || popup(&id));
}

pub(crate) fn close(id: &str) {
    MENUS.with_borrow_mut(|menus| {
        if let Some(menu) = menus.get_mut(id) {
            menu.open = false;
        }
    });
    amane::mark_all();
}

/// Moves the keyboard row by `step`, from the active row if none yet.
pub(crate) fn key(id: &str, key: Key) -> bool {
    let chosen = MENUS.with_borrow_mut(|menus| {
        let menu = menus.get_mut(id)?;
        let rows = menu.shown.len();
        if rows == 0 {
            return None;
        }
        let at = menu
            .keyed
            .or_else(|| menu.shown.iter().position(|&i| Some(i) == menu.active));
        match key {
            Key::Down => {
                menu.keyed = Some(at.map_or(0, |at| (at + 1).min(rows - 1)));
                None
            }
            Key::Up => {
                menu.keyed = Some(at.map_or(rows - 1, |at| at.saturating_sub(1)));
                None
            }
            Key::Enter => menu
                .keyed
                .map(|row| (menu.shown[row], menu.on_select.clone())),
            _ => None,
        }
    });
    amane::mark_all();
    match key {
        Key::Enter => {
            close(id);
            if let Some((index, select)) = chosen {
                select(index);
            }
            true
        }
        Key::Escape => {
            close(id);
            true
        }
        Key::Up | Key::Down => true,
        _ => false,
    }
}

fn row_style() -> crate::tokens::TextStyle {
    crate::tokens::sys::r#type::label()
}

/// How wide the panel is for `items`.
fn panel_width(items: &[MenuItem], shown: &[usize]) -> f32 {
    let glyph = px(15.0);
    let widest = shown
        .iter()
        .filter_map(|&i| items.get(i))
        .map(|item| {
            let mut w = text_width(&item.text, row_style()) + px(24.0);
            if item.icon.is_some() {
                w += glyph + px(8.0);
            }
            if item.trailing.is_some() {
                w += glyph + px(8.0);
            }
            w
        })
        .fold(0.0, f32::max);
    (widest + px(8.0)).max(px(130.0)).ceil()
}

fn popup(id: &str) -> Popup {
    let state = MENUS.with_borrow(|menus| {
        menus.get(id).map(|menu| {
            (
                menu.items.clone(),
                menu.shown.clone(),
                menu.active,
                menu.open,
                menu.opened,
                menu.keyed,
                menu.origin,
                menu.size,
                menu.parent.clone(),
                menu.at_pointer,
            )
        })
    });
    let Some((items, shown, active, open, opened, keyed, origin, size, parent, at_pointer)) = state
    else {
        return Popup::new("amane.macform.menu")
            .size(1, 1)
            .anchor_rect(0, 0, 1, 1)
            .visible(false);
    };

    let pad_in = px(4.0);
    let width = panel_width(&items, &shown);
    let height = shown.len().max(1) as f32 * theme::menu_item_height() + pad_in * 2.0;

    let rows: Vec<Box<dyn Widget>> = shown
        .iter()
        .enumerate()
        .map(|(row, &index)| {
            boxed(menu_row(
                id,
                &items[index],
                index,
                Some(index) == active,
                keyed == Some(row),
                row == 0,
                row + 1 == shown.len(),
            ))
        })
        .collect();

    // fading in, with a short slide down from the control
    let t = (opened.elapsed().as_secs_f32() / FADE.as_secs_f32()).clamp(0.0, 1.0);
    if t < 1.0 {
        request_frame();
    }
    let eased = 1.0 - (1.0 - t).powi(3);

    let panel = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .radius(px(8.0))
        .fill(theme::menu_fill())
        .border(1.0, theme::field_border())
        .shadow(
            // the original's elevation 2
            Shadow::drop(theme::shadow_color())
                .blur(6.66)
                .spread(-0.81)
                .offset(0.0, 1.5),
        )
        .padding(pad(pad_in, pad_in))
        .clip()
        .opacity(eased)
        .translate(0.0, (1.0 - eased) * -4.0)
        .child(Column::new(rows).width(Size::Parent));

    let surface = Rectangle::new()
        .width(Size::Fixed(width + MARGIN * 2.0))
        .height(Size::Fixed(height + MARGIN * 2.0))
        .padding(pad(MARGIN, MARGIN))
        .child(panel);

    let escaped = id.to_string();
    let dismissed = id.to_string();

    let popup = Popup::new("amane.macform.menu")
        .size(
            (width + MARGIN * 2.0).ceil() as u32,
            (height + MARGIN * 2.0).ceil() as u32,
        )
        .anchor_rect(
            origin.x.round() as i32,
            origin.y.round() as i32,
            size.0.round().max(1.0) as i32,
            size.1.round().max(1.0) as i32,
        )
        .anchor(if at_pointer {
            PopupAnchor::TopLeft
        } else {
            PopupAnchor::BottomRight
        })
        .gravity(if at_pointer {
            Gravity::BottomRight
        } else {
            Gravity::BottomLeft
        })
        .offset(
            if at_pointer { -MARGIN } else { MARGIN } as i32,
            if at_pointer { -MARGIN } else { GAP - MARGIN } as i32,
        )
        .constraint_adjustment(
            ConstraintAdjustment::SLIDE_X
                | ConstraintAdjustment::FLIP_X
                | ConstraintAdjustment::FLIP_Y
                | ConstraintAdjustment::SLIDE_Y,
        )
        .grab()
        .visible(open);
    let popup = match parent {
        Parent::Window(window) => popup.parent_window(&window),
        Parent::Layer(namespace) => popup.parent(namespace),
    };
    popup
        .on_dismiss(move || close(&dismissed))
        .on_key(move |k| {
            key(&escaped, k);
        })
        .child(surface)
}

fn menu_row(
    id: &str,
    item: &MenuItem,
    index: usize,
    active: bool,
    keyed: bool,
    first: bool,
    last: bool,
) -> Rectangle {
    let key = format!("{id}.row.{index}");
    let glyph = px(15.0);
    let (fg, glyph_color) = if active {
        (theme::glyph_on_tint(), theme::glyph_on_tint())
    } else {
        (theme::label_primary(), theme::label_secondary())
    };

    let mut parts: Vec<Box<dyn Widget>> = Vec::new();
    if let Some(g) = item.icon {
        parts.push(boxed(icon(g, glyph, glyph_color)));
    }
    parts.push(boxed(
        Text::new(item.text.clone())
            .style(row_style())
            .color(fg)
            .elide(),
    ));
    if let Some(g) = item.trailing {
        parts.push(boxed(icon(g, glyph, glyph_color)));
    }

    let base_fill = if active {
        theme::tint()
    } else {
        Color::TRANSPARENT
    };
    let mut fill = hover_fill(&key, base_fill, fg, false);
    if keyed {
        fill = crate::color::over(theme::hover_of(fg), fill);
    }

    let (small, round) = (px(4.0), px(6.0));
    let corner = |edge: bool| if active || edge { round } else { small };

    let base = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(theme::menu_item_height()))
        .radius_top_left(corner(first))
        .radius_top_right(corner(first))
        .radius_bottom_left(corner(last))
        .radius_bottom_right(corner(last))
        .fill(fill)
        .padding(pad(px(12.0), 0.0))
        .align_child(amane::Start, Center)
        .child(
            Row::new(parts)
                .gap(px(8.0))
                .align(Center)
                .width(Size::Parent),
        );

    let id = id.to_string();
    interactive(
        base,
        key,
        false,
        on_press(move || {
            let select = MENUS.with_borrow(|menus| menus.get(&id).map(|m| m.on_select.clone()));
            close(&id);
            if let Some(select) = select {
                select(index);
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_open_menu_builds_every_kind_of_row() {
        let items: Vec<MenuItem> = vec![
            MenuItem::new("Name").icon('\u{e5ca}'),
            MenuItem::new("Date"),
            MenuItem::new("Size").trailing('\u{e5ca}'),
        ];
        MENUS.with_borrow_mut(|menus| {
            menus.insert(
                "test.menu".into(),
                Menu {
                    items,
                    shown: vec![0, 1, 2],
                    active: Some(1),
                    on_select: Rc::new(|_| {}),
                    open: true,
                    opened: Instant::now(),
                    keyed: Some(2),
                    origin: Point { x: 10.0, y: 10.0 },
                    size: (110.0, 24.0),
                    parent: Parent::Window("test".into()),
                    at_pointer: false,
                },
            );
        });
        // a single row is both first and last
        let _ = popup("test.menu");
        MENUS.with_borrow_mut(|menus| menus.get_mut("test.menu").unwrap().shown = vec![0]);
        let _ = popup("test.menu");
    }
}
