//! Notification center — a full-height sidebar at the right edge that
//! slides in when the clock is clicked, holding every notification of the
//! session until it's dismissed.
//!
//! Notifications are grouped by app, newest group first. A group of
//! several is an iOS stack: the newest card in front, up to
//! `notification.stack.max-peek` blank plates peeking below it, each
//! smaller and fainter. Clicking the stack springs it open into a list
//! under the app's name, with "Show Less" and a clear button; a single
//! notification is a plain card. Cards that go fade and slide away.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use amane::{
    Animation, Button, Center, Color, Column, Easing, Full, Horizontal, Key, Keyboard, Layer,
    LayerWindow, Margin, Notification, Notifications, Parent, Point, Pointer, Radius, Rectangle,
    Row, ScrollArea, Service, Size, SpaceBetween, Stack, Text, Vertical, Widget, Zone,
};
use chrono::{DateTime, Local};

use super::card::{self, Place};
use super::dnd::{DoNotDisturb, Until};
use super::motion::{self, Arrive, Item, Pose};
use crate::components::taskbar::{Clock, Panel, boxed, close, icon, is_open, pad};
use crate::tokens::{Styled, notification, sys};

/// Layer-surface namespace; the niri `layer-rule` matches it for blur, and
/// the Do Not Disturb menu hangs under it.
pub const CENTER_NS: &str = "amane.notifications.center";

const ICON_DND: char = '\u{ef44}';
const ICON_CLOSE: char = '\u{e5cd}';

// iOS-like motion; become `sys.motion.*` tokens once they exist in Penpot
const SLIDE: Duration = Duration::from_millis(500);
const SLIDE_BOUNCE: f32 = 0.1;

/// Every list of cards in the center is a `motion` scope under this.
const SCOPE: &str = "center.";

thread_local! {
    // 0 hidden off the right edge, 1 in place
    static SLIDE_IN: RefCell<Animation> = RefCell::new(
        Animation::new(0.0)
            .duration(SLIDE)
            .easing(Easing::Spring { bounce: SLIDE_BOUNCE }),
    );

    // the groups opened out from a stack, by key
    static EXPANDED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());

    // the groups drawn last time, so one whose last card went can finish
    // sliding it out
    static DRAWN: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };

    // the newest notification the user has seen, for the bar's dot
    static SEEN: Cell<u32> = const { Cell::new(0) };

    // where the pointer last was on the sheet, for anchoring the menu
    static CURSOR: Cell<(i32, i32)> = const { Cell::new((0, 0)) };
}

// ───── runtime ───────────────────────────────────────────────────────────────

/// The center opened: toasts go, everything counts as seen.
pub fn opened() {
    super::toasts::Toasts::hide_all();
    let newest = Notifications::read()
        .list()
        .iter()
        .map(Notification::id)
        .max()
        .unwrap_or(0);
    SEEN.with(|seen| seen.set(seen.get().max(newest)));
}

/// The center closed: stacks fold back up for next time.
pub fn closed() {
    EXPANDED.with_borrow_mut(HashSet::clear);
}

/// Whether something arrived since the center was last opened.
pub fn unseen() -> bool {
    let seen = SEEN.with(Cell::get);
    Notifications::read().list().iter().any(|n| n.id() > seen)
}

// ───── groups ────────────────────────────────────────────────────────────────

struct Group {
    key: String,
    name: String,
    // newest first
    cards: Vec<Notification>,
}

/// What groups a notification: its desktop entry, else its app name.
fn group_key(desktop_entry: &str, app_name: &str) -> String {
    let key = if desktop_entry.is_empty() {
        app_name
    } else {
        desktop_entry
    };
    key.to_lowercase()
}

/// Groups newest first, each newest card first. `list` is oldest first,
/// as amane keeps it.
fn groups(list: &[Notification]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for n in list.iter().rev() {
        let key = group_key(n.desktop_entry(), n.app_name());
        match groups.iter_mut().find(|group| group.key == key) {
            Some(group) => group.cards.push(n.clone()),
            None => groups.push(Group {
                key,
                name: card::app_name(n).to_string(),
                cards: vec![n.clone()],
            }),
        }
    }
    groups
}

// ───── view ──────────────────────────────────────────────────────────────────

pub fn center() -> LayerWindow {
    let open = is_open(Panel::Notifications);
    let progress = SLIDE_IN.with_borrow_mut(|slide| {
        slide.to(if open { 1.0 } else { 0.0 });
        slide.value()
    });
    let shown = open || progress > 0.001;

    let width = notification::center::width();
    let gap = sys::space::gap::m();
    // slides from just past the right edge to `gap` in from it
    let right = gap - (1.0 - progress) * (width + gap);

    let base = LayerWindow::new()
        .width(width)
        .height(Full)
        .anchor_vertical(Vertical::Top)
        .anchor_horizontal(Horizontal::Right)
        .margin(Margin {
            top: gap as i32,
            right: right.round() as i32,
            bottom: gap as i32,
            left: 0,
        })
        .layer(Layer::Overlay)
        .space(Zone::Respect)
        .keyboard(Keyboard::OnDemand)
        .namespace(CENTER_NS)
        .visible(shown)
        .on_key(|key| {
            if matches!(key, Key::Escape) {
                close();
            }
        });

    if !shown {
        motion::forget(SCOPE);
        DRAWN.with_borrow_mut(Vec::clear);
        return base.child(
            Rectangle::new()
                .width(Parent)
                .height(Parent)
                .fill(Color::TRANSPARENT),
        );
    }

    base.child(sheet())
}

fn sheet() -> Rectangle {
    // a clone, so no lock is held while handlers are built
    let list: Vec<Notification> = Notifications::read().list().to_vec();
    // read for its minute tick, which keeps "5m ago" current
    drop(Clock::read());
    let now = SystemTime::now();

    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(header(!list.is_empty()))];

    let groups = groups(&list);
    let mut drawn = Vec::new();
    let mut views: Vec<Box<dyn Widget>> = Vec::new();
    for group in &groups {
        drawn.push((group.key.clone(), group.name.clone()));
        if let Some(view) = group_view(&group.key, &group.name, &group.cards, now) {
            views.push(view);
        }
    }
    // a group that just emptied keeps drawing while its cards slide out
    for (key, name) in DRAWN.with_borrow(Vec::clone) {
        if !groups.iter().any(|group| group.key == key)
            && let Some(view) = group_view(&key, &name, &[], now)
        {
            drawn.push((key, name));
            views.push(view);
        }
    }
    DRAWN.with_borrow_mut(|old| *old = drawn);

    if views.is_empty() {
        rows.push(boxed(
            Rectangle::new()
                .width(Parent)
                .height(Parent)
                .fill(Color::TRANSPARENT)
                .align_child(Center, Center)
                .child(
                    Text::new("No Notifications")
                        .style(sys::r#type::label())
                        .color(sys::color::surface::popover::fg_secondary()),
                ),
        ));
    } else {
        rows.push(boxed(ScrollArea::new(
            "amane.notifications.center",
            Column::new(views).gap(sys::space::gap::l()).width(Parent),
        )));
    }

    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .radius(sys::radius::popover())
        .fill(sys::color::surface::popover::bg())
        .border(
            sys::border::hairline(),
            sys::color::surface::popover::stroke(),
        )
        .padding(pad(sys::space::inset::l(), sys::space::inset::l()))
        .on_move(|p: Point| CURSOR.with(|c| c.set((p.x as i32, p.y as i32))))
        .child(Column::new(rows).gap(sys::space::gap::l()).width(Parent))
}

/// "Notifications", what Do Not Disturb is doing, and the sheet's buttons.
fn header(any: bool) -> Row {
    let until = DoNotDisturb::read().until();

    let mut title: Vec<Box<dyn Widget>> = vec![boxed(
        Text::new("Notifications")
            .style(sys::r#type::title())
            .color(sys::color::surface::popover::fg()),
    )];
    if let Some(until) = until {
        title.push(boxed(
            Text::new(dnd_status(until))
                .style(sys::r#type::caption())
                .color(sys::color::surface::popover::fg_secondary()),
        ));
    }

    let dnd_color = if until.is_some() {
        sys::color::accent::bg()
    } else {
        sys::color::surface::popover::fg_secondary()
    };
    let side = sys::size::control::height();
    let dnd = Rectangle::new()
        .width(Size::Fixed(side))
        .height(Size::Fixed(side))
        .radius(Radius::Full)
        .fill(sys::color::surface::raised::bg())
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(|button| {
            if button == Button::Left || button == Button::Right {
                let (x, y) = CURSOR.with(Cell::get);
                DoNotDisturb::open_menu(CENTER_NS, x, y);
            }
        })
        .child(icon(ICON_DND, sys::size::icon::m(), dnd_color));

    let mut trailing: Vec<Box<dyn Widget>> = Vec::new();
    if any {
        trailing.push(boxed(pill("Clear All", Notifications::clear)));
    }
    trailing.push(boxed(dnd));

    Row::new(vec![
        boxed(Column::new(title).gap(sys::space::gap::_2xs())),
        boxed(Row::new(trailing).gap(sys::space::gap::s()).align(Center)),
    ])
    .width(Parent)
    .justify(SpaceBetween)
    .align(Center)
}

fn dnd_status(until: Until) -> String {
    match until {
        Until::Forever => String::from("Do Not Disturb"),
        Until::At(end) => format!(
            "Do Not Disturb until {}",
            DateTime::<Local>::from(end).format("%H:%M")
        ),
    }
}

/// A small text button.
fn pill(label: &'static str, press: impl Fn() + 'static) -> Rectangle {
    let text = Text::new(label)
        .style(sys::r#type::caption())
        .color(sys::color::surface::popover::fg());
    let width = match text.width() {
        Size::Fixed(width) => width,
        Size::Parent => 0.0,
    };

    Rectangle::new()
        .width(Size::Fixed(width + sys::space::inset::m() * 2.0))
        .height(Size::Fixed(sys::size::control::height()))
        .radius(Radius::Full)
        .fill(sys::color::surface::raised::bg())
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |button| {
            if button == Button::Left {
                press();
            }
        })
        .child(text)
}

/// What a stack draws: its title row, or a card.
#[derive(Clone)]
enum Entry {
    Header,
    Card(Box<Notification>),
}

const HEADER_KEY: u64 = u64::MAX;

/// One app's notifications: a stack, a list, or a lone card. `None` once
/// nothing in it is left to draw.
fn group_view(
    key: &str,
    name: &str,
    cards: &[Notification],
    now: SystemTime,
) -> Option<Box<dyn Widget>> {
    let width = Place::Center.width();
    let many = cards.len() > 1;
    let expanded = many && EXPANDED.with_borrow(|open| open.contains(key));
    let header_h = sys::size::control::height();

    let heights: Vec<f32> = cards
        .iter()
        .map(|n| card::height(n, Place::Center))
        .collect();
    let poses = stack_poses(&heights, expanded, header_h + sys::space::gap::s());

    let mut items: Vec<Item<Entry>> = Vec::new();
    if expanded {
        let pose = Pose::at(0.0);
        items.push(Item {
            key: HEADER_KEY,
            value: Entry::Header,
            height: header_h,
            pose,
            enter: pose.faded(),
            exit: pose.faded(),
        });
    }
    for ((n, height), pose) in cards.iter().zip(&heights).zip(&poses) {
        items.push(Item {
            key: u64::from(n.id()),
            value: Entry::Card(Box::new(n.clone())),
            height: *height,
            pose: *pose,
            enter: Pose {
                scale: 0.9,
                ..pose.faded()
            },
            exit: pose.moved(width).faded(),
        });
    }

    let scope = format!("{SCOPE}{key}");
    let shown = motion::animate(&scope, items, Arrive::Rest);
    if shown.is_empty() {
        return None;
    }

    let height = shown
        .iter()
        .filter(|s| s.pose.opacity > 0.01)
        .map(|s| s.pose.bottom(s.height))
        .fold(0.0, f32::max);

    // drawn bottom up: cards on their way out, the plates from the back,
    // the front card, then the title
    let rank = |key: u64| {
        if key == HEADER_KEY {
            return usize::MAX;
        }
        cards
            .iter()
            .position(|n| u64::from(n.id()) == key)
            .map_or(0, |i| cards.len() - i)
    };
    let mut order: Vec<_> = shown.into_iter().collect();
    order.sort_by_key(|s| (!s.leaving, rank(s.key)));

    let group = key.to_string();
    let ids: Vec<u32> = cards.iter().map(Notification::id).collect();
    let layers: Vec<Box<dyn Widget>> = order
        .into_iter()
        .map(|s| {
            let pose = s.pose;
            let view = match s.value {
                Entry::Header => group_header(name, &group, ids.clone(), width, header_h),
                Entry::Card(n) => {
                    let id = n.id();
                    let group = group.clone();
                    card::card(&n, Place::Center, now, pose.content)
                        .on_hover(move |inside| card::hover(id, inside))
                        .on_click(move |button| {
                            if button != Button::Left {
                                return;
                            }
                            if many && !expanded {
                                EXPANDED.with_borrow_mut(|open| open.insert(group.clone()));
                            } else {
                                Notifications::click(id);
                            }
                        })
                        .cursor(Pointer)
                }
            };
            boxed(
                view.translate(pose.x, pose.y)
                    .scale(pose.scale)
                    .opacity(pose.opacity.clamp(0.0, 1.0)),
            )
        })
        .collect();

    Some(boxed(
        Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height.max(1.0)))
            .fill(Color::TRANSPARENT)
            .clip()
            .child(Stack::new(layers)),
    ))
}

/// Where each card of a group goes, newest first. Opened out, they're a
/// list under the group's title (`top` tall); stacked, the newest is in
/// front and the next few are blank plates peeking out below it, each a
/// step smaller and fainter, and the rest hide behind.
fn stack_poses(heights: &[f32], expanded: bool, top: f32) -> Vec<Pose> {
    let gap = sys::space::gap::s();
    let peek = notification::stack::peek();
    let max_peek = notification::stack::max_peek() as usize;
    let scale_step = notification::stack::scale_step();
    let fade_step = notification::stack::fade_step();
    let front = heights.first().copied().unwrap_or(0.0);

    let mut y = if expanded && heights.len() > 1 {
        top
    } else {
        0.0
    };
    heights
        .iter()
        .enumerate()
        .map(|(i, &height)| {
            if expanded || i == 0 {
                let pose = Pose::at(y);
                y += height + gap;
                return pose;
            }
            let step = i.min(max_peek) as f32;
            let scale = 1.0 - scale_step * step;
            // its bottom edge shows `peek` below the one in front of it
            let bottom = front + peek * step;
            Pose {
                x: 0.0,
                y: bottom - height * (1.0 + scale) / 2.0,
                scale,
                opacity: if i > max_peek {
                    0.0
                } else {
                    1.0 - fade_step * step
                },
                content: 0.0,
            }
        })
        .collect()
}

/// An opened group's title: the app's name, "Show Less", and clear.
fn group_header(name: &str, group: &str, ids: Vec<u32>, width: f32, height: f32) -> Rectangle {
    let collapse = group.to_string();
    let clear = Rectangle::new()
        .width(Size::Fixed(height))
        .height(Size::Fixed(height))
        .radius(Radius::Full)
        .fill(sys::color::surface::raised::bg())
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |button| {
            if button == Button::Left {
                for id in &ids {
                    Notifications::dismiss(*id);
                }
            }
        })
        .child(icon(
            ICON_CLOSE,
            sys::size::icon::s(),
            sys::color::surface::popover::fg(),
        ));

    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .child(
            Row::new(vec![
                boxed(
                    Text::new(name.to_string())
                        .style(sys::r#type::title())
                        .color(sys::color::surface::popover::fg()),
                ),
                boxed(
                    Row::new(vec![
                        boxed(pill("Show Less", move || {
                            EXPANDED.with_borrow_mut(|open| open.remove(&collapse));
                        })),
                        boxed(clear),
                    ])
                    .gap(sys::space::gap::s())
                    .align(Center),
                ),
            ])
            .width(Parent)
            .height(Parent)
            .justify(SpaceBetween)
            .align(Center),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_by_desktop_entry_then_app_name() {
        assert_eq!(group_key("firefox", "Firefox Nightly"), "firefox");
        assert_eq!(group_key("", "Slack"), "slack");
        assert_eq!(group_key("Org.Gnome.Nautilus", ""), "org.gnome.nautilus");
    }

    #[test]
    fn a_stack_peeks_and_an_open_group_lists() {
        let heights = [60.0, 80.0, 60.0, 60.0];

        let stacked = stack_poses(&heights, false, 30.0);
        assert_eq!(stacked[0], Pose::at(0.0));
        // each plate behind ends lower, smaller and fainter
        assert!(stacked[1].bottom(80.0) > stacked[0].bottom(60.0));
        assert!(stacked[2].bottom(60.0) > stacked[1].bottom(80.0));
        assert!(stacked[1].scale > stacked[2].scale);
        assert!(stacked[1].opacity > stacked[2].opacity);
        assert_eq!(stacked[1].content, 0.0);
        // past the peeking ones, the rest hide behind
        assert_eq!(stacked[3].opacity, 0.0);

        let open = stack_poses(&heights, true, 30.0);
        assert_eq!(open[0].y, 30.0);
        assert!(
            open.iter()
                .all(|pose| pose.scale == 1.0 && pose.content == 1.0)
        );
        assert!(open[1].y > open[0].y + 60.0);
    }
}
