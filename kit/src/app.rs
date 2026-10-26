//! What an app beside the shell needs to look like the shell: its window,
//! its text fields, a dialog and a sidebar to move between pages.
//!
//! ```ignore
//! fn main() {
//!     amane_kit::app::app("amane-settings")
//!         .normal_window("main", main_window)
//!         .run();
//! }
//!
//! fn main_window() -> Window {
//!     app_window(content())
//!         .title("Settings")
//!         .size(720.0, 480.0)
//!         .on_shortcut(standard_shortcut)
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use amane::{
    App, Center, Color, Column, End, Key, Radius, Rectangle, Row, Size, Text, TextInput, Widget,
    Window, close_window, current_window, modifiers, open_window_with, window_state,
};

use crate::buttons::{Actions, PushButton, Shape, Style, interactive, state_fill};
use crate::color::with_alpha;
use crate::layout::{boxed, icon, pad};
use crate::material::{self, FillLevel, Level, Material};
use crate::tokens::{Styled, sys};

/// An app with this app-id: its windows, IPC socket and settings folder go
/// by it, and only one copy of it runs.
pub fn app(app_id: &str) -> App {
    App::new().app_id(app_id)
}

/// What an app window is drawn on: the window material's tint where the
/// compositor frosts behind the window, else the opaque window background
/// (`sys.color.window.bg`), as there's nothing frosted to see through to.
pub fn window_bg() -> Color {
    if frosted() {
        Material::Window.tint()
    } else {
        sys::color::window::bg()
    }
}

/// Whether the compositor frosts behind app windows. The window's own
/// material being offered is the sign: the shell's generated config defines
/// it with the window rule that frosts the window; a stand-in material
/// doesn't count.
fn frosted() -> bool {
    Material::Window.niri() == Some(Material::Window.niri_name())
}

/// A normal window in the shell's look: its surface and padding behind
/// `content`, dimmed a little while another window has the focus. Set the
/// rest (title, size, shortcuts …) on the `Window` it gives back.
pub fn app_window(content: impl Widget + 'static) -> Window {
    let focused = window_state().focused;

    // TODO(token): `sys.opacity.inactive` once Penpot has it
    let opacity = if focused { 1.0 } else { 0.85 };

    Window::new().vibrant(material::vibrant()).child(
        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Parent)
            .fill(window_bg())
            .padding(pad(sys::space::inset::l(), sys::space::inset::l()))
            .child(
                Rectangle::new()
                    .width(Size::Parent)
                    .height(Size::Parent)
                    .opacity(opacity)
                    .child(content),
            ),
    )
}

/// The shortcuts every window has: Ctrl+W closes it. Call it from your own
/// `on_shortcut`, and handle what it leaves (`false`) yourself.
pub fn standard_shortcut(key: Key) -> bool {
    let held = modifiers();

    if !held.ctrl || held.alt || held.logo {
        return false;
    }

    match key {
        Key::Character('w') => {
            close_window(&current_window());

            true
        }

        _ => false,
    }
}

/// A one-line text field: a rounded box around a `TextInput`, ringed in the
/// accent while it takes the keys. `id` is the input's, so its text is kept
/// under it.
pub fn text_field(id: &'static str, placeholder: &str) -> TextInput {
    TextInput::new(id)
        .placeholder(placeholder)
        .size(sys::r#type::label().size)
        .color(material::label(Level::Primary))
        .selection_color(with_alpha(sys::color::accent::bg(), 0.35))
}

/// The box a [`text_field`] sits in.
pub fn field_box(id: &'static str, input: TextInput, width: impl Into<Size>) -> Rectangle {
    let ring = if TextInput::has_focus(id) {
        (sys::border::hairline() * 2.0, sys::color::accent::bg())
    } else {
        (sys::border::hairline(), material::separator())
    };

    Rectangle::new()
        .width(width)
        .height(Size::Fixed(sys::size::control::height()))
        .radius(Radius::Fixed(sys::radius::raised()))
        .fill(material::fill_role(FillLevel::Secondary))
        .border(ring.0, ring.1)
        .padding(pad(sys::space::inset::s(), 0.0))
        .align_child(amane::Start, Center)
        .clip()
        .child(input)
}

/// One page in a [`sidebar`]: its glyph and its name.
pub struct Page {
    pub glyph: char,
    pub name: String,
}

impl Page {
    pub fn new(glyph: char, name: impl Into<String>) -> Self {
        Self {
            glyph,
            name: name.into(),
        }
    }
}

/// The pages of a settings-style window, one row each, the shown one filled.
/// `on_select` gets the index of a row clicked. Over a frosted window it's
/// the window's own glass; on an opaque one it's a panel of the sidebar's
/// colour, paler and with greyer labels while another window has the focus.
pub fn sidebar(
    id: &str,
    pages: &[Page],
    selected: usize,
    on_select: impl Fn(usize) + 'static,
) -> Rectangle {
    let on_select = Rc::new(on_select);
    let focused = window_state().focused;
    let opaque = !frosted();

    let rows: Vec<Box<dyn Widget>> = pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let key = format!("{id}.{index}");
            let chosen = index == selected;

            let fill = if chosen {
                sys::color::state::selected()
            } else {
                Color::TRANSPARENT
            };

            let fg = if opaque && !focused {
                sys::color::sidebar::label::inactive()
            } else {
                material::label(Level::Primary)
            };

            let base = Rectangle::new()
                .width(Size::Parent)
                .height(Size::Fixed(sys::size::row::height()))
                .radius(Radius::Fixed(sys::radius::raised()))
                .fill(state_fill(&key, fill, false))
                .padding(pad(sys::space::inset::s(), 0.0))
                .align_child(amane::Start, Center)
                .child(
                    Row::new(vec![
                        boxed(icon(page.glyph, sys::size::icon::s(), fg)),
                        boxed(
                            Text::new(page.name.clone())
                                .style(sys::r#type::label())
                                .color(fg),
                        ),
                    ])
                    .gap(sys::space::gap::s())
                    .align(Center),
                );

            let select = on_select.clone();

            let actions = Actions {
                press: Some(Rc::new(move || select(index))),
                ..Actions::default()
            };

            boxed(interactive(base, key, false, actions))
        })
        .collect();

    let bg = match (opaque, focused) {
        (false, _) => Color::TRANSPARENT,
        (true, true) => sys::color::sidebar::bg::active(),
        (true, false) => sys::color::sidebar::bg::inactive(),
    };

    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .radius(Radius::Fixed(sys::radius::popover()))
        .fill(bg)
        .padding(pad(sys::space::inset::s(), sys::space::inset::s()))
        .child(
            Column::new(rows)
                .gap(sys::space::gap::_2xs())
                .width(Size::Parent),
        )
}

/// A question over a window, answered with one of its buttons: "Discard
/// unsaved changes?" with Cancel and Discard. It keeps the user out of its
/// window until answered, where the compositor can.
pub struct Dialog {
    name: String,
    title: String,
    message: String,
    buttons: Vec<(String, Style, Rc<dyn Fn()>)>,
}

thread_local! {
    // each open dialog's buttons, which its view reads after it's built
    static BUTTONS: RefCell<HashMap<String, Vec<(String, Style, Rc<dyn Fn()>)>>> =
        RefCell::new(HashMap::new());
}

impl Dialog {
    /// `name` is the dialog's window's name, so it must be unique.
    pub fn new(name: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: title.into(),
            message: String::new(),
            buttons: Vec::new(),
        }
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// A button that closes the dialog and runs `action`; plain.
    pub fn button(mut self, label: impl Into<String>, action: impl Fn() + 'static) -> Self {
        self.buttons
            .push((label.into(), Style::Tonal, Rc::new(action)));
        self
    }

    /// The one main answer, in the accent; it goes last, on the right.
    pub fn main_button(mut self, label: impl Into<String>, action: impl Fn() + 'static) -> Self {
        self.buttons
            .push((label.into(), Style::Filled, Rc::new(action)));
        self
    }

    /// Opens it over the window with this name.
    pub fn open(self, parent: &str) {
        let Self {
            name,
            title,
            message,
            buttons,
        } = self;

        BUTTONS.with_borrow_mut(|open| {
            open.insert(name.clone(), buttons);
        });

        let parent = parent.to_string();
        let key = name.clone();

        open_window_with(name, move || dialog(&key, &parent, &title, &message));
    }
}

fn dialog(name: &str, parent: &str, title: &str, message: &str) -> Window {
    let buttons = BUTTONS.with_borrow(|open| open.get(name).cloned().unwrap_or_default());

    let fg = material::label(Level::Primary);

    let row: Vec<Box<dyn Widget>> = buttons
        .into_iter()
        .enumerate()
        .map(|(index, (label, style, action))| {
            let closing = name.to_string();

            boxed(
                PushButton::new(format!("{name}.button.{index}"), label)
                    .style(style)
                    .shape(Shape::Round)
                    .on_press(move || {
                        close_window(&closing);
                        BUTTONS.with_borrow_mut(|open| open.remove(&closing));

                        action();
                    })
                    .build(),
            )
        })
        .collect();

    let content = Column::new(vec![
        boxed(Text::new(title).style(sys::r#type::title()).color(fg)),
        boxed(
            Text::new(message)
                .style(sys::r#type::label())
                .color(material::label(Level::Secondary))
                .wrap(),
        ),
        boxed(
            Row::new(row)
                .gap(sys::space::gap::s())
                .justify(End)
                .width(Size::Parent),
        ),
    ])
    .gap(sys::space::gap::m())
    .width(Size::Parent)
    .height(Size::Parent);

    let closed = name.to_string();

    app_window(content)
        .title(title)
        .size(380.0, 170.0)
        .resizable(false)
        .parent(parent)
        .modal(true)
        // closing it from the compositor is the same as no answer
        .on_close(move || {
            BUTTONS.with_borrow_mut(|open| open.remove(&closed));

            true
        })
        .on_shortcut(standard_shortcut)
}
