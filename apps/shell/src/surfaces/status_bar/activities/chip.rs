//! An activity in the bar: its chip, its picture, and the text or ring at its end.

use super::*;

pub(super) fn glyph_button(id: &str, glyph: &str) -> ActivityButton {
    ActivityButton {
        icon: Some(ItemIcon::Glyph(String::from(glyph))),
        ..ActivityButton::new(id, "")
    }
}

/// The activities going on: the most relevant compact, the rest minimal.
pub fn widgets() -> Vec<Box<dyn Widget>> {
    Tick::watch();
    let shown = gather();
    alert(&shown);
    let compact = status_bar().compact_activities();
    shown
        .into_iter()
        .enumerate()
        .map(|(index, activity)| boxed(chip(activity, index < compact)))
        .collect()
}

/// An activity in the bar: its picture, and its text when compact.
pub(super) fn chip(activity: Shown, compact: bool) -> Rectangle {
    let content = &activity.content;
    let tint = content.tint.map(tint_color);
    let trailing = if compact {
        trailing_text(&content.trailing, activity.key == "now-playing")
    } else {
        None
    };
    let ring = match content.trailing {
        Trailing::Progress(done) if compact => Some(done),
        _ => None,
    };

    let inset = sys::space::inset::s();
    let leading_size = leading();
    let width = match (&trailing, ring) {
        (Some((text, style)), _) => inset * 2.0 + leading_size + 6.0 + text_width(text, *style),
        (None, Some(_)) => inset * 2.0 + leading_size + 6.0 + leading_size,
        (None, None) => super::super::metrics().item_width,
    };
    let tip = match content.subtitle.is_empty() {
        true => content.title.clone(),
        false => format!("{} · {}", content.title, content.subtitle),
    };
    let leading = content.leading.clone();
    let dim = activity.dim;
    let key = activity.key.clone();
    let showing = OPEN.with_borrow(|open| open.as_ref().is_some_and(|open| open.key == key));

    let mut button = item(DrawnButton::new(
        format!("amane.bar.activity.{}", activity.key),
        move |color| {
            let label = if dim {
                material::label(Level::Secondary)
            } else {
                color
            };
            let mut parts = vec![picture(&leading, leading_size, tint.unwrap_or(label), dim)];
            if let Some((text, style)) = trailing {
                parts.push(boxed(Text::new(text).style(style).color(label)));
            }
            if let Some(done) = ring {
                parts.push(boxed(progress_ring(
                    done,
                    leading_size,
                    tint.unwrap_or(label),
                )));
            }
            Rectangle::new()
                .width(Size::Fixed(width))
                .height(Parent)
                .fill(Color::TRANSPARENT)
                .align_child(Center, Center)
                .child(Row::new(parts).gap(6.0).align(Center))
        },
    ))
    .width(width)
    .color(material::label(Level::Primary))
    .active(showing)
    .on_press(move || toggle(&key))
    .tooltip(BAR_NS, tip);
    if let Some(middle) = activity.middle {
        button = button.on_middle(move || middle());
    }
    button.build()
}

/// A compact activity's text and its style: a clipped label, or a
/// countdown or count-up that the bar ticks itself.
pub(super) fn trailing_text(
    trailing: &Trailing,
    long: bool,
) -> Option<(String, crate::tokens::TextStyle)> {
    match trailing {
        Trailing::None | Trailing::Progress(_) => None,
        Trailing::Text(text) if text.is_empty() => None,
        Trailing::Text(text) => {
            let style = super::super::metrics().label();
            Some((clip(text, style, if long { TITLE } else { TEXT }), style))
        }
        Trailing::Countdown(until) => {
            let left = Duration::from_millis(
                until.saturating_sub(amane_kit::time::unix_ms()).max(0) as u64,
            );
            tick_in(Duration::from_nanos(u64::from(left.subsec_nanos())));
            Some((timer::format(left), super::super::metrics().numeric()))
        }
        Trailing::CountUp(since) => {
            let gone = Duration::from_millis(
                amane_kit::time::unix_ms().saturating_sub(*since).max(0) as u64,
            );
            tick_in(Duration::from_secs(1) - Duration::from_nanos(u64::from(gone.subsec_nanos())));
            // whole seconds, as a recording's length shows
            Some((
                timer::format(Duration::from_secs(gone.as_secs())),
                super::super::metrics().numeric(),
            ))
        }
    }
}

/// Asks for a frame `wait` from now, just past a second turning over,
/// unless one is already asked for sooner.
pub(super) fn tick_in(wait: Duration) {
    let now = Instant::now();
    let due = now + wait + TICK;
    let asked = TICK_DUE.get().filter(|asked| *asked > now);
    if asked.is_some_and(|asked| asked <= due) {
        return;
    }
    TICK_DUE.set(Some(due));
    motion::redraw_after::<Tick>(wait + TICK);
}

/// A picture: one of the bar's symbols, a theme icon or a file, at `size`;
/// a dot while a file is still being read or can't be.
pub(in crate::surfaces::status_bar) fn picture(
    icon_of: &ItemIcon,
    size: f32,
    color: Color,
    dim: bool,
) -> Box<dyn Widget> {
    let file = match icon_of {
        ItemIcon::Glyph(name) => {
            let glyph = match name.as_str() {
                "music" => ICON_MUSIC,
                "timer" => ICON_TIMER,
                "screen-share" => ICON_SCREEN_SHARE,
                name => named(name).unwrap_or(ICON_RECORD),
            };
            return boxed(icon(glyph, size * 0.9, color));
        }
        ItemIcon::Theme(name) => Apps::find_icon(name),
        ItemIcon::File(path) => Some(path.clone()),
    };
    let Some(path) = file.filter(|path| Image::loaded(path)) else {
        return boxed(icon(ICON_RECORD, size * 0.5, color));
    };
    boxed(image(path, size, dim))
}

pub(super) fn image(path: PathBuf, size: f32, dim: bool) -> Rectangle {
    let picture = Rectangle::new()
        .width(Size::Fixed(size))
        .height(Size::Fixed(size))
        .radius((size / 4.5).round())
        .fill(Image::cover(path));
    if dim { picture.opacity(0.6) } else { picture }
}

/// A ring `diameter` across, filled `done` of the way round from the top.
pub(super) fn progress_ring(done: f32, diameter: f32, color: Color) -> Canvas {
    let thickness = 2.5;
    let track = material::fill_role(FillLevel::Secondary);
    let mut shapes: Vec<Box<dyn Shape>> = vec![Box::new(Arc::new().stroke(thickness, track))];
    let sweep = 360.0 * done.clamp(0.0, 1.0);
    if sweep > 0.0 {
        shapes.push(Box::new(
            Arc::new()
                .sweep(sweep.max(1.0))
                .stroke(thickness, color)
                .cap(Cap::Round),
        ));
    }
    Canvas::new()
        .width(Size::Fixed(diameter))
        .height(Size::Fixed(diameter))
        .shapes(shapes)
}

/// A palette colour, as the desktop's accents are.
pub(super) fn tint_color(tint: Tint) -> Color {
    match tint {
        Tint::Accent => sys::color::accent::bg(),
        Tint::Red => Accent::Red.color(),
        Tint::Orange => Accent::Orange.color(),
        Tint::Yellow => Accent::Yellow.color(),
        Tint::Green => Accent::Green.color(),
        Tint::Mint => Accent::Mint.color(),
        Tint::Teal => Accent::Teal.color(),
        Tint::Cyan => Accent::Cyan.color(),
        Tint::Blue => Accent::Blue.color(),
        Tint::Indigo => Accent::Indigo.color(),
        Tint::Purple => Accent::Purple.color(),
        Tint::Pink => Accent::Pink.color(),
        Tint::Gray => Accent::Gray.color(),
    }
}

/// `text` cut with an ellipsis to fit `width`.
pub(in crate::surfaces::status_bar) fn clip(
    text: &str,
    style: crate::tokens::TextStyle,
    width: f32,
) -> String {
    if text_width(text, style) <= width {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let cut = format!("{}…", chars.iter().collect::<String>().trim_end());
        if text_width(&cut, style) <= width {
            return cut;
        }
    }
    String::from("…")
}
