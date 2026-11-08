//! What an app beside the shell needs to look like the shell: its window,
//! its text fields, a dialog ([`Dialog`], from [`crate::dialog`]) and a
//! sidebar to move between pages.
//!
//! ```ignore
//! fn main() {
//!     emaki_kit::app::app("emaki-settings")
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
use crate::controls::text_field;
use crate::layout::{boxed, caption, icon, pad, text_height};
use crate::leading_icon::LeadingIcon;
use crate::material::{self, Level, Material};
use crate::style;
use crate::tokens::{Styled, sys};

pub use crate::dialog::Dialog;

/// An app with this app-id: its windows, IPC socket and settings folder go
/// by it, and only one copy of it runs.
pub fn app(app_id: &str) -> App {
    App::new().app_id(app_id)
}

/// What an app window is drawn on: the opaque window background
/// (`sys.color.window.bg`). Only the
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
        .size(style::control_font().size)
        .color(style::label_primary())
        .selection_color(with_alpha(style::tint(), 0.4))
}

/// Puts `text` in the field `id` the first time it's built this run: the
/// field shows its setting, then what's typed, so emptying it leaves it
/// empty rather than filling it again.
pub fn fill_once(id: &'static str, text: impl FnOnce() -> String) {
    use std::cell::RefCell;
    use std::collections::HashSet;
    thread_local! {
        static FILLED: RefCell<HashSet<&'static str>> = RefCell::new(HashSet::new());
    }
    if FILLED.with_borrow_mut(|filled| filled.insert(id)) {
        TextInput::set_text(id, &text());
    }
}

/// Runs `commit` with the field's text when the field `id` has just lost
/// the keys, so what was typed is kept when you click away. Call it
/// while drawing the view that shows the field; amane has no blur event,
/// so this notices the change between one drawing and the next.
pub fn commit_on_blur(id: &'static str, commit: impl FnOnce(String)) {
    use std::cell::RefCell;
    use std::collections::HashSet;
    thread_local! {
        static FOCUSED: RefCell<HashSet<&'static str>> = RefCell::new(HashSet::new());
    }
    let now = TextInput::has_focus(id);
    let was = FOCUSED.with_borrow_mut(|focused| {
        if now {
            !focused.insert(id)
        } else {
            focused.remove(id)
        }
    });
    if was && !now {
        commit(TextInput::text(id));
    }
}

/// The box a [`text_field`] sits in, drawn in the form controls' look:
/// radius 6, `fieldFill` in a hairline that takes the accent while
/// the field has the keys.
pub fn field_box(id: &'static str, input: TextInput, width: impl Into<Size>) -> Rectangle {
    text_field::frame(input, width, text_field::edge(id))
}

/// One row in a [`sidebar`]: a page, with its glyph and its name, or a
/// section's title over the pages after it.
pub struct Page {
    /// The page's glyph, if it has one.
    pub glyph: Option<char>,
    /// The page's name, or the section's title.
    pub name: String,
    /// A title over a run of pages, not a page: it can't be selected.
    pub section: bool,
    /// The tile its glyph sits on, as Emaki Settings draws its sidebar;
    /// with none the glyph is drawn bare.
    pub accent: Option<Accent>,
}

impl Page {
    /// A page named `name` with `glyph`, drawn bare.
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

    /// A section's title, grouping the sidebar's pages. It takes an
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
    sidebar_with(id, pages, selected, on_select, None, &[])
}

/// A [`sidebar`] with `top` over its pages (a search field), showing only
/// the pages `shown` marks true (all of them when it's empty); a section's
/// title shows while any of its pages does.
pub fn sidebar_with(
    id: &str,
    pages: &[Page],
    selected: usize,
    on_select: impl Fn(usize) + 'static,
    top: Option<Box<dyn Widget>>,
    shown: &[bool],
) -> Rectangle {
    let on_select = Rc::new(on_select);
    let visible = |index: usize| shown.is_empty() || shown.get(index).copied().unwrap_or(false);
    // a section shows while a page under it does
    let section_shown = |index: usize| {
        pages[index + 1..]
            .iter()
            .enumerate()
            .take_while(|(_, page)| !page.section)
            .any(|(offset, _)| visible(index + 1 + offset))
    };

    let mut rows: Vec<Box<dyn Widget>> = Vec::new();
    if let Some(top) = top {
        rows.push(top);
    }
    rows.extend(
        pages
            .iter()
            .enumerate()
            .filter(|(index, page)| {
                if page.section {
                    section_shown(*index)
                } else {
                    visible(*index)
                }
            })
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
                            .height(height)
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

                let fg = sidebar_label();

                let base = Rectangle::new()
                    .width(Size::Parent)
                    .height(sys::size::row::height())
                    .radius(Radius::Fixed(sys::radius::raised()))
                    .fill(state_fill(&key, fill, false))
                    .padding(pad(sys::space::inset::s(), 0.0))
                    .align_child(amane::Start, Center)
                    .child(
                        Row::new(vec![
                            match page.accent {
                                Some(accent) => boxed(
                                    LeadingIcon::new(page.glyph.unwrap_or(' '))
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

                let actions = Actions::press(move || select(index));

                boxed(interactive(base, key, false, actions))
            }),
    );

    sidebar_panel(
        Column::new(rows)
            .gap(sys::space::gap::_2xs())
            .width(Size::Parent),
    )
}

/// The colour of a label in a [`sidebar_panel`]: primary, or greyer on
/// an opaque panel while another window has the focus.
pub fn sidebar_label() -> Color {
    if !frosted() && !window_state().focused {
        sys::color::sidebar::label::inactive()
    } else {
        material::label(Level::Primary)
    }
}

/// The panel a [`sidebar`] sits on, around any `content`: a list of
/// conversations, say. Over a frosted window it's the window's own glass;
/// on an opaque one, a panel of the sidebar's colour, paler while another
/// window has the focus. Draw the labels in it with [`sidebar_label`].
pub fn sidebar_panel(content: impl Widget + 'static) -> Rectangle {
    let focused = window_state().focused;
    let opaque = !frosted();

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
        .child(content);

    if opaque {
        return sidebar;
    }

    // a hole in the window's opaque surface, down to the frost behind the
    // window, the sidebar being the one behind-window part
    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .radius(radius)
        .fill(Mask)
        .child(sidebar)
}
