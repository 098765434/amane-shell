//! Activities in the bar: what's going on now. The bar's own come
//! straight from their services: Now Playing,
//! the launcher's timer and screen sharing. Other programs' come over the
//! bar's bus service ([`activities`](crate::services::desktop::activities)).
//!
//! The most relevant show **compact**: the leading picture and the
//! trailing text, countdown or ring (`StatusBar::activities_compact` of
//! them). The rest show **minimal**, the picture alone. A click opens the
//! **expanded** view under the bar, with the title, a progress bar and up to
//! three buttons. An update that alerts brings that view up for a few
//! seconds, but not under Do Not Disturb.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use amane::{
    Apps, Arc, Canvas, Cap, Center, Color, ConstraintAdjustment, Gravity, Image, Media, Parent,
    Popup, Rectangle, Row, Service, Shape, Size, Start, Text, Widget, window_size,
};
use amane_kit::color::Accent;
use amane_kit::icons::{
    ICON_MUSIC, ICON_PAUSE, ICON_PLAY, ICON_RECORD, ICON_SCREEN_SHARE, ICON_SKIP_NEXT,
    ICON_SKIP_PREVIOUS, ICON_TIMER, named,
};
use amane_kit::settings::{BUILTIN_ACTIVITIES, status_bar};
use amane_status::{ActivityButton, ActivityContent, ItemIcon, Tint, Trailing};

use crate::services::desktop::activities::{self as registry, Live, now_ms};
use crate::services::desktop::privacy::{CastTarget, Privacy, stop_cast};
use crate::services::do_not_disturb::DoNotDisturb;
use crate::services::launcher::timer;
use crate::surfaces::launcher;
use crate::surfaces::quick_settings::{self, Page};
use crate::tokens::{Styled, sys};
use amane_kit::buttons::{DrawnButton, IconButton, PushButton, Style};
use amane_kit::card::{Card, Variant};
use amane_kit::layout::{boxed, cursor, icon, text_width};
use amane_kit::material::{self, FillLevel, Level, Material};
use amane_kit::motion;

use super::{BAR_NS, item, menu_edge};

/// The expanded view's popup namespace; it hangs under the bar, whose
/// layer rule frosts its popups.
pub const POPUP_NS: &str = "amane.status-bar.activity";

/// How long a paused player keeps its activity.
const PAUSED_FOR: Duration = Duration::from_secs(5 * 60);

/// How wide a compact activity's text may be, and Now Playing's title.
const TEXT: f32 = 120.0;
const TITLE: f32 = 160.0;

/// The leading picture's size expanded; compact, it's [`leading`].
const LEADING_BIG: f32 = 36.0;

/// The leading picture's size in the bar: a little over a glyph.
fn leading() -> f32 {
    super::metrics().icon + 2.0
}

/// The expanded view's width, and its rows' heights.
const CARD_WIDTH: f32 = 320.0;
const HEADER: f32 = 40.0;
const BAR: f32 = 4.0;
const BUTTONS: f32 = 30.0;

/// A beat past each second turning over, so the frame finds it turned.
const TICK: Duration = Duration::from_millis(20);

/// The apps the bar's own activities go under, for the permission list.
pub const NOW_PLAYING: &str = BUILTIN_ACTIVITIES[0];
pub const TIMER: &str = BUILTIN_ACTIVITIES[1];
pub const SHARING: &str = BUILTIN_ACTIVITIES[2];

amane_kit::marker! {
    /// A countdown's second turned over, or the expanded view opened or
    /// closed: the bar and the view draw again.
    pub Tick
}

/// One activity as the bar shows it.
#[derive(Clone)]
struct Shown {
    /// What tells it apart: `now-playing`, `timer`, `sharing` or `bus.<handle>`.
    key: String,
    content: ActivityContent,
    /// Out of date, or ended and showing its last state: dimmed.
    dim: bool,
    /// A button by its id.
    act: Rc<dyn Fn(&str)>,
    /// What a middle click does, as Now Playing plays or pauses.
    middle: Option<Rc<dyn Fn()>>,
}

/// The expanded view showing.
#[derive(Clone)]
struct Open {
    key: String,
    /// Where it hangs from on the bar.
    x: i32,
    /// Opened by a click, so it takes a grab; an alert's doesn't.
    clicked: bool,
}

thread_local! {
    // when something last played
    static LAST_PLAYED: Cell<Option<Instant>> = const { Cell::new(None) };

    // the expanded view showing
    static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };

    // the instant the next countdown frame was asked for
    static TICK_DUE: Cell<Option<Instant>> = const { Cell::new(None) };

    // the alert that already opened the view, so closing it keeps it closed
    static ALERTED: Cell<Option<u32>> = const { Cell::new(None) };
}

// ───── what's going on ───────────────────────────────────────────────────────

/// Every activity the bar may show now, most relevant first.
fn gather() -> Vec<Shown> {
    let bar = status_bar();
    let mut shown: Vec<Shown> = [now_playing(), timer(), sharing()]
        .into_iter()
        .flatten()
        .chain(registry::list().into_iter().map(from_bus))
        .filter(|activity| bar.allows_activity(&activity.content.app))
        .collect();
    drop(bar);
    // stable, so ties keep the order they started in
    shown.sort_by(|a, b| b.content.relevance.total_cmp(&a.content.relevance));
    shown
}

fn from_bus(live: Live) -> Shown {
    let now = Instant::now();
    let handle = live.handle;
    Shown {
        key: format!("bus.{handle}"),
        dim: live.leaving.is_some() || live.stale(now),
        content: live.content,
        act: Rc::new(move |button| registry::act(handle, button)),
        middle: None,
    }
}

/// What plays, or played in the last few minutes; with `media_always`,
/// "Not Playing" when nothing does.
fn now_playing() -> Option<Shown> {
    let media = Media::read();
    let playing = media.playing();
    let title = media.title().to_string();
    let artist = media.artist().to_string();
    let player = media
        .active()
        .map(|player| player.identity().to_string())
        .unwrap_or_default();
    let art = crate::services::media_art::path(media.art_url());
    drop(media);

    let now = Instant::now();
    if playing {
        LAST_PLAYED.set(Some(now));
    }
    let lately = LAST_PLAYED
        .get()
        .is_some_and(|at| now.duration_since(at) < PAUSED_FOR);
    let active = !title.is_empty() && (playing || lately);
    if !active && !status_bar().media_always {
        return None;
    }
    if active && !playing {
        // gone once the pause has lasted long enough
        let left = LAST_PLAYED.get().map_or(PAUSED_FOR, |at| {
            PAUSED_FOR.saturating_sub(now.duration_since(at))
        });
        // once per wait, not once per frame drawn while paused
        tick_in(left);
    }

    let mut content = ActivityContent::new(
        NOW_PLAYING,
        art.map_or_else(
            || ItemIcon::Glyph(String::from("music")),
            |path| ItemIcon::File(PathBuf::from(path)),
        ),
        if active { &title } else { "Not Playing" },
    );
    content.subtitle = if active { artist } else { player.clone() };
    content.trailing = Trailing::Text(if active {
        title
    } else {
        String::from("Not Playing")
    });
    content.relevance = 0.3;
    if active {
        let play = if playing { "pause" } else { "play" };
        content.buttons = vec![
            glyph_button("previous", "skip-previous"),
            glyph_button("play-pause", play),
            glyph_button("next", "skip-next"),
        ];
    }
    let more = if player.is_empty() {
        String::from("Quick Settings")
    } else {
        format!("{player} in Quick Settings")
    };
    content.buttons.push(ActivityButton::new("more", &more));

    Some(Shown {
        key: String::from("now-playing"),
        content,
        dim: !playing,
        act: Rc::new(|button| match button {
            "previous" => Media::previous(),
            "play-pause" => Media::play_pause(),
            "next" => Media::next(),
            _ => {
                close();
                quick_settings::open(Page::Grid);
            }
        }),
        middle: Some(Rc::new(Media::play_pause)),
    })
}

/// The launcher's timer while it runs, counting down to its end.
fn timer() -> Option<Shown> {
    let (ends, label) = timer::ends()?;
    let ends_ms = ends
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64);

    let mut content = ActivityContent::new(TIMER, ItemIcon::Glyph(String::from("timer")), &label);
    content.subtitle = String::from("Timer");
    content.trailing = Trailing::Countdown(ends_ms);
    content.tint = Some(Tint::Orange);
    content.relevance = 0.7;
    content.buttons = vec![
        ActivityButton {
            destructive: true,
            ..ActivityButton::new("cancel", "Cancel")
        },
        ActivityButton::new("open", "Open in the Launcher"),
    ];

    Some(Shown {
        key: String::from("timer"),
        content,
        dim: false,
        act: Rc::new(|button| {
            close();
            match button {
                "cancel" => timer::cancel(),
                _ => {
                    launcher::open(launcher::Mode::Actions);
                }
            }
        }),
        middle: None,
    })
}

/// While niri casts the screen or a window: what's shared, each with Stop
/// Sharing.
fn sharing() -> Option<Shown> {
    let privacy = Privacy::read();
    if !privacy.screen() {
        return None;
    }
    let casts = privacy.casts().to_vec();
    drop(privacy);

    let windows = window_titles();
    let what = |target: &CastTarget| match target {
        CastTarget::Output(name) => format!("Screen {name}"),
        CastTarget::Window(id) => windows
            .iter()
            .find(|(window, _)| window == id)
            .map_or_else(|| String::from("A window"), |(_, title)| title.clone()),
        CastTarget::Nothing => String::from("Nothing yet"),
    };

    let mut content = ActivityContent::new(
        SHARING,
        ItemIcon::Glyph(String::from("screen-share")),
        "Sharing",
    );
    content.subtitle = casts
        .iter()
        .map(|cast| what(&cast.target))
        .collect::<Vec<_>>()
        .join(", ");
    content.trailing = Trailing::Text(String::from("Sharing"));
    content.tint = Some(Tint::Purple);
    content.relevance = 0.9;
    content.buttons = casts
        .iter()
        .take(3)
        .map(|cast| ActivityButton {
            destructive: true,
            ..ActivityButton::new(
                &format!("stop:{}", cast.session),
                if casts.len() == 1 {
                    "Stop Sharing".to_string()
                } else {
                    format!("Stop {}", what(&cast.target))
                }
                .as_str(),
            )
        })
        .collect();

    Some(Shown {
        key: String::from("sharing"),
        content,
        dim: false,
        act: Rc::new(|button| {
            if let Some(session) = button.strip_prefix("stop:").and_then(|s| s.parse().ok()) {
                stop_cast(session);
                close();
            }
        }),
        middle: None,
    })
}

/// Every window by id with its title, from niri.
fn window_titles() -> Vec<(u64, String)> {
    amane::Windows::read()
        .list()
        .iter()
        .map(|window| (window.id(), window.title().unwrap_or_default().to_string()))
        .collect()
}

fn glyph_button(id: &str, glyph: &str) -> ActivityButton {
    ActivityButton {
        icon: Some(ItemIcon::Glyph(String::from(glyph))),
        ..ActivityButton::new(id, "")
    }
}

// ───── in the bar ────────────────────────────────────────────────────────────

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
fn chip(activity: Shown, compact: bool) -> Rectangle {
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
        (None, None) => super::metrics().item_width,
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
fn trailing_text(trailing: &Trailing, long: bool) -> Option<(String, crate::tokens::TextStyle)> {
    match trailing {
        Trailing::None | Trailing::Progress(_) => None,
        Trailing::Text(text) if text.is_empty() => None,
        Trailing::Text(text) => {
            let style = super::metrics().label();
            Some((clip(text, style, if long { TITLE } else { TEXT }), style))
        }
        Trailing::Countdown(until) => {
            let left = Duration::from_millis(until.saturating_sub(now_ms()).max(0) as u64);
            tick_in(Duration::from_nanos(u64::from(left.subsec_nanos())));
            Some((timer::format(left), super::metrics().numeric()))
        }
        Trailing::CountUp(since) => {
            let gone = Duration::from_millis(now_ms().saturating_sub(*since).max(0) as u64);
            tick_in(Duration::from_secs(1) - Duration::from_nanos(u64::from(gone.subsec_nanos())));
            // whole seconds, as a recording's length shows
            Some((
                timer::format(Duration::from_secs(gone.as_secs())),
                super::metrics().numeric(),
            ))
        }
    }
}

/// Asks for a frame `wait` from now, just past a second turning over,
/// unless one is already asked for sooner.
fn tick_in(wait: Duration) {
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
pub(super) fn picture(icon_of: &ItemIcon, size: f32, color: Color, dim: bool) -> Box<dyn Widget> {
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

fn image(path: PathBuf, size: f32, dim: bool) -> Rectangle {
    let picture = Rectangle::new()
        .width(Size::Fixed(size))
        .height(Size::Fixed(size))
        .radius((size / 4.5).round())
        .fill(Image::cover(path));
    if dim { picture.opacity(0.6) } else { picture }
}

/// A ring `diameter` across, filled `done` of the way round from the top.
fn progress_ring(done: f32, diameter: f32, color: Color) -> Canvas {
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
fn tint_color(tint: Tint) -> Color {
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
pub(super) fn clip(text: &str, style: crate::tokens::TextStyle, width: f32) -> String {
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

// ───── the expanded view ─────────────────────────────────────────────────────

/// Opens the expanded view of `key` under the pointer, or closes it.
fn toggle(key: &str) {
    let showing = OPEN.with_borrow(|open| open.as_ref().is_some_and(|open| open.key == key));
    if showing {
        close();
        return;
    }
    OPEN.set(Some(Open {
        key: key.to_string(),
        x: cursor().0,
        clicked: true,
    }));
    Tick::changed();
}

/// Closes the expanded view.
fn close() {
    if OPEN.take().is_some() {
        registry::quiet_alert();
        Tick::changed();
    }
}

/// An alerting update brings its view up, unless another is open or Do
/// Not Disturb is on; it closes again once the alert is over.
fn alert(shown: &[Shown]) {
    let alerting = registry::alerting();
    let key = alerting.map(|handle| format!("bus.{handle}"));
    let open_alert = OPEN.with_borrow(|open| {
        open.as_ref()
            .filter(|open| !open.clicked)
            .map(|open| open.key.clone())
    });
    match (key, open_alert) {
        // the alert is over, or another took its place
        (key, Some(open)) if key.as_ref() != Some(&open) => {
            OPEN.set(None);
            Tick::changed();
        }
        (Some(key), None) => {
            let handle = alerting.unwrap_or_default();
            let fresh = ALERTED.replace(Some(handle)) != Some(handle);
            let nothing_open = OPEN.with_borrow(Option::is_none);
            let there = shown.iter().any(|activity| activity.key == key);
            if fresh && nothing_open && there && !DoNotDisturb::read().on() {
                OPEN.set(Some(Open {
                    key,
                    // the activities sit towards the bar's right
                    x: (window_size().0 - CARD_WIDTH) as i32,
                    clicked: false,
                }));
                Tick::changed();
            }
        }
        _ => {}
    }
    if alerting.is_none() {
        ALERTED.set(None);
    }
}

/// The expanded view's popup, registered once with `App::popup`.
pub fn popup() -> Popup {
    Tick::watch();
    let open = OPEN.with_borrow(Clone::clone);
    let found = open.as_ref().and_then(|open| {
        gather()
            .into_iter()
            .find(|activity| activity.key == open.key)
    });
    let (Some(open), Some(activity)) = (open, found) else {
        if OPEN.with_borrow(Option::is_some) {
            // what it showed ended
            OPEN.set(None);
        }
        return Popup::new(POPUP_NS)
            .size(1, 1)
            .anchor_rect(0, 0, 1, 1)
            .parent(BAR_NS)
            .visible(false)
            .child(amane_kit::layout::blank());
    };

    let (card, height) = expanded(&activity);
    let mut popup = Popup::new(POPUP_NS)
        .size(CARD_WIDTH as u32, height as u32)
        .anchor_rect(open.x, menu_edge(), 1, 1)
        .vibrant(material::vibrant())
        .parent(BAR_NS)
        .gravity(Gravity::BottomRight)
        .constraint_adjustment(
            ConstraintAdjustment::SLIDE_X
                | ConstraintAdjustment::SLIDE_Y
                | ConstraintAdjustment::FLIP_X
                | ConstraintAdjustment::FLIP_Y,
        )
        .visible(true)
        .on_dismiss(close)
        .child(card);
    if open.clicked {
        popup = popup.grab();
    }
    popup
}

/// The expanded card and its height: the picture, title and subtitle and
/// the trailing part; the progress bar; the buttons.
fn expanded(activity: &Shown) -> (Rectangle, f32) {
    let content = &activity.content;
    let padding = sys::space::inset::m();
    let gap = sys::space::gap::s();
    let inner = CARD_WIDTH - padding * 2.0;
    let tint = content.tint.map(tint_color);
    let primary = material::label(Level::Primary);
    let secondary = material::label(Level::Secondary);

    let mut text = vec![boxed(
        Text::new(clip(&content.title, sys::r#type::label(), inner - 120.0))
            .style(sys::r#type::label())
            .color(primary),
    )];
    if !content.subtitle.is_empty() {
        text.push(boxed(
            Text::new(clip(
                &content.subtitle,
                sys::r#type::caption(),
                inner - 120.0,
            ))
            .style(sys::r#type::caption())
            .color(secondary),
        ));
    }
    let mut header = vec![
        picture(
            &content.leading,
            LEADING_BIG,
            tint.unwrap_or(primary),
            activity.dim,
        ),
        boxed(
            amane::Column::new(text)
                .gap(2.0)
                .width(Size::Fixed(inner - LEADING_BIG - 12.0 - 72.0)),
        ),
    ];
    let big = match &content.trailing {
        Trailing::Progress(done) => {
            Some(boxed(progress_ring(*done, 28.0, tint.unwrap_or(primary))))
        }
        Trailing::Text(_) if activity.key == "now-playing" => None,
        trailing => trailing_text(trailing, false).map(|(text, style)| {
            boxed(Text::new(text).style(style).color(tint.unwrap_or(primary)))
        }),
    };
    header.extend(big);
    let header = Rectangle::new()
        .width(Size::Fixed(inner))
        .height(Size::Fixed(HEADER))
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .child(Row::new(header).gap(12.0).align(Center));

    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(header)];
    let mut height = padding * 2.0 + HEADER;

    if let Some(done) = content.progress {
        let track = Rectangle::new()
            .width(Size::Fixed(inner))
            .height(Size::Fixed(BAR))
            .radius(BAR / 2.0)
            .fill(material::fill_role(FillLevel::Secondary))
            .align_child(Start, Center)
            .child(
                Rectangle::new()
                    .width(Size::Fixed((inner * done).max(BAR)))
                    .height(Size::Fixed(BAR))
                    .radius(BAR / 2.0)
                    .fill(tint.unwrap_or_else(sys::color::accent::bg)),
            );
        rows.push(boxed(track));
        height += gap + BAR;
    }

    let buttons: Vec<&ActivityButton> = content.buttons.iter().take(3).collect();
    if !buttons.is_empty() {
        let row_gap = sys::space::gap::xs();
        let each = (inner - row_gap * (buttons.len() as f32 - 1.0)) / buttons.len() as f32;
        let row: Vec<Box<dyn Widget>> = buttons
            .into_iter()
            .map(|button| boxed(activity_button(activity, button, each)))
            .collect();
        rows.push(boxed(
            Row::new(row)
                .gap(row_gap)
                .height(Size::Fixed(BUTTONS))
                .align(Center),
        ));
        height += gap + BUTTONS;
    }

    let card = Card::new(rows)
        .material(Material::Menu)
        .variant(Variant::Outlined)
        .glass(true)
        .gap(gap)
        .padding(padding)
        .build();
    (card, height.ceil())
}

/// A button of the expanded view: a symbol alone, or a label.
fn activity_button(activity: &Shown, button: &ActivityButton, width: f32) -> Rectangle {
    let act = activity.act.clone();
    let id = button.id.clone();
    let key = format!("amane.bar.activity.{}.{}", activity.key, button.id);
    let style = if button.destructive {
        Style::Destructive
    } else {
        Style::Tonal
    };
    let glyph = button.icon.as_ref().and_then(|icon| match icon {
        ItemIcon::Glyph(name) => Some(match name.as_str() {
            "skip-previous" => ICON_SKIP_PREVIOUS,
            "skip-next" => ICON_SKIP_NEXT,
            "play" => ICON_PLAY,
            "pause" => ICON_PAUSE,
            name => named(name).unwrap_or(ICON_RECORD),
        }),
        _ => None,
    });
    let press = move || act(&id);
    match (glyph, button.label.is_empty()) {
        (Some(glyph), true) => IconButton::new(key, glyph)
            .style(style)
            .width(width)
            .height(Size::Fixed(BUTTONS))
            .on_press(press)
            .build(),
        (glyph, _) => {
            let mut push = PushButton::new(key, button.label.clone())
                .style(style)
                .width(width)
                .height(Size::Fixed(BUTTONS))
                .on_press(press);
            if let Some(glyph) = glyph {
                push = push.icon(glyph);
            }
            push.build()
        }
    }
}

/// `status-bar activities list | end <key>`: each going on, most relevant
/// first, as `key<TAB>app<TAB>title`; ending one is the user's, as an
/// activity can be dismissed.
pub fn ipc(args: &[&str]) -> String {
    match args {
        ["list"] => gather()
            .iter()
            .map(|activity| {
                format!(
                    "{}\t{}\t{}",
                    activity.key, activity.content.app, activity.content.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        ["end", key] => match key.strip_prefix("bus.").and_then(|h| h.parse().ok()) {
            Some(handle) => {
                registry::dismiss(handle);
                String::from("ok")
            }
            None => format!("only other programs' activities end here, as bus.<handle>; not {key}"),
        },
        // what a click on one of its buttons does
        ["press", key, button] => match key.strip_prefix("bus.").and_then(|h| h.parse().ok()) {
            Some(handle) => {
                registry::act(handle, button);
                String::from("ok")
            }
            None => format!("only other programs' activities are pressed here; not {key}"),
        },
        _ => String::from(
            "usage: status-bar activities (list | end bus.<handle> | press bus.<handle> <button>)",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdowns_never_go_below_nothing() {
        let (text, _) = trailing_text(&Trailing::Countdown(0), false).expect("a countdown");
        assert_eq!(text, "0:00");
    }

    #[test]
    fn text_needs_something_to_show() {
        assert!(trailing_text(&Trailing::Text(String::new()), false).is_none());
        assert!(trailing_text(&Trailing::None, false).is_none());
        assert!(trailing_text(&Trailing::Progress(0.5), false).is_none());
    }
}
