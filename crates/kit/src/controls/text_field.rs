//! A text field's frame: a 24 px box, radius 6, `fieldFill`, the text 8 px in, with no edge
//! at rest. The edge takes the accent while the field has the keys, and a
//! field whose text is wrong or doubtful says so in its [`FieldState`]'s colour: the edge and a
//! faint tint of it. [`app`](crate::app)'s text fields and [`field_box`](crate::app::field_box)
//! draw with it.

use amane::{Center, Color, Rectangle, Size, TextInput};

use crate::color::{Accent, over, with_alpha};
use crate::style::{self, px};
use crate::tokens::sys;

/// What a field's text is, as far as its form can tell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FieldState {
    /// Nothing to say.
    #[default]
    Normal,
    /// Kept, but likely not what was meant.
    Warning,
    /// Can't be kept as it is.
    Error,
}

impl FieldState {
    /// The colour it's said in, if any.
    fn color(self) -> Option<Color> {
        match self {
            Self::Normal => None,
            Self::Warning => Some(Accent::Orange.color()),
            Self::Error => Some(sys::color::status::critical::bg()),
        }
    }
}

/// The edge of field `id`: its state's colour, else the accent while it has
/// the keys, else none.
pub(crate) fn edge(id: &'static str, state: FieldState) -> Color {
    match state.color() {
        Some(color) => color,
        None if TextInput::has_focus(id) => style::tint(),
        None => Color::TRANSPARENT,
    }
}

/// The box a text input sits in: 24 px tall, radius 6, `fieldFill` (tinted
/// in `state`'s colour) in a 1 px `edge`, the text 8 px in.
/// [`field_box`](crate::app::field_box) draws with it too.
pub(crate) fn frame(
    input: TextInput,
    width: impl Into<Size>,
    edge: Color,
    state: FieldState,
) -> Rectangle {
    let fill = match state.color() {
        Some(color) => over(with_alpha(color, 0.15), style::field_fill()),
        None => style::field_fill(),
    };
    Rectangle::new()
        .width(width)
        .height(style::control_height())
        .radius(style::field_radius())
        .fill(fill)
        .border(1.0, edge)
        .padding(crate::layout::pad_x(px(8.0), px(4.0)))
        .align_child(amane::Start, Center)
        .clip()
        .child(input)
}
