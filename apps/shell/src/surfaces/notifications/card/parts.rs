//! What a notification card is made of: its icon or image, the app's name, its text and its age.

use super::*;

/// The app's icon, centered in its frame as tall as the card's content.
pub(super) fn leading(n: &Notification, height: f32) -> Box<dyn Widget> {
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
pub(super) fn image_path(n: &Notification) -> Option<String> {
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
pub(super) fn icon_file(n: &Notification) -> Option<&str> {
    let icon = n.icon();
    (icon.starts_with('/') || icon.starts_with("file://")).then_some(icon)
}

/// A path or a `file://` url as a path, its `%20`s and the like decoded.
pub(super) fn file_path(sent: &str) -> String {
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
pub(super) fn app_icon(n: &Notification, side: f32) -> Rectangle {
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

pub(super) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// When it arrived, briefly: "now", "5m ago", "2h ago", then the day.
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
