//! One notification as a card, shared by the toasts and the center.
//!
//! Laid out like an iOS/macOS banner: the app icon (or the sender's image,
//! with the app icon as a badge) on the left; the app name and time on one
//! line, the summary, and up to [`BODY_LINES`] of body to its right; the
//! sender's actions as a row of equal buttons underneath. A hovered card
//! swaps the time for a close button.
//!
//! The card is measured before it is drawn ([`measure`]), so the stacks
//! around it can place and animate cards exactly, and drawing reuses the
//! measurement.

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use amane::{
    Button, Center, Color, Column, Image, Notification, Notifications, Pointer, Radius, Rectangle,
    Row, Size, Stack, Start, Text, Urgency, Widget,
};
use chrono::{DateTime, Local};

use crate::components::buttons::{IconButton, PushButton, Style};
use crate::components::card::Surface;
use crate::components::icons::ICON_CLOSE;
use crate::components::layout::{blank, boxed, inner_width, pad, split_row};
use crate::components::taskbar::{app_icon_tile, icon_for};
use crate::tokens::{Styled, notification, sys};

/// The body is cut after this many lines, like a banner's.
const BODY_LINES: usize = 4;

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
            Place::Toast => Look {
                fill: Surface::Notification.bg(),
                fg: Surface::Notification.fg(),
                secondary: Surface::Notification.fg_secondary(),
                stroke: Surface::Notification.stroke().unwrap_or(Color::TRANSPARENT),
            },
            // a raised card on the center's popover sheet, in its colours
            Place::Center => Look {
                fill: sys::color::surface::raised::bg(),
                fg: Surface::Popover.fg(),
                secondary: Surface::Popover.fg_secondary(),
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

// ───── layout ────────────────────────────────────────────────────────────────

/// The text a card shows, cleaned, and how tall each part is.
#[derive(Clone)]
pub struct Layout {
    summary: String,
    body: String,
    text_width: f32,
    line: f32,
    title_line: f32,
    body_height: f32,
    /// The whole card's height.
    pub height: f32,
}

/// `n`'s card in `place`, measured: how tall it is, and what it shows.
pub fn measure(n: &Notification, place: Place) -> Layout {
    let width = place.width();
    let inset = sys::space::inset::l();
    let text_width =
        (width - inset * 2.0 - notification::card::icon_size() - sys::space::gap::m()).max(1.0);

    let summary = one_line(&plain(n.summary()));
    let body = plain(n.body());

    let line = Text::new("Ag")
        .style(sys::r#type::caption())
        .height_in(f32::MAX);
    let title_line = Text::new("Ag")
        .style(sys::r#type::title())
        .height_in(f32::MAX);
    let body_height = if body.is_empty() {
        0.0
    } else {
        body_text(&body, Color::TRANSPARENT).height_in(text_width)
    };

    let mut text = line;
    if !summary.is_empty() {
        text += sys::space::gap::_2xs() + title_line;
    }
    if body_height > 0.0 {
        text += sys::space::gap::_2xs() + body_height;
    }

    let mut height = inset * 2.0 + text.max(notification::card::icon_size());
    if !n.actions().is_empty() {
        height += sys::space::gap::m() + sys::size::control::height();
    }

    Layout {
        summary,
        body,
        text_width,
        line,
        title_line,
        body_height,
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
        .style(sys::r#type::label())
        .color(color)
        .wrap()
        .max_lines(BODY_LINES)
        .elide()
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

    let fixed = |w: f32, h: f32| {
        Rectangle::new()
            .width(Size::Fixed(w))
            .height(Size::Fixed(h))
            .fill(Color::TRANSPARENT)
            .clip()
    };

    // app name on the left, the time (or close, while hovered) on the right
    let trailing: Box<dyn Widget> = if hovered {
        let side = parts.line + sys::space::gap::xs();
        boxed(close_button(
            format!("amane.notification.{id}.close"),
            side,
            look.secondary,
            move || Notifications::dismiss(id),
        ))
    } else {
        boxed(
            Text::new(ago(n.received(), now))
                .style(sys::r#type::caption())
                .color(look.secondary),
        )
    };
    // the app's name takes most of the line, leaving the rest to the time
    let app = fixed(parts.text_width * 0.7, parts.line)
        .align_child(Start, Center)
        .child(
            Text::new(app_name(n))
                .style(sys::r#type::caption())
                .color(look.secondary)
                .elide(),
        );
    let header = split_row(app, vec![trailing]).width(Size::Fixed(parts.text_width));

    let mut text: Vec<Box<dyn Widget>> = vec![boxed(header)];
    if !parts.summary.is_empty() {
        text.push(boxed(
            fixed(parts.text_width, parts.title_line)
                .align_child(Start, Center)
                .child(
                    Text::new(parts.summary.clone())
                        .style(sys::r#type::title())
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

    let top = Row::new(vec![
        leading(n, notification::card::icon_size()),
        boxed(Column::new(text).gap(sys::space::gap::_2xs())),
    ])
    .gap(sys::space::gap::m());

    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(top)];
    if !n.actions().is_empty() {
        rows.push(boxed(actions(n, inner_width(width), look.fg)));
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

    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(parts.height))
        .radius(notification::card::radius())
        .fill(look.fill)
        .border(sys::border::hairline(), stroke)
        .padding(pad(sys::space::inset::l(), sys::space::inset::l()))
        .clip()
        .child(body)
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

/// The app's icon, or the sender's own image with the app icon as a badge.
fn leading(n: &Notification, side: f32) -> Box<dyn Widget> {
    let image = n.image().strip_prefix("file://").unwrap_or(n.image());
    if !image.is_empty() && Image::loaded(image) {
        let badge = notification::card::badge_size();
        let avatar = Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(Radius::Full)
            .fill(Image::cover(image));
        let badge = app_icon(n, badge).translate(side - badge * 0.8, side - badge * 0.8);
        return boxed(Stack::new(vec![boxed(avatar), boxed(badge)]));
    }
    boxed(app_icon(n, side))
}

/// The sending app's icon, from its desktop entry, its name, or the icon
/// it sent; a first letter when none loads.
fn app_icon(n: &Notification, side: f32) -> Rectangle {
    let found: Option<PathBuf> = icon_for(n.desktop_entry())
        .or_else(|| icon_for(n.app_name()))
        .or_else(|| n.icon_path().map(PathBuf::from));

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
