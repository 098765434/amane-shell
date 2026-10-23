//! One notification as a card, shared by the toasts and the center.
//!
//! Laid out like an iOS/macOS banner: the app icon (or the sender's image,
//! with the app icon as a badge) on the left; the app name and time on one
//! line, the summary, and up to [`BODY_LINES`] of body to its right; the
//! sender's actions as a row of equal buttons underneath. A hovered card
//! swaps the time for a close button.
//!
//! The card's height is known before it is drawn ([`height`]), so the
//! stacks around it can place and animate cards exactly.

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use amane::{
    Button, Center, Color, Column, Image, Notification, Notifications, Parent, Pointer, Radius,
    Rectangle, Row, Size, SpaceBetween, Stack, Start, Text, Urgency, Widget,
};
use chrono::{DateTime, Local};

use crate::components::taskbar::{app_icon_tile, boxed, icon, icon_for, pad};
use crate::tokens::{Styled, notification, sys};

/// The body is cut after this many lines, like a banner's.
const BODY_LINES: usize = 4;

const ICON_CLOSE: char = '\u{e5cd}';

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
            Place::Center => notification::center::width() - sys::space::inset::l() * 2.0,
        }
    }

    fn look(self) -> Look {
        match self {
            Place::Toast => Look {
                fill: sys::color::surface::notification::bg(),
                fg: sys::color::surface::notification::fg(),
                secondary: sys::color::surface::notification::fg_secondary(),
                stroke: sys::color::surface::notification::stroke(),
            },
            Place::Center => Look {
                fill: sys::color::surface::raised::bg(),
                fg: sys::color::surface::popover::fg(),
                secondary: sys::color::surface::popover::fg_secondary(),
                stroke: Color::TRANSPARENT,
            },
        }
    }
}

thread_local! {
    // the card under the pointer, which shows its close button
    static HOVERED: Cell<Option<u32>> = const { Cell::new(None) };
}

/// Callers pass their card's `on_hover` here.
pub fn hover(id: u32, inside: bool) {
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
struct Layout {
    summary: String,
    body: String,
    text_width: f32,
    line: f32,
    title_line: f32,
    body_height: f32,
    height: f32,
}

fn layout(n: &Notification, width: f32) -> Layout {
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

/// How tall `n`'s card is in `place`.
pub fn height(n: &Notification, place: Place) -> f32 {
    layout(n, place.width()).height
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

/// `n`'s card, its content at `content` opacity (0 for a blank plate
/// stacked behind another). Callers add the click and hover handlers.
pub fn card(n: &Notification, place: Place, now: SystemTime, content: f32) -> Rectangle {
    let width = place.width();
    let look = place.look();
    let parts = layout(n, width);
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
        boxed(close_button(id, parts.line, look.secondary))
    } else {
        boxed(
            Text::new(ago(n.received(), now))
                .style(sys::r#type::caption())
                .color(look.secondary),
        )
    };
    let header = Row::new(vec![
        boxed(
            fixed(parts.text_width * 0.7, parts.line)
                .align_child(Start, Center)
                .child(
                    Text::new(app_name(n))
                        .style(sys::r#type::caption())
                        .color(look.secondary)
                        .elide(),
                ),
        ),
        trailing,
    ])
    .width(Size::Fixed(parts.text_width))
    .justify(SpaceBetween)
    .align(Center);

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
        rows.push(boxed(actions(
            n,
            width - sys::space::inset::l() * 2.0,
            look.fg,
        )));
    }

    let mut body = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .child(Column::new(rows).gap(sys::space::gap::m()));
    if content < 1.0 {
        body = body.opacity(content.clamp(0.0, 1.0));
    }

    let stroke = if n.urgency() == Urgency::Critical {
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
                Rectangle::new()
                    .width(Size::Fixed(each))
                    .height(Size::Fixed(sys::size::control::height()))
                    .radius(sys::radius::raised())
                    .fill(sys::color::surface::raised::bg())
                    .align_child(Center, Center)
                    .clip()
                    .cursor(Pointer)
                    .on_click(move |button| {
                        if button == Button::Left {
                            Notifications::invoke(id, &key);
                        }
                    })
                    // not elided, which would stretch it and lose the centering;
                    // the button clips a label too long for it
                    .child(
                        Text::new(action.label())
                            .style(sys::r#type::label())
                            .color(color),
                    ),
            )
        })
        .collect();

    Row::new(buttons).gap(gap)
}

fn close_button(id: u32, side: f32, color: Color) -> Rectangle {
    let side = side + sys::space::gap::xs();
    Rectangle::new()
        .width(Size::Fixed(side))
        .height(Size::Fixed(side))
        .radius(Radius::Full)
        .fill(sys::color::state::hover())
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |button| {
            if button == Button::Left {
                Notifications::dismiss(id);
            }
        })
        .child(icon(ICON_CLOSE, sys::size::icon::s(), color))
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
        _ => DateTime::<Local>::from(received)
            .format("%a %H:%M")
            .to_string(),
    }
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
