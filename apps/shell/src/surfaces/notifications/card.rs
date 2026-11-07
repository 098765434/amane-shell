//! One notification as a card, shared by the toasts and the center.
//!
//! Laid out as a banner, in three columns: the app icon,
//! centered in its frame on the left; the app name, the summary and up to
//! [`BODY_LINES`] of body in the middle; the time on the right, with the
//! sender's image under it as a thumbnail. The sender's actions run as a row
//! of equal buttons underneath. A hovered card swaps the time for a close
//! button.
//!
//! The card is measured before it is drawn ([`measure`]), so the stacks
//! around it can place and animate cards exactly, and drawing reuses the
//! measurement.

mod actions;
mod parts;

use actions::*;
pub use actions::{close_button, interactive};
use parts::*;
pub use parts::{ago, app_name, plain};

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::services::clock::local;
use amane::{
    Button, Center, Color, Column, End, Image, Notification, Notifications, Padding, Pointer,
    Rectangle, Row, Start, Text, Urgency, Widget,
};

use crate::tokens::{Styled, notification, sys};
use amane_kit::app_icon::{icon_for, tile as app_icon_tile};
use amane_kit::buttons::{IconButton, PushButton, Style};
use amane_kit::icons::ICON_CLOSE;
use amane_kit::layout::{blank, boxed, frame, inner_width, text_width};
use amane_kit::material::{self, FillLevel, Level, Material};

/// The body is cut after this many lines, like a banner's.
const BODY_LINES: usize = 2;

/// The widest times [`ago`] gives, which the time column is sized to fit.
const WIDEST_TIMES: [&str; 2] = ["59m ago", "Wed 23:59"];

/// Where a card is drawn, which decides its width and material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// On its own frosted surface, as a banner.
    Toast,
    /// On the center's frosted sidebar, as a raised card.
    Center,
}

struct Look {
    fill: Color,
    fg: Color,
    secondary: Color,
    stroke: Color,
}

impl Place {
    pub fn width(self) -> f32 {
        match self {
            Place::Toast => notification::toast::width(),
            Place::Center => inner_width(notification::center::width()),
        }
    }

    fn look(self) -> Look {
        match self {
            // a banner of its own: the compositor frosts behind it
            // (`toasts::slot`) and, where it has the material, lays on the
            // tint and edge
            Place::Toast => Look {
                fill: Material::Banner.fill(),
                fg: material::label(Level::Primary),
                secondary: material::label(Level::Secondary),
                stroke: Material::Banner.stroke(material::separator()),
            },
            // a raised card on the center's panel sheet, in its colours
            Place::Center => Look {
                fill: material::fill_role(FillLevel::Secondary),
                fg: material::label(Level::Primary),
                secondary: material::label(Level::Secondary),
                stroke: Color::TRANSPARENT,
            },
        }
    }
}

thread_local! {
    // the card under the pointer, which shows its close button
    static HOVERED: Cell<Option<u32>> = const { Cell::new(None) };
}

/// The pointer entered or left card `id`. Kept apart from the buttons'
/// hover, which the close button inside the card takes over.
fn hover(id: u32, inside: bool) {
    HOVERED.with(|hovered| {
        if inside {
            hovered.set(Some(id));
        } else if hovered.get() == Some(id) {
            hovered.set(None);
        }
    });
}

/// The room between a card's edge and what's on it: more on the trailing
/// side, where the time sits, as in a banner.
fn padding() -> Padding {
    let block = notification::card::padding_block();
    Padding {
        top: block,
        right: notification::card::padding_end(),
        bottom: block,
        left: notification::card::padding_start(),
    }
}

// ───── layout ────────────────────────────────────────────────────────────────

/// The text a card shows, cleaned, and how big each part is.
#[derive(Clone)]
pub struct Layout {
    summary: String,
    body: String,
    /// The sender's image, shown as a thumbnail under the time.
    image: Option<String>,
    text_width: f32,
    trailing_width: f32,
    line: f32,
    title_line: f32,
    time_line: f32,
    body_height: f32,
    /// The three columns' height, which the icon centers in.
    content: f32,
    /// The whole card's height.
    pub height: f32,
}

/// `n`'s card in `place`, measured: how tall it is, and what it shows.
pub fn measure(n: &Notification, place: Place) -> Layout {
    let width = place.width();
    let padding = padding();
    let gap = sys::space::gap::m();

    let summary = one_line(&plain(n.summary()));
    let body = plain(n.body());
    let image = image_path(n);

    let line = Text::new("Ag")
        .style(sys::r#type::caption())
        .height_in(f32::MAX);
    let title_line = Text::new("Ag")
        .style(notification::card::title())
        .height_in(f32::MAX);
    let time_line = Text::new("Ag")
        .style(notification::card::time())
        .height_in(f32::MAX);

    // as wide as the widest time, so the text doesn't move as it ages, and
    // wide enough for the thumbnail and the close button that stands in
    let trailing_width = WIDEST_TIMES
        .iter()
        .map(|sample| text_width(sample, notification::card::time()))
        .fold(close_side(time_line), f32::max)
        .max(notification::card::thumbnail_size());

    let text_width = (width
        - padding.left
        - padding.right
        - notification::card::icon_frame()
        - trailing_width
        - gap * 2.0)
        .max(1.0);

    let body_height = if body.is_empty() {
        0.0
    } else {
        body_text(&body, Color::TRANSPARENT).height_in(text_width)
    };

    let mut text = line;
    if !summary.is_empty() {
        text += sys::space::gap::_2xs() + title_line;
    }
    text += body_height;

    // the image's room is kept from the start, so the card doesn't grow
    // when it finishes loading
    let mut trailing = time_line;
    if image.is_some() {
        trailing += notification::card::trailing_gap() + notification::card::thumbnail_size();
    }

    let content = text.max(notification::card::icon_size()).max(trailing);

    let mut height = padding.top + padding.bottom + content;
    if has_actions(n) {
        height += gap + sys::size::control::height();
    }

    Layout {
        summary,
        body,
        image,
        text_width,
        trailing_width,
        line,
        title_line,
        time_line,
        body_height,
        content,
        height,
    }
}

/// A notification with its card measured, for the lists that place cards.
#[derive(Clone)]
pub struct Measured {
    pub notification: Notification,
    pub layout: Layout,
}

impl Measured {
    pub fn new(notification: Notification, place: Place) -> Self {
        let layout = measure(&notification, place);
        Self {
            notification,
            layout,
        }
    }
}

fn body_text(body: &str, color: Color) -> Text {
    Text::new(body)
        .style(notification::card::body())
        .color(color)
        .wrap()
        .max_lines(BODY_LINES)
        .elide()
}

/// The close button's side: a little more than the time it stands in for.
fn close_side(time_line: f32) -> f32 {
    time_line + sys::space::gap::xs()
}

// ───── view ──────────────────────────────────────────────────────────────────

/// `m`'s card, its content at `content` opacity (0 for a blank plate
/// stacked behind another). Callers make it clickable with [`interactive`].
pub fn card(m: &Measured, place: Place, now: SystemTime, content: f32) -> Rectangle {
    let n = &m.notification;
    let parts = &m.layout;
    let width = place.width();
    let look = place.look();
    let id = n.id();
    let hovered = HOVERED.with(Cell::get) == Some(id);
    let padding = padding();

    let fixed = |w: f32, h: f32| frame(w, h).clip();

    // the middle: the app's name, the summary, and the body
    let app = fixed(parts.text_width, parts.line)
        .align_child(Start, Center)
        .child(
            Text::new(app_name(n))
                .style(sys::r#type::caption())
                .color(look.secondary)
                .elide(),
        );

    let mut text: Vec<Box<dyn Widget>> = vec![boxed(app)];
    if !parts.summary.is_empty() {
        text.push(boxed(fixed(parts.text_width, sys::space::gap::_2xs())));
        text.push(boxed(
            fixed(parts.text_width, parts.title_line)
                .align_child(Start, Center)
                .child(
                    Text::new(parts.summary.clone())
                        .style(notification::card::title())
                        .color(look.fg)
                        .elide(),
                ),
        ));
    }
    if parts.body_height > 0.0 {
        text.push(boxed(
            fixed(parts.text_width, parts.body_height).child(body_text(&parts.body, look.fg)),
        ));
    }

    // the right: the time (or close, while hovered), then the image
    let time: Box<dyn Widget> = if hovered {
        let side = close_side(parts.time_line);
        let close = close_button(
            format!("amane.notification.{id}.close"),
            side,
            look.secondary,
            move || Notifications::dismiss(id),
        );
        // a time line tall, like the time it replaces, so the column doesn't
        // grow (and the card measured without it doesn't shift); the button
        // spills evenly above and below
        boxed(
            frame(side, parts.time_line)
                .align_child(Center, Center)
                .child(close),
        )
    } else {
        boxed(
            Text::new(ago(n.received(), now))
                .style(notification::card::time())
                .color(look.secondary),
        )
    };
    let mut trailing: Vec<Box<dyn Widget>> = vec![time];
    if let Some(image) = parts.image.as_deref().filter(|path| Image::loaded(path)) {
        let side = notification::card::thumbnail_size();
        trailing.push(boxed(
            Rectangle::new()
                .width(side)
                .height(side)
                .radius(notification::card::thumbnail_radius())
                .fill(Image::cover(image)),
        ));
    }

    let top = Row::new(vec![
        leading(n, parts.content),
        boxed(Column::new(text)),
        boxed(
            fixed(parts.trailing_width, parts.content)
                .align_child(End, Start)
                .child(
                    Column::new(trailing)
                        .gap(notification::card::trailing_gap())
                        .align(End),
                ),
        ),
    ])
    .gap(sys::space::gap::m());

    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(top)];
    if has_actions(n) {
        rows.push(boxed(actions(
            n,
            width - padding.left - padding.right,
            look.fg,
        )));
    }

    let mut body = blank().child(Column::new(rows).gap(sys::space::gap::m()));
    if content < 1.0 {
        body = body.opacity(content.clamp(0.0, 1.0));
    }

    let stroke = if critical(n) {
        sys::color::status::critical::bg()
    } else {
        look.stroke
    };

    let card = Rectangle::new()
        .width(width)
        .height(parts.height)
        .radius(notification::card::radius())
        .fill(look.fill)
        .border(sys::border::hairline(), stroke)
        .padding(padding)
        .clip()
        .child(body);

    // a toast is its own translucent surface; a card in the center sits on one
    match place {
        Place::Toast => Material::Banner.lit(card, notification::card::radius()),
        Place::Center => card,
    }
}

/// Whether `n` is critical, which shows until acted on and through Do Not
/// Disturb.
pub fn critical(n: &Notification) -> bool {
    n.urgency() == Urgency::Critical
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_become_paths() {
        assert_eq!(
            file_path("file:///home/me/Screenshot%20from%202026-10-26%2009-09-47.png"),
            "/home/me/Screenshot from 2026-10-26 09-09-47.png"
        );
        assert_eq!(file_path("/plain/path.png"), "/plain/path.png");
        assert_eq!(file_path("file:///a%2"), "/a%2");
        assert_eq!(file_path("file:///caf%C3%A9.png"), "/café.png");
    }

    #[test]
    fn markup_is_stripped() {
        assert_eq!(plain("<b>Bold</b> and <i>it</i>"), "Bold and it");
        assert_eq!(plain("<a href=\"x\">link</a>"), "link");
        assert_eq!(plain("a &lt;b&gt; &amp; c"), "a <b> & c");
        assert_eq!(plain("&amp;lt;"), "&lt;");
        assert_eq!(plain("  padded \n"), "padded");
        assert_eq!(plain("1 < 2 > 0"), "1 < 2 > 0");
    }

    #[test]
    fn summaries_are_one_line() {
        assert_eq!(one_line("two\nlines  here"), "two lines here");
    }

    #[test]
    fn times_read_in_words() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100_000);
        let before = |secs| now - Duration::from_secs(secs);

        assert_eq!(ago(before(0), now), "just now");
        assert_eq!(ago(before(59), now), "just now");
        assert_eq!(ago(before(60), now), "1 min ago");
        assert_eq!(ago(before(59 * 60), now), "59 min ago");
        assert_eq!(ago(before(3 * 3600), now), "3 h ago");
        // a clock that went backwards still reads as now
        assert_eq!(ago(now + Duration::from_secs(5), now), "just now");
    }
}
