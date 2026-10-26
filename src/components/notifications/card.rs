//! One notification as a card, shared by the toasts and the center.
//!
//! Laid out like an iOS/macOS banner, in three columns: the app icon,
//! centered in its frame on the left; the app name, the summary and up to
//! [`BODY_LINES`] of body in the middle; the time on the right, with the
//! sender's image under it as a thumbnail. The sender's actions run as a row
//! of equal buttons underneath. A hovered card swaps the time for a close
//! button.
//!
//! The card is measured before it is drawn ([`measure`]), so the stacks
//! around it can place and animate cards exactly, and drawing reuses the
//! measurement.

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use amane::{
    Button, Center, Color, Column, End, Image, Notification, Notifications, Padding, Pointer,
    Rectangle, Row, Size, Start, Text, Urgency, Widget,
};
use chrono::{DateTime, Local};

use crate::components::app_icon::{icon_for, tile as app_icon_tile};
use crate::components::buttons::{IconButton, PushButton, Style};
use crate::components::icons::ICON_CLOSE;
use crate::components::layout::{blank, boxed, inner_width, text_width};
use crate::components::material::{self, FillLevel, Level, Material};
use crate::tokens::{Styled, notification, sys};

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
    if !n.actions().is_empty() {
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

    let fixed = |w: f32, h: f32| {
        Rectangle::new()
            .width(Size::Fixed(w))
            .height(Size::Fixed(h))
            .fill(Color::TRANSPARENT)
            .clip()
    };

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
            Rectangle::new()
                .width(Size::Fixed(side))
                .height(Size::Fixed(parts.time_line))
                .fill(Color::TRANSPARENT)
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
                .width(Size::Fixed(side))
                .height(Size::Fixed(side))
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
    if !n.actions().is_empty() {
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
        .width(Size::Fixed(width))
        .height(Size::Fixed(parts.height))
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

/// The sender's actions as equal buttons across `width`.
fn actions(n: &Notification, width: f32, color: Color) -> Row {
    let count = n.actions().len() as f32;
    let gap = sys::space::gap::s();
    let each = ((width - gap * (count - 1.0)) / count).max(1.0);
    let id = n.id();

    let buttons = n
        .actions()
        .iter()
        .map(|action| {
            let key = action.key().to_string();
            boxed(
                PushButton::new(
                    format!("amane.notification.{id}.action.{key}"),
                    action.label(),
                )
                .width(each)
                .text(sys::r#type::label())
                .color(color)
                .on_press(move || Notifications::invoke(id, &key))
                .build(),
            )
        })
        .collect();

    Row::new(buttons).gap(gap)
}

/// Makes card `id` clickable: a left click does `press`, and the pointer
/// coming onto it or leaving shows its close button and calls `hover`.
pub fn interactive(
    card: Rectangle,
    id: u32,
    hover_also: impl Fn(bool) + 'static,
    press: impl Fn() + 'static,
) -> Rectangle {
    card.cursor(Pointer)
        .on_hover(move |inside| {
            hover(id, inside);
            hover_also(inside);
        })
        .on_click(move |button| {
            if button == Button::Left {
                press();
            }
        })
}

/// A small tonal close button `side` square, as a card and a group use.
pub fn close_button(id: String, side: f32, color: Color, press: impl Fn() + 'static) -> Rectangle {
    IconButton::new(id, ICON_CLOSE)
        .style(Style::Tonal)
        .size(sys::size::icon::s())
        .color(color)
        .width(side)
        .height(side)
        .on_press(press)
        .build()
}

/// Whether `n` is critical, which shows until acted on and through Do Not
/// Disturb.
pub fn critical(n: &Notification) -> bool {
    n.urgency() == Urgency::Critical
}

/// The app's icon, centered in its frame as tall as the card's content.
fn leading(n: &Notification, height: f32) -> Box<dyn Widget> {
    boxed(
        Rectangle::new()
            .width(Size::Fixed(notification::card::icon_frame()))
            .height(Size::Fixed(height))
            .fill(Color::TRANSPARENT)
            .align_child(Center, Center)
            .child(app_icon(n, notification::card::icon_size())),
    )
}

/// The sender's own image, as a path: the image it sent, else an icon it
/// sent as a file rather than a theme name, which the spec ranks after the
/// image hints (niri sends its screenshots that way).
fn image_path(n: &Notification) -> Option<String> {
    let sent = if n.image().is_empty() {
        icon_file(n)?
    } else {
        n.image()
    };
    let path = file_path(sent);
    (!path.is_empty()).then_some(path)
}

/// The icon `n` sent, when it's a file (a path or a `file://` url) and not
/// an icon theme name.
fn icon_file(n: &Notification) -> Option<&str> {
    let icon = n.icon();
    (icon.starts_with('/') || icon.starts_with("file://")).then_some(icon)
}

/// A path or a `file://` url as a path, its `%20`s and the like decoded.
fn file_path(sent: &str) -> String {
    let Some(url) = sent.strip_prefix("file://") else {
        return sent.to_string();
    };
    let bytes = url.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The sending app's icon, from its desktop entry, its name, or the icon
/// it sent; a first letter when none loads. An icon sent as a file is the
/// thumbnail instead ([`image_path`]), so it isn't drawn twice.
fn app_icon(n: &Notification, side: f32) -> Rectangle {
    let sent = || {
        icon_file(n)
            .is_none()
            .then(|| n.icon_path().map(PathBuf::from))
            .flatten()
    };
    let found: Option<PathBuf> = icon_for(n.desktop_entry())
        .or_else(|| icon_for(n.app_name()))
        .or_else(sent);

    if let Some(path) = found
        && Image::loaded(&path)
    {
        return Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .fill(Image::contain(path));
    }
    app_icon_tile(app_name(n), n.summary(), side)
}

/// What to call the sender: its app name, else its desktop entry.
pub fn app_name(n: &Notification) -> &str {
    if n.app_name().is_empty() {
        n.desktop_entry()
    } else {
        n.app_name()
    }
}

// ───── text ──────────────────────────────────────────────────────────────────

/// The text without the simple markup some senders still send (`<b>`,
/// `<a href>`), and with its entities decoded.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            // a tag starts with a name or a slash; "1 < 2" is just text
            '<' if !in_tag
                && chars
                    .peek()
                    .is_some_and(|next| next.is_ascii_alphabetic() || *next == '/') =>
            {
                in_tag = true;
            }
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// When it arrived, macOS-style: "now", "5m ago", "2h ago", then the day.
pub fn ago(received: SystemTime, now: SystemTime) -> String {
    let elapsed = now.duration_since(received).unwrap_or(Duration::ZERO);
    let minutes = elapsed.as_secs() / 60;
    match minutes {
        0 => String::from("now"),
        1..60 => format!("{minutes}m ago"),
        60..1440 => format!("{}h ago", minutes / 60),
        _ => local(received, "%a %H:%M"),
    }
}

/// `time` on the local clock, in chrono's `format`.
pub fn local(time: SystemTime, format: &str) -> String {
    DateTime::<Local>::from(time).format(format).to_string()
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
    fn times_read_like_macos() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100_000);
        let before = |secs| now - Duration::from_secs(secs);

        assert_eq!(ago(before(0), now), "now");
        assert_eq!(ago(before(59), now), "now");
        assert_eq!(ago(before(60), now), "1m ago");
        assert_eq!(ago(before(59 * 60), now), "59m ago");
        assert_eq!(ago(before(3 * 3600), now), "3h ago");
        // a clock that went backwards still reads as now
        assert_eq!(ago(now + Duration::from_secs(5), now), "now");
    }
}
