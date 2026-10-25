//! Dropdown — one choice out of a list, behind a button that shows it.
//!
//! Modelled on Material 3's exposed dropdown menu, drawn like macOS's pop-up
//! button: a control-height button with the chosen option and a chevron,
//! and a click opens a popover menu of the options at the pointer, the
//! chosen one checked. Choosing one, clicking outside or Escape closes it.
//!
//! The menu is an xdg-popup over the surface the dropdown is on: a normal
//! window's (found by itself, from `current_window`), or a layer window's
//! namespace given with [`Dropdown::layer`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use amane::{
    Center, Column, ConstraintAdjustment, Gravity, Key, Point, Popup, PopupAnchor, Radius,
    Rectangle, Row, Size, Text, Widget, current_window, open_popup, pointer_position,
};

use crate::buttons::{Actions, interactive, state_fill};
use crate::card::{Card, Surface};
use crate::icons::{ICON_CHECK, ICON_EXPAND_MORE};
use crate::layout::{boxed, icon, pad, text_width};
use crate::tokens::{Styled, sys};

/// What an open menu needs once its dropdown has been rebuilt away.
struct Menu {
    options: Vec<String>,
    selected: usize,
    on_select: Rc<dyn Fn(usize)>,
    open: bool,
}

thread_local! {
    // each dropdown's options and handler, refreshed every time it's built
    static MENUS: RefCell<HashMap<String, Menu>> = RefCell::new(HashMap::new());
}

/// Where the menu opens over.
#[derive(Clone)]
enum Parent {
    Window(String),
    Layer(&'static str),
}

/// A dropdown. `id` keys its hover and its menu, so it must be unique and
/// stable.
pub struct Dropdown {
    id: String,
    options: Vec<String>,
    selected: usize,
    width: Option<f32>,
    disabled: bool,
    layer: Option<&'static str>,
    on_select: Rc<dyn Fn(usize)>,
}

impl Dropdown {
    pub fn new(id: impl Into<String>, options: Vec<String>, selected: usize) -> Self {
        Self {
            id: id.into(),
            options,
            selected,
            width: None,
            disabled: false,
            layer: None,
            on_select: Rc::new(|_| {}),
        }
    }

    /// As wide as this; by default as wide as the longest option.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Dimmed and unclickable.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The namespace of the layer window it's on, for a dropdown outside a
    /// normal window.
    pub fn layer(mut self, namespace: &'static str) -> Self {
        self.layer = Some(namespace);
        self
    }

    /// Runs with the index of the option chosen.
    pub fn on_select(mut self, select: impl Fn(usize) + 'static) -> Self {
        self.on_select = Rc::new(select);
        self
    }

    pub fn build(self) -> Rectangle {
        let style = sys::r#type::label();
        let fg = sys::color::surface::panel::fg();
        let inset = sys::space::inset::s();
        let glyph = sys::size::icon::s();

        let widest = self
            .options
            .iter()
            .map(|option| text_width(option, style))
            .fold(0.0, f32::max);

        let width = self
            .width
            .unwrap_or(widest + glyph + inset * 2.0 + sys::space::gap::s());

        let shown = self.options.get(self.selected).cloned().unwrap_or_default();

        let content = Row::new(vec![
            boxed(Text::new(shown).style(style).color(fg).elide()),
            // the chevron says a list opens
            boxed(icon(
                ICON_EXPAND_MORE,
                glyph,
                sys::color::content::secondary(),
            )),
        ])
        .width(Size::Parent)
        .gap(sys::space::gap::s())
        .align(Center);

        let base = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(sys::size::control::height()))
            .radius(Radius::Fixed(sys::radius::raised()))
            .fill(state_fill(
                &self.id,
                sys::color::surface::raised::bg(),
                self.disabled,
            ))
            .border(sys::border::hairline(), sys::color::separator())
            .padding(pad(inset, 0.0))
            .align_child(amane::Start, Center)
            .child(content);

        let parent = match (self.layer, current_window()) {
            (Some(namespace), _) => Parent::Layer(namespace),
            (None, window) => Parent::Window(window),
        };

        let id = self.id.clone();

        // the menu reads these while open, after this dropdown is gone
        MENUS.with_borrow_mut(|menus| {
            let open = menus.get(&id).is_some_and(|menu| menu.open);

            menus.insert(
                id.clone(),
                Menu {
                    options: self.options.clone(),
                    selected: self.selected,
                    on_select: self.on_select.clone(),
                    open,
                },
            );
        });

        let actions = Actions {
            press: Some(Rc::new(move || open(&id, parent.clone()))),
            ..Actions::default()
        };

        interactive(base, self.id, self.disabled, actions)
    }
}

fn open(id: &str, parent: Parent) {
    let at = pointer_position();

    MENUS.with_borrow_mut(|menus| {
        if let Some(menu) = menus.get_mut(id) {
            menu.open = true;
        }
    });

    let id = id.to_string();

    open_popup(move || menu(&id, &parent, at));
}

fn close(id: &str) {
    MENUS.with_borrow_mut(|menus| {
        if let Some(menu) = menus.get_mut(id) {
            menu.open = false;
        }
    });

    // the popup goes once its view says so, on the next turn of the loop
    amane::mark_all();
}

/// The open menu: one row per option, the chosen one checked.
fn menu(id: &str, parent: &Parent, at: Point) -> Popup {
    let (options, selected, open) = MENUS.with_borrow(|menus| {
        menus
            .get(id)
            .map(|menu| (menu.options.clone(), menu.selected, menu.open))
            .unwrap_or_default()
    });

    let style = sys::r#type::label();
    let inset = sys::space::inset::s();
    let glyph = sys::size::icon::s();
    let row_height = sys::size::row::height();
    let card_padding = sys::space::gap::xs();

    let widest = options
        .iter()
        .map(|option| text_width(option, style))
        .fold(0.0, f32::max);

    let width = widest + glyph + inset * 2.0 + sys::space::gap::s() + card_padding * 2.0;
    let height = row_height * options.len() as f32 + card_padding * 2.0;

    let rows: Vec<Box<dyn Widget>> = options
        .iter()
        .enumerate()
        .map(|(index, option)| boxed(row(id, index, option, index == selected)))
        .collect();

    let card = Card::new(vec![boxed(Column::new(rows).width(Size::Parent))])
        .surface(Surface::Popover)
        .blur(true)
        .glass(true)
        .padding(card_padding)
        .build();

    let popup = Popup::new("amane.dropdown")
        .size(width.ceil() as u32, height.ceil() as u32)
        .anchor_rect(at.x as i32, at.y as i32, 1, 1)
        .anchor(PopupAnchor::TopLeft)
        .gravity(Gravity::BottomRight)
        .constraint_adjustment(
            ConstraintAdjustment::SLIDE_X
                | ConstraintAdjustment::SLIDE_Y
                | ConstraintAdjustment::FLIP_Y,
        )
        .grab()
        .visible(open);

    let popup = match parent {
        Parent::Window(window) => popup.parent_window(window),
        Parent::Layer(namespace) => popup.parent(namespace),
    };

    let dismissed = id.to_string();
    let escaped = id.to_string();

    popup
        .on_dismiss(move || close(&dismissed))
        .on_key(move |key| {
            if key == Key::Escape {
                close(&escaped);
            }
        })
        .child(card)
}

fn row(id: &str, index: usize, option: &str, chosen: bool) -> Rectangle {
    let fg = sys::color::surface::popover::fg();
    let glyph = sys::size::icon::s();
    let key = format!("{id}.option.{index}");

    // the check sits in its own slot, so labels line up whether checked or not
    let check: Box<dyn Widget> = if chosen {
        boxed(icon(ICON_CHECK, glyph, fg))
    } else {
        boxed(
            Rectangle::new()
                .width(Size::Fixed(glyph))
                .height(Size::Fixed(glyph)),
        )
    };

    let base = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(sys::size::row::height()))
        .radius(Radius::Fixed(sys::radius::raised()))
        .fill(state_fill(&key, amane::Color::TRANSPARENT, false))
        .padding(pad(sys::space::inset::s(), 0.0))
        .align_child(amane::Start, Center)
        .child(
            Row::new(vec![
                check,
                boxed(Text::new(option).style(sys::r#type::label()).color(fg)),
            ])
            .gap(sys::space::gap::s())
            .align(Center),
        );

    let id = id.to_string();

    let actions = Actions {
        press: Some(Rc::new(move || {
            let on_select =
                MENUS.with_borrow(|menus| menus.get(&id).map(|menu| menu.on_select.clone()));

            close(&id);

            if let Some(on_select) = on_select {
                on_select(index);
            }
        })),
        ..Actions::default()
    };

    interactive(base, key, false, actions)
}
