//! Toast — a short word that something was done, in a pill near the window's bottom edge,
//! with at most one button ("Removed Ctrl+K · **Undo**"). It goes by itself after
//! [`SHOWN_FOR`]; a newer one takes its place. Drawn as a pill, the
//! text 14 px in, a soft shadow and a light inner edge.
//!
//! An app calls [`show`] where the thing is done and lays [`over`] over its window's content.
//! For something that leaves its own trace on the page (a switch, a row that appears) a toast
//! says nothing new; it's for what would otherwise happen silently: a save, a removal, a reset.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use amane::{Center, Color, Radius, Rectangle, Row, Size, Stack, Text, Widget};

use crate::buttons::{PushButton, Style};
use crate::color::with_alpha;
use crate::layout::{boxed, text_height};
use crate::style::{self, px};
use crate::tokens::{Styled, sys};

/// How long a toast stays.
pub const SHOWN_FOR: Duration = Duration::from_secs(4);

struct Toast {
    text: String,
    action: Option<(String, Rc<dyn Fn()>)>,
    since: Instant,
}

thread_local! {
    static SHOWING: RefCell<Option<Toast>> = const { RefCell::new(None) };
}

/// Shows `text` for [`SHOWN_FOR`], in place of any toast showing.
pub fn show(text: impl Into<String>) {
    put(text.into(), None);
}

/// Shows `text` with a button, `label`, that runs `action` and puts the toast away.
pub fn show_with(text: impl Into<String>, label: impl Into<String>, action: impl Fn() + 'static) {
    put(text.into(), Some((label.into(), Rc::new(action))));
}

/// [`show`] from any thread: a job that finishes on its own thread (a
/// save through the shell, say) says it's done on the UI thread's next turn.
pub fn post(text: impl Into<String>) {
    let text = text.into();
    amane::Timer::after(Duration::ZERO, move || show(text));
}

/// Puts the toast away now.
pub fn dismiss() {
    SHOWING.with_borrow_mut(|showing| *showing = None);
    amane::mark_all();
}

fn put(text: String, action: Option<(String, Rc<dyn Fn()>)>) {
    SHOWING.with_borrow_mut(|showing| {
        *showing = Some(Toast {
            text,
            action,
            since: Instant::now(),
        });
    });
    amane::mark_all();
    // drawn again once it's due to go, with no frames in between
    amane::redraw_after(SHOWN_FOR);
}

/// The text of the toast showing now, if one is.
pub fn showing() -> Option<String> {
    showing_at(Instant::now())
}

fn showing_at(now: Instant) -> Option<String> {
    SHOWING.with_borrow(|showing| {
        showing
            .as_ref()
            .filter(|toast| now.duration_since(toast.since) < SHOWN_FOR)
            .map(|toast| toast.text.clone())
    })
}

/// The pill for the toast showing now, if one is.
fn pill() -> Option<Rectangle> {
    let now = Instant::now();
    let (text, action) = SHOWING.with_borrow(|showing| {
        showing
            .as_ref()
            .filter(|toast| now.duration_since(toast.since) < SHOWN_FOR)
            .map(|toast| (toast.text.clone(), toast.action.clone()))
    })?;

    let font = sys::r#type::label();
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(
        Text::new(text.clone())
            .style(font)
            .color(style::label_primary())
            .elide(),
    )];
    let mut width = crate::layout::text_width(&text, font) + px(28.0);
    if let Some((label, action)) = action {
        let button = PushButton::new("kit.toast.action", label)
            .style(Style::Tonal)
            .on_press(move || {
                action();
                dismiss();
            })
            .build();
        width += match button.width() {
            Size::Fixed(w) => w + px(14.0),
            Size::Parent => 0.0,
        };
        parts.push(boxed(button));
    }

    let height = (text_height(font) + px(14.0)).max(style::control_height() + px(10.0));
    Some(
        Rectangle::new()
            .width(width)
            .height(height)
            .radius(Radius::Full)
            .fill(style::menu_fill())
            .border(1.0, with_alpha(Color::WHITE, 0.1))
            .shadow(
                amane::Shadow::drop(Color::BLACK)
                    .opacity(0.35)
                    .blur(8.0)
                    .offset(0.0, 3.0),
            )
            .padding(crate::layout::pad_x(px(14.0), px(7.0)))
            .align_child(amane::Start, Center)
            .child(Row::new(parts).gap(px(14.0)).align(Center)),
    )
}

/// `content`, with the toast showing now laid over it, centred 21 px above its bottom edge.
pub fn over(content: impl Widget + 'static) -> Stack {
    let mut layers: Vec<Box<dyn Widget>> = vec![boxed(content)];
    if let Some(pill) = pill() {
        layers.push(boxed(
            Rectangle::new()
                .width(Size::Parent)
                .height(Size::Parent)
                .padding(amane::Padding {
                    bottom: px(21.0),
                    ..amane::Padding::default()
                })
                .align_child(Center, amane::End)
                .child(pill),
        ));
    }
    Stack::new(layers).width(Size::Parent).height(Size::Parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_toast_goes_by_itself_and_a_newer_one_takes_its_place() {
        show("Saved");
        let start = Instant::now();
        assert_eq!(showing_at(start).as_deref(), Some("Saved"));
        assert_eq!(
            showing_at(start + SHOWN_FOR + Duration::from_millis(50)),
            None
        );
        show_with("Removed Ctrl+K", "Undo", || {});
        assert_eq!(showing().as_deref(), Some("Removed Ctrl+K"));
        assert!(pill().is_some());
        dismiss();
        assert_eq!(showing(), None);
        assert!(pill().is_none());
    }
}
