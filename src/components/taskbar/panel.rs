//! Shared scaffolding for the bar's popover panels — Wi-Fi, Bluetooth, Sound
//! and power.
//!
//! Only one panel is open at a time: each is the same frosted card on its own
//! bottom-right `LayerWindow` (`Keyboard::OnDemand`, so a panel's text field
//! can take keys while the bar never steals focus), toggled by its bar
//! indicator and dismissed with Escape or by toggling again. The header,
//! switch, section, row and slider pieces here keep the four panels alike.

use std::cell::Cell;

use amane::{
    Button, Center, Color, Column, Horizontal, Key, Keyboard, Layer, LayerWindow, Margin, Parent,
    Point, Pointer, Rectangle, Row, Scroll, Size, SpaceBetween, Start, Text, Vertical, Widget,
    Zone, mark_all,
};

use super::{ICON_CHECK, bluetooth, boxed, icon, pad, wifi};
use crate::components::context_menu;
use crate::tokens::{Styled, sys};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Wifi,
    Bluetooth,
    Audio,
    Power,
}

thread_local! {
    static OPEN: Cell<Option<Panel>> = const { Cell::new(None) };

    // Last pointer position over the open card (surface-local). A right-click
    // handler only gets a Button, so it reads this to anchor a menu.
    static CURSOR: Cell<(i32, i32)> = const { Cell::new((0, 0)) };
}

// ───── runtime ───────────────────────────────────────────────────────────────

/// Opens `panel`, closing whichever other one is open, or closes it if it is.
pub fn toggle(panel: Panel) {
    set(if is_open(panel) { None } else { Some(panel) });
}

pub fn close() {
    set(None);
}

pub fn is_open(panel: Panel) -> bool {
    OPEN.with(Cell::get) == Some(panel)
}

/// Where the pointer last was over the open card, for anchoring a menu.
pub fn cursor() -> (i32, i32) {
    CURSOR.with(Cell::get)
}

fn set(next: Option<Panel>) {
    let previous = OPEN.with(|open| open.replace(next));
    if previous == next {
        return;
    }

    match previous {
        Some(Panel::Wifi) => wifi::reset(),
        Some(Panel::Bluetooth) => bluetooth::closed(),
        _ => {}
    }
    // a device menu belongs to the card it was opened from
    if previous.is_some() {
        context_menu::close();
    }

    mark_all();
}

// ───── view ──────────────────────────────────────────────────────────────────

/// The panel's surface: hidden and empty while closed, else `card()`.
pub fn surface(
    namespace: &'static str,
    panel: Panel,
    card: impl FnOnce() -> Rectangle,
) -> LayerWindow {
    let open = is_open(panel);

    let base = LayerWindow::new()
        .width(sys::size::popover::width())
        .height(sys::size::popover::height())
        .anchor_vertical(Vertical::Bottom)
        .anchor_horizontal(Horizontal::Right)
        .margin(Margin {
            top: 0,
            right: sys::space::gap::m() as i32,
            bottom: (sys::size::panel::height() + sys::space::gap::m()) as i32,
            left: 0,
        })
        .layer(Layer::Overlay)
        .space(Zone::Respect)
        .keyboard(Keyboard::OnDemand)
        .namespace(namespace)
        .visible(open)
        .on_key(|key| {
            if matches!(key, Key::Escape) {
                close();
            }
        });

    if !open {
        return base.child(
            Rectangle::new()
                .width(Parent)
                .height(Parent)
                .fill(Color::TRANSPARENT),
        );
    }

    base.child(card())
}

/// The frosted card every panel draws, its sections `gap` apart.
pub fn card(body: Vec<Box<dyn Widget>>, gap: f32) -> Rectangle {
    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .radius(sys::radius::popover())
        .fill(sys::color::surface::panel::bg())
        .blur(sys::effect::backdrop_blur())
        .padding(pad(sys::space::inset::l(), sys::space::inset::l()))
        .on_move(|p: Point| CURSOR.with(|c| c.set((p.x as i32, p.y as i32))))
        .child(Column::new(body).gap(gap).width(Parent))
}

/// Glyph + title on the left, the panel's own controls on the right.
pub fn header(
    glyph: char,
    title: impl Into<String>,
    color: Color,
    trailing: Vec<Box<dyn Widget>>,
) -> Row {
    let left = Row::new(vec![
        boxed(icon(glyph, sys::size::icon::m(), color)),
        boxed(
            Text::new(title.into())
                .style(sys::r#type::title())
                .color(color),
        ),
    ])
    .gap(sys::space::gap::s())
    .align(Center);

    Row::new(vec![
        boxed(left),
        boxed(Row::new(trailing).gap(sys::space::gap::s()).align(Center)),
    ])
    .width(Parent)
    .justify(SpaceBetween)
    .align(Center)
}

/// An On/Off pill; clicking it asks for the other state.
pub fn switch(on: bool, toggle: impl Fn(bool) + 'static) -> Rectangle {
    let (label, color) = if on {
        ("On", sys::color::accent::bg())
    } else {
        ("Off", sys::color::content::secondary())
    };

    Rectangle::new()
        .width(Size::Fixed(sys::size::switch::width()))
        .height(Size::Fixed(sys::size::switch::height()))
        .radius(sys::size::switch::height() / 2.0)
        .fill(sys::color::surface::raised::bg())
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |button| {
            if button == Button::Left {
                toggle(!on);
            }
        })
        .child(Text::new(label).style(sys::r#type::caption()).color(color))
}

/// A secondary caption: a section title, or a note like "Searching…".
pub fn caption(text: impl Into<String>) -> Text {
    Text::new(text.into())
        .style(sys::r#type::caption())
        .color(sys::color::content::secondary())
}

/// One list entry: glyph, elided label, trailing badges. The selected row
/// (joined network, default output, connected device) is filled and its label
/// full-strength; the rest read secondary. Callers add the click handlers.
pub fn list_row(
    glyph: char,
    label: impl Into<String>,
    trailing: Vec<Box<dyn Widget>>,
    selected: bool,
) -> Rectangle {
    let color = if selected {
        sys::color::surface::panel::fg()
    } else {
        sys::color::content::secondary()
    };
    let label_w = (sys::size::popover::width() - sys::size::row::trailing_width()).max(1.0);

    let left = Row::new(vec![
        boxed(icon(glyph, sys::size::icon::m(), color)),
        boxed(
            Rectangle::new()
                .width(Size::Fixed(label_w))
                .height(Parent)
                .fill(Color::TRANSPARENT)
                .align_child(Start, Center)
                .clip()
                .child(
                    Text::new(label.into())
                        .style(sys::r#type::label())
                        .color(color)
                        .elide(),
                ),
        ),
    ])
    .gap(sys::space::gap::m())
    .align(Center);

    let content = Row::new(vec![
        boxed(left),
        boxed(Row::new(trailing).gap(sys::space::gap::s()).align(Center)),
    ])
    .width(Parent)
    .justify(SpaceBetween)
    .align(Center);

    let fill = if selected {
        sys::color::state::selected()
    } else {
        sys::color::surface::raised::bg()
    };

    Rectangle::new()
        .width(Parent)
        .height(Size::Fixed(sys::size::row::height()))
        .radius(sys::radius::raised())
        .fill(fill)
        .padding(pad(sys::space::inset::l(), 0.0))
        .align_child(Center, Center)
        .clip()
        .child(content)
}

/// The accent check a selected row ends with.
pub fn check() -> Box<dyn Widget> {
    boxed(icon(
        ICON_CHECK,
        sys::size::icon::m(),
        sys::color::accent::bg(),
    ))
}

/// A 0–100 slider `width` wide: press or drag anywhere on it, or scroll in
/// steps of 5 like the bar's volume pill. `active` false greys the fill (muted).
pub fn slider(value: u8, active: bool, width: f32, set: fn(u8)) -> Rectangle {
    let track_h = sys::size::slider::height();
    let fill = if active {
        sys::color::accent::bg()
    } else {
        sys::color::content::secondary()
    };

    let track = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(track_h))
        .radius(track_h / 2.0)
        .fill(sys::color::surface::raised::bg())
        .clip()
        .child(
            Rectangle::new()
                .width(Size::Fixed(width * f32::from(value) / 100.0))
                .height(Size::Fixed(track_h))
                .radius(track_h / 2.0)
                .fill(fill),
        );

    // the hit area is a full control tall, so the thin track is easy to grab
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(sys::size::control::height()))
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .cursor(Pointer)
        .on_drag(move |p: Point| {
            // the drag keeps reporting past either end, so it is clamped
            let ratio = (p.x / width).clamp(0.0, 1.0);
            set((ratio * 100.0).round() as u8);
        })
        .on_scroll(move |scroll: Scroll| {
            // scroll up (negative y) raises, down lowers, in 5% steps
            let next = if scroll.y < 0.0 {
                value.saturating_add(5).min(100)
            } else {
                value.saturating_sub(5)
            };
            set(next);
        })
        .child(track)
}
