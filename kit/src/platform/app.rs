//! What an app beside the shell needs to look like the shell: its window,
//! its text fields, a dialog ([`Dialog`], from [`crate::dialog`]) and a
//! sidebar to move between pages.
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

use std::rc::Rc;

use amane::{
    App, Center, Color, Column, Key, Mask, Radius, Rectangle, Row, Size, Text, TextInput, Widget,
    Window, close_window, current_window, modifiers, window_state,
};

use crate::buttons::{Actions, interactive, state_fill};
use crate::color::{Accent, with_alpha};
use crate::layout::{boxed, caption, icon, pad, text_height};
use crate::macform::theme as mac;
use crate::macform::{MacLeadingIcon, text_field};
use crate::material::{self, Level, Material};
use crate::tokens::{Styled, sys};

pub use crate::dialog::Dialog;

/// An app with this app-id: its windows, IPC socket and settings folder go
/// by it, and only one copy of it runs.
pub fn app(app_id: &str) -> App {
    App::new().app_id(app_id)
}

/// What an app window is drawn on: the opaque window background
/// (`sys.color.window.bg`), as macOS draws a window's content. Only the
/// [`sidebar`] shows the compositor's frost behind the window.
pub fn window_bg() -> Color {
    sys::color::window::bg()
}

/// Whether the compositor frosts behind app windows. The window's own
/// material being offered is the sign: the shell's generated config defines
/// it with the window rule that frosts the window; a stand-in material
/// doesn't count.
pub(crate) fn frosted() -> bool {
    Material::Window.niri() == Some(Material::Window.niri_name())
}

/// A normal window in the shell's look: its surface and padding behind
/// `content`. Set the rest (title, size, shortcuts …) on the `Window` it
/// gives back. A [`sidebar`] in `content` cuts itself out of the surface,
/// so nothing between the two may be a `Rectangle`.
pub fn app_window(content: impl Widget + 'static) -> Window {
    Window::new().vibrant(material::vibrant()).child(
        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Parent)
            .fill(window_bg())
            .padding(pad(sys::space::inset::l(), sys::space::inset::l()))
            .child(content),
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
        .size(mac::control_font().size)
        .color(mac::label_primary())
        .selection_color(with_alpha(mac::tint(), 0.4))
}

/// The box a [`text_field`] sits in, drawn like macOS's (the macform
/// look): radius 6, `fieldFill` in a hairline that takes the accent while
/// the field has the keys.
pub fn field_box(id: &'static str, input: TextInput, width: impl Into<Size>) -> Rectangle {
    text_field::frame(input, width, text_field::edge(id))
}

/// One row in a [`sidebar`]: a page, with its glyph and its name, or a
/// section's title over the pages after it.
pub struct Page {
    pub glyph: Option<char>,
    pub name: String,
    /// A title over a run of pages, not a page: it can't be selected.
    pub section: bool,
    /// The tile its glyph sits on, as macOS's System Settings draws its
    /// sidebar; with none the glyph is drawn bare.
    pub accent: Option<Accent>,
}

impl Page {
    pub fn new(glyph: char, name: impl Into<String>) -> Self {
        Self {
            glyph: Some(glyph),
            name: name.into(),
            section: false,
            accent: None,
        }
    }

    /// Its glyph on a tile of `accent`.
    pub fn accent(mut self, accent: Accent) -> Self {
        self.accent = Some(accent);
        self
    }

    /// A section's title, as macOS's sidebar groups its pages. It takes an
    /// index like a page, so one array still names every row.
    pub fn section(name: impl Into<String>) -> Self {
        Self {
            glyph: None,
            name: name.into(),
            section: true,
            accent: None,
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

            if page.section {
                // a title over its pages, set off from the pages above it
                let above = if index == 0 {
                    sys::space::gap::xs()
                } else {
                    sys::space::gap::m()
                };

                let height = above + text_height(sys::r#type::caption());

                return boxed(
                    Rectangle::new()
                        .width(Size::Parent)
                        .height(Size::Fixed(height))
                        .fill(Color::TRANSPARENT)
                        .padding(pad(sys::space::inset::s(), 0.0))
                        .align_child(amane::Start, amane::End)
                        .child(caption(page.name.clone())),
                );
            }

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
                        match page.accent {
                            Some(accent) => boxed(
                                MacLeadingIcon::new(page.glyph.unwrap_or(' '))
                                    .accent(accent)
                                    .build(),
                            ),
                            None => {
                                boxed(icon(page.glyph.unwrap_or(' '), sys::size::icon::s(), fg))
                            }
                        },
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
        (false, _) => Material::Sidebar.tint(),
        (true, true) => sys::color::sidebar::bg::active(),
        (true, false) => sys::color::sidebar::bg::inactive(),
    };

    let radius = Radius::Fixed(sys::radius::popover());
    let sidebar = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .radius(radius)
        .fill(bg)
        .padding(pad(sys::space::inset::s(), sys::space::inset::s()))
        .child(
            Column::new(rows)
                .gap(sys::space::gap::_2xs())
                .width(Size::Parent),
        );

    if opaque {
        return sidebar;
    }

    // a hole in the window's opaque surface, down to the frost behind the
    // window, as a macOS sidebar is the one behind-window part
    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .radius(radius)
        .fill(Mask)
        .child(sidebar)
}
