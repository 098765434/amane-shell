//! One list item: M3's anatomy (leading element, overline, headline,
//! supporting text, trailing elements) laid out from a [`ListTokens`].

use std::path::PathBuf;
use std::rc::Rc;

use amane::{
    Align, Center, Color, Column, Cursor, Gradient, Image, Padding, Radius, Rectangle, Row, Size,
    Stack, Start, Text, Widget,
};

use super::state::{self, Segment, State};
use super::style::{Line, ListStyle, ListTokens};
use crate::avatar::Avatar;
use crate::buttons::{Actions, Checkbox, Radio, Switch, held, hovered, interactive};
use crate::color::{Accent, with_alpha};
use crate::icons::{
    ICON_CHECK, ICON_CHEVRON_RIGHT, ICON_DRAG_HANDLE, ICON_EXPAND_MORE, ICON_IMAGE, ICON_PLAY,
};
use crate::layout::{boxed, icon, slot, wrapped_lines};
use crate::macform::MacLeadingIcon;
use crate::tokens::{Styled, TextStyle};

/// What sits before the text.
pub enum Leading {
    /// A Material Symbols glyph.
    Icon(char),
    /// A person: their initials on a circle, or their photo.
    Avatar {
        name: String,
        photo: Option<PathBuf>,
    },
    /// A square image (a placeholder until the file loads).
    Image(Option<PathBuf>),
    /// A video's thumbnail with a play mark, small or large.
    Video {
        thumbnail: Option<PathBuf>,
        large: bool,
    },
    Checkbox(bool),
    Radio(bool),
    Switch(bool),
    /// The glyph on an accent tile, as the shell's sidebars mark rows.
    Tile(char, Accent),
    Widget(Box<dyn Widget>),
}

/// What sits after the text. An item can have several.
pub enum Trailing {
    Icon(char),
    /// Trailing supporting text: "100+", a time, a percentage.
    Text(String),
    Checkbox(bool),
    Radio(bool),
    Switch(bool),
    /// Goes somewhere: a link item's chevron.
    Chevron,
    /// Opens and closes what's under it: a chevron that turns.
    Expand(bool),
    /// What a reorderable item is dragged by.
    DragHandle,
    /// The accent check of the chosen item.
    Check,
    Widget(Box<dyn Widget>),
}

/// How an item behaves: M3's (and Material Web's) item types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ItemKind {
    /// Shows something; can't be clicked or focused.
    #[default]
    Text,
    /// Does something when pressed.
    Button,
    /// Goes somewhere when pressed: ends with a chevron.
    Link,
}

/// One list item. `id` keys its states, so it must be unique and stable.
pub struct ListItem {
    pub(super) id: String,
    headline: String,
    overline: Option<String>,
    supporting: Option<String>,
    supporting_lines: usize,
    pub(super) leading: Option<Leading>,
    pub(super) trailing: Vec<Trailing>,
    kind: Option<ItemKind>,
    pub(super) selected: bool,
    pub(super) disabled: bool,
    pub(super) actions: Actions,
    pub(super) toggle: Option<Rc<dyn Fn(bool)>>,
    forced: Option<State>,
}

/// What building an item gives a list: the item, how tall it is, where
/// its text starts (for an inset divider) and what pressing it does (for
/// the keyboard).
pub(super) struct Built {
    pub rect: Rectangle,
    pub height: f32,
    pub text_x: f32,
    pub focusable: bool,
    pub press: Option<Rc<dyn Fn()>>,
}

/// How the list an item is in draws it this frame.
#[derive(Clone, Copy, Default)]
pub(super) struct Context {
    pub segment: Option<Segment>,
    pub focused: bool,
    pub dragged: bool,
}

/// How many lines an item is: three with an overline over supporting
/// text, or supporting text on two lines; two with either; else one.
pub fn line_type(overline: bool, supporting_lines: usize) -> usize {
    match (overline, supporting_lines) {
        (true, n) if n >= 1 => 3,
        (_, n) if n >= 2 => 3,
        (true, _) | (_, 1) => 2,
        _ => 1,
    }
}

impl ListItem {
    pub fn new(id: impl Into<String>, headline: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            headline: headline.into(),
            overline: None,
            supporting: None,
            supporting_lines: 1,
            leading: None,
            trailing: Vec::new(),
            kind: None,
            selected: false,
            disabled: false,
            actions: Actions::default(),
            toggle: None,
            forced: None,
        }
    }

    /// A small line over the headline: a category, a date.
    pub fn overline(mut self, text: impl Into<String>) -> Self {
        self.overline = Some(text.into()).filter(|t: &String| !t.is_empty());
        self
    }

    /// The line (or two) under the headline.
    pub fn supporting(mut self, text: impl Into<String>) -> Self {
        self.supporting = Some(text.into()).filter(|t: &String| !t.is_empty());
        self
    }

    /// Lets the supporting text wrap onto `lines` lines (1 or 2); two
    /// lines make a three-line item.
    pub fn supporting_lines(mut self, lines: usize) -> Self {
        self.supporting_lines = lines.clamp(1, 2);
        self
    }

    pub fn leading(mut self, leading: Leading) -> Self {
        self.leading = Some(leading);
        self
    }

    /// One more trailing element, after the others.
    pub fn trailing(mut self, trailing: Trailing) -> Self {
        self.trailing.push(trailing);
        self
    }

    /// Trailing supporting text: shorthand for `Trailing::Text`.
    pub fn trailing_text(self, text: impl Into<String>) -> Self {
        self.trailing(Trailing::Text(text.into()))
    }

    /// How it behaves; by default a button once it has an action, else text.
    pub fn kind(mut self, kind: ItemKind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Dimmed, and takes no clicks or focus.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_press(mut self, press: impl Fn() + 'static) -> Self {
        self.actions.press = Some(Rc::new(press));
        self
    }

    pub fn on_secondary(mut self, secondary: impl Fn() + 'static) -> Self {
        self.actions.secondary = Some(Rc::new(secondary));
        self
    }

    /// What the item's checkbox, radio or switch asks for: the whole item
    /// toggles it, as in M3.
    pub fn on_toggle(mut self, toggle: impl Fn(bool) + 'static) -> Self {
        self.toggle = Some(Rc::new(toggle));
        self
    }

    /// Draws it in `state` whatever the pointer does: for a showcase.
    pub fn force_state(mut self, state: State) -> Self {
        self.forced = Some(state);
        self
    }

    /// Builds the item alone, `width` wide, in `style`.
    pub fn render(self, style: ListStyle, width: f32) -> Rectangle {
        self.build(&style.tokens(), width, Context::default()).rect
    }

    /// The value of the item's control, if it has one.
    fn control(&self) -> Option<(bool, bool)> {
        let of = |radio: bool, on: bool| Some((radio, on));
        let leading = match &self.leading {
            Some(Leading::Checkbox(on) | Leading::Switch(on)) => of(false, *on),
            Some(Leading::Radio(on)) => of(true, *on),
            _ => None,
        };
        leading.or_else(|| {
            self.trailing.iter().find_map(|t| match t {
                Trailing::Checkbox(on) | Trailing::Switch(on) => of(false, *on),
                Trailing::Radio(on) => of(true, *on),
                _ => None,
            })
        })
    }

    /// Sets the item's checkbox, radio or switch to `on`: a list's
    /// selection drives it.
    pub(super) fn set_control(&mut self, on: bool) {
        let set = |value: &mut bool| *value = on;
        if let Some(Leading::Checkbox(v) | Leading::Switch(v) | Leading::Radio(v)) =
            &mut self.leading
        {
            set(v);
        }
        for t in &mut self.trailing {
            if let Trailing::Checkbox(v) | Trailing::Switch(v) | Trailing::Radio(v) = t {
                set(v);
            }
        }
    }

    /// What a press does: its action, else its control's toggle.
    fn press(&self) -> Option<Rc<dyn Fn()>> {
        if let Some(press) = &self.actions.press {
            return Some(press.clone());
        }
        let toggle = self.toggle.clone()?;
        let (radio, on) = self.control().unwrap_or((false, self.selected));
        let next = radio || !on;
        Some(Rc::new(move || toggle(next)))
    }

    pub(super) fn build(mut self, t: &ListTokens, width: f32, ctx: Context) -> Built {
        let press = self.press();
        let kind = self
            .kind
            .unwrap_or(if press.is_some() || self.actions.secondary.is_some() {
                ItemKind::Button
            } else {
                ItemKind::Text
            });
        if kind == ItemKind::Link && self.trailing.is_empty() {
            self.trailing.push(Trailing::Chevron);
        }
        let interactive_item = kind != ItemKind::Text && !self.disabled;

        // the state it's drawn in
        let animate = self.forced.is_none();
        let state = match self.forced {
            Some(state) => state,
            None if ctx.dragged => State::Dragged,
            None if !interactive_item => State::Enabled,
            None if held(&self.id) => State::Pressed,
            None if ctx.focused => State::Focused,
            None if hovered(&self.id) => State::Hovered,
            None => State::Enabled,
        };
        let state = if self.disabled { State::Enabled } else { state };
        let dragged = state == State::Dragged;

        // colours
        let content = if dragged {
            t.dragged_content
        } else if self.selected {
            t.selected_content
        } else {
            None
        };
        let paint = Paint {
            headline: content
                .or(if self.selected {
                    t.selected_headline
                } else {
                    None
                })
                .unwrap_or(t.headline_color),
            supporting: content.unwrap_or(t.supporting_color),
            overline: content.unwrap_or(t.overline_color),
            leading: content.unwrap_or(t.leading_color),
            trailing: if self.selected && !dragged {
                t.selected_trailing
            } else {
                content.unwrap_or(t.trailing_color)
            },
            trailing_text: content.unwrap_or(t.trailing_color),
        };
        let container = match () {
            _ if dragged && t.dragged_container.is_some() => {
                t.dragged_container.unwrap_or(t.container)
            }
            _ if self.selected => t.selected_container.unwrap_or(t.container),
            _ => t.container,
        };

        // the slots, built, so they can be measured
        let id = self.id.clone();
        let toggle = self.toggle.clone();
        let disabled = self.disabled;
        let leading = self
            .leading
            .take()
            .map(|l| leading_widget(l, t, &paint, &id, toggle.clone(), disabled));
        let trailing: Vec<Box<dyn Widget>> = std::mem::take(&mut self.trailing)
            .into_iter()
            .map(|tr| trailing_widget(tr, t, &paint, &id, toggle.clone(), disabled))
            .collect();
        let trailing = (!trailing.is_empty()).then(|| {
            Row::new(trailing)
                .gap(crate::tokens::sys::space::gap::s())
                .align(Center)
        });

        let lw = leading.as_deref().map_or(0.0, fixed_w);
        let lh = leading.as_deref().map_or(0.0, fixed_h);
        let tw = trailing.as_ref().map_or(0.0, |r| fixed_w(r));
        let th = trailing.as_ref().map_or(0.0, |r| fixed_h(r));
        let leading_room = if leading.is_some() {
            lw + t.leading_gap
        } else {
            0.0
        };
        let trailing_room = if trailing.is_some() {
            tw + t.trailing_gap
        } else {
            0.0
        };
        let text_w =
            (width - t.leading_space - t.trailing_space - leading_room - trailing_room).max(1.0);

        // the text, a line at a time
        let supporting = self.supporting.as_deref().map(|s| {
            if self.supporting_lines > 1 {
                wrapped_lines(s, t.supporting.style, text_w, self.supporting_lines)
            } else {
                vec![s.to_string()]
            }
        });
        let lines = line_type(
            self.overline.is_some(),
            supporting.as_ref().map_or(0, Vec::len),
        );
        let mut text: Vec<Box<dyn Widget>> = Vec::new();
        let mut text_h = 0.0;
        let mut push = |s: String, line: Line, color: Color, text: &mut Vec<Box<dyn Widget>>| {
            text_h += line.height;
            text.push(boxed(text_line(s, line, color, text_w)));
        };
        if let Some(overline) = self.overline.take() {
            push(overline, t.overline, paint.overline, &mut text);
        }
        push(
            std::mem::take(&mut self.headline),
            t.headline,
            paint.headline,
            &mut text,
        );
        for s in supporting.into_iter().flatten() {
            push(s, t.supporting, paint.supporting, &mut text);
        }

        let pad_y = t.padding_y(lines);
        let content_h = text_h.max(lh).max(th);
        let height = t.min_height(lines).max(content_h + 2.0 * pad_y);
        let top = lines == 3;
        let cross = if top { Align::Start } else { Align::Center };

        let mut row: Vec<Box<dyn Widget>> = Vec::new();
        if let Some(leading) = leading {
            row.push(leading);
            row.push(boxed(spacer(t.leading_gap)));
        }
        row.push(boxed(Column::new(text).width(Size::Fixed(text_w))));
        if let Some(trailing) = trailing {
            row.push(boxed(spacer(t.trailing_gap)));
            row.push(boxed(trailing));
        }
        let inner_w = width - t.leading_space - t.trailing_space;
        let mut content = Rectangle::new()
            .width(Size::Fixed(inner_w))
            .height(Size::Fixed(height - 2.0 * pad_y))
            .fill(Color::TRANSPARENT)
            .align_child(Start, cross)
            .child(Row::new(row).align(cross));
        if self.disabled {
            content = content.opacity(t.disabled_opacity);
        }

        // the container, its state layer and shape
        let opacity = state::layer(&self.id, state.layer_opacity(t), animate);
        let fill = t.layer_over(container, opacity);
        let target = state::corners(&t.shapes, state, self.selected, ctx.segment);
        let [tl, tr, br, bl] = state::shape(&self.id, target, animate);
        let mut rect = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height))
            .fill(fill)
            .radius_top_left(tl)
            .radius_top_right(tr)
            .radius_bottom_right(br)
            .radius_bottom_left(bl)
            .padding(Padding {
                top: pad_y,
                right: t.trailing_space,
                bottom: pad_y,
                left: t.leading_space,
            })
            .align_child(Start, Start)
            .clip()
            .child(content);
        if state == State::Focused {
            rect = rect.border(t.focus_ring_width, t.focus_ring);
        }
        if dragged {
            rect = rect.shadow(t.drag_shadow());
        }

        let focusable = interactive_item;
        let rect = if interactive_item {
            let actions = Actions {
                press: press.clone(),
                ..self.actions.clone()
            };
            interactive(rect, self.id, false, actions)
        } else {
            rect
        };

        Built {
            rect,
            height,
            text_x: t.leading_space + leading_room,
            focusable,
            press: if focusable { press } else { None },
        }
    }
}

struct Paint {
    headline: Color,
    supporting: Color,
    overline: Color,
    leading: Color,
    trailing: Color,
    trailing_text: Color,
}

fn fixed_w(widget: &dyn Widget) -> f32 {
    match widget.width() {
        Size::Fixed(w) => w,
        Size::Parent => 0.0,
    }
}

fn fixed_h(widget: &dyn Widget) -> f32 {
    match widget.height() {
        Size::Fixed(h) => h,
        Size::Parent => 0.0,
    }
}

fn spacer(width: f32) -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(1.0))
        .fill(Color::TRANSPARENT)
}

/// One line of text `width` wide and the line's height tall, elided.
fn text_line(text: String, line: Line, color: Color, width: f32) -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(line.height))
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .clip()
        .child(Text::new(text).style(line.style).color(color).elide())
}

fn trailing_label(text: String, style: TextStyle, color: Color) -> Text {
    Text::new(text).style(style).color(color)
}

/// An image, or a placeholder for one: a soft gradient with a glyph.
fn picture(path: Option<PathBuf>, w: f32, h: f32, radius: f32, t: &ListTokens) -> Rectangle {
    let frame = Rectangle::new()
        .width(Size::Fixed(w))
        .height(Size::Fixed(h))
        .radius(radius)
        .clip();
    match path.filter(|p| Image::loaded(p)) {
        Some(path) => frame.fill(Image::cover(path).thumbnail((w * 2.0) as u32, (h * 2.0) as u32)),
        None => frame
            .fill(Gradient::linear(
                135.0,
                [
                    (0.0, with_alpha(t.avatar_fill, 1.0)),
                    (1.0, with_alpha(t.selected_trailing, 0.55)),
                ],
            ))
            .align_child(Center, Center)
            .child(icon(
                ICON_IMAGE,
                (h * 0.42).min(24.0),
                with_alpha(t.avatar_text, 0.8),
            )),
    }
}

fn leading_widget(
    leading: Leading,
    t: &ListTokens,
    paint: &Paint,
    id: &str,
    toggle: Option<Rc<dyn Fn(bool)>>,
    disabled: bool,
) -> Box<dyn Widget> {
    let material = t.style.material_controls();
    match leading {
        Leading::Icon(glyph) => boxed(slot(t.icon).child(icon(glyph, t.icon, paint.leading))),
        Leading::Avatar { name, photo } => {
            let avatar = Avatar::new(name, t.avatar).photo(photo);
            boxed(if material {
                avatar
                    .colors(t.avatar_fill, t.avatar_text)
                    .style(t.avatar_label)
                    .build()
            } else {
                avatar.build()
            })
        }
        Leading::Image(path) => boxed(picture(path, t.image, t.image, t.image_radius, t)),
        Leading::Video { thumbnail, large } => {
            let (w, h) = if large { t.video_large } else { t.video_small };
            let video = picture(thumbnail, w, h, t.video_radius, t);
            let play = Rectangle::new()
                .width(Size::Fixed(w))
                .height(Size::Fixed(h))
                .fill(Color::TRANSPARENT)
                .align_child(Center, Center)
                .child(
                    Rectangle::new()
                        .width(Size::Fixed(28.0))
                        .height(Size::Fixed(28.0))
                        .radius(Radius::Full)
                        .fill(Color::rgba(0, 0, 0, 120))
                        .align_child(Center, Center)
                        .child(icon(ICON_PLAY, 20.0, Color::WHITE)),
                );
            boxed(
                Stack::new(vec![boxed(video), boxed(play)])
                    .width(Size::Fixed(w))
                    .height(Size::Fixed(h)),
            )
        }
        Leading::Checkbox(on) => control(Control::Checkbox, on, t, id, "lead", toggle, disabled),
        Leading::Radio(on) => control(Control::Radio, on, t, id, "lead", toggle, disabled),
        Leading::Switch(on) => control(Control::Switch, on, t, id, "lead", toggle, disabled),
        Leading::Tile(glyph, accent) => {
            let side =
                crate::tokens::sys::size::icon::m() + crate::tokens::sys::space::gap::xs() * 2.0;
            boxed(
                MacLeadingIcon::new(glyph)
                    .color(accent.color())
                    .side(side)
                    .build(),
            )
        }
        Leading::Widget(widget) => widget,
    }
}

fn trailing_widget(
    trailing: Trailing,
    t: &ListTokens,
    paint: &Paint,
    id: &str,
    toggle: Option<Rc<dyn Fn(bool)>>,
    disabled: bool,
) -> Box<dyn Widget> {
    let glyph = |g: char, color: Color| boxed(slot(t.icon).child(icon(g, t.icon, color)));
    match trailing {
        Trailing::Icon(g) => glyph(g, paint.trailing),
        Trailing::Text(s) => boxed(trailing_label(
            s,
            t.trailing_text.style,
            paint.trailing_text,
        )),
        Trailing::Checkbox(on) => control(Control::Checkbox, on, t, id, "trail", toggle, disabled),
        Trailing::Radio(on) => control(Control::Radio, on, t, id, "trail", toggle, disabled),
        Trailing::Switch(on) => control(Control::Switch, on, t, id, "trail", toggle, disabled),
        Trailing::Chevron => glyph(ICON_CHEVRON_RIGHT, paint.trailing),
        Trailing::Expand(open) => {
            let turn = state::spring(id, "expand", if open { 180.0 } else { 0.0 });
            boxed(
                slot(t.icon)
                    .rotate(turn)
                    .child(icon(ICON_EXPAND_MORE, t.icon, paint.trailing)),
            )
        }
        Trailing::DragHandle => boxed(slot(t.icon).cursor(Cursor::Grab).child(icon(
            ICON_DRAG_HANDLE,
            t.icon,
            paint.trailing,
        ))),
        Trailing::Check => glyph(ICON_CHECK, t.selected_trailing),
        Trailing::Widget(w) => w,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    Checkbox,
    Radio,
    Switch,
}

/// A selection control: the kit's own in the shell's style, which takes
/// its own click; M3's drawn, only showing the state, in the M3 styles,
/// where the whole item takes the click.
fn control(
    kind: Control,
    on: bool,
    t: &ListTokens,
    id: &str,
    at: &str,
    toggle: Option<Rc<dyn Fn(bool)>>,
    disabled: bool,
) -> Box<dyn Widget> {
    if !t.style.material_controls() {
        let key = format!("{id}.{at}");
        let toggle = toggle.unwrap_or_else(|| Rc::new(|_| {}));
        return match kind {
            Control::Checkbox => boxed(
                Checkbox::new(key, on)
                    .disabled(disabled)
                    .on_toggle(move |v| toggle(v))
                    .build(),
            ),
            Control::Radio => boxed(
                Radio::new(key, on)
                    .disabled(disabled)
                    .on_select(move || toggle(true))
                    .build(),
            ),
            Control::Switch => boxed(
                Switch::new(key, on)
                    .disabled(disabled)
                    .on_toggle(move |v| toggle(v))
                    .build(),
            ),
        };
    }

    match kind {
        // 18 dp box, 2 dp stroke, radius 2, in a 24 dp slot
        Control::Checkbox => {
            let mark = Rectangle::new()
                .width(Size::Fixed(18.0))
                .height(Size::Fixed(18.0))
                .radius(2.0)
                .align_child(Center, Center);
            let mark = if on {
                mark.fill(t.control_on)
                    .child(icon(ICON_CHECK, 16.0, t.control_on_mark))
            } else {
                mark.fill(Color::TRANSPARENT).border(2.0, t.control_off)
            };
            boxed(slot(24.0).child(mark))
        }
        // 20 dp ring, 2 dp stroke, a 10 dp dot when chosen
        Control::Radio => {
            let color = if on { t.control_on } else { t.control_off };
            let ring = Rectangle::new()
                .width(Size::Fixed(20.0))
                .height(Size::Fixed(20.0))
                .radius(Radius::Full)
                .fill(Color::TRANSPARENT)
                .border(2.0, color)
                .align_child(Center, Center);
            let ring = if on {
                ring.child(
                    Rectangle::new()
                        .width(Size::Fixed(10.0))
                        .height(Size::Fixed(10.0))
                        .radius(Radius::Full)
                        .fill(color),
                )
            } else {
                ring
            };
            boxed(slot(24.0).child(ring))
        }
        // a 52 × 32 track; the handle 16 off, 24 on
        Control::Switch => {
            // off: 16 at x 8; on: 24 at x 24 (4 from the end)
            let travel = state::spring(&format!("{id}.{at}"), "switch", if on { 1.0 } else { 0.0 });
            let handle = 16.0 + 8.0 * travel;
            let x = 8.0 + 16.0 * travel;
            let track = Rectangle::new()
                .width(Size::Fixed(52.0))
                .height(Size::Fixed(32.0))
                .radius(Radius::Full)
                .fill(if on { t.control_on } else { t.track_off });
            let track = if on {
                track
            } else {
                track.border(2.0, t.control_off)
            };
            let knob = Rectangle::new()
                .width(Size::Fixed(handle))
                .height(Size::Fixed(handle))
                .radius(Radius::Full)
                .fill(if on { t.control_on_mark } else { t.control_off })
                .translate(x, (32.0 - handle) / 2.0)
                .align_child(Center, Center);
            let knob = if on {
                knob.child(icon(ICON_CHECK, 16.0, t.control_on))
            } else {
                knob
            };
            boxed(
                Stack::new(vec![boxed(track), boxed(knob)])
                    .width(Size::Fixed(52.0))
                    .height(Size::Fixed(32.0)),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_come_from_the_content() {
        assert_eq!(line_type(false, 0), 1);
        assert_eq!(line_type(false, 1), 2);
        assert_eq!(line_type(true, 0), 2);
        assert_eq!(line_type(true, 1), 3);
        assert_eq!(line_type(false, 2), 3);
    }

    #[test]
    fn items_are_at_least_their_line_height() {
        for style in [ListStyle::M3, ListStyle::M3Expressive] {
            let t = style.tokens();
            let one = ListItem::new("t1", "Headline").build(&t, 360.0, Context::default());
            let two = ListItem::new("t2", "Headline")
                .supporting("Supporting text")
                .build(&t, 360.0, Context::default());
            let three = ListItem::new("t3", "Headline")
                .overline("Overline")
                .supporting("Supporting text")
                .build(&t, 360.0, Context::default());
            assert_eq!(one.height, 56.0, "{style:?}");
            assert_eq!(two.height, 72.0, "{style:?}");
            assert_eq!(three.height, 88.0, "{style:?}");
        }
    }

    #[test]
    fn a_tall_leading_element_grows_the_item() {
        let t = ListStyle::M3.tokens();
        let video = ListItem::new("tv", "Video")
            .leading(Leading::Video {
                thumbnail: None,
                large: true,
            })
            .build(&t, 360.0, Context::default());
        // 69 + 8 above and below
        assert_eq!(video.height, 85.0);
    }

    #[test]
    fn text_items_take_no_focus() {
        let t = ListStyle::M3.tokens();
        let text = ListItem::new("tt", "Text").build(&t, 360.0, Context::default());
        assert!(!text.focusable);
        let button =
            ListItem::new("tb", "Button")
                .on_press(|| {})
                .build(&t, 360.0, Context::default());
        assert!(button.focusable);
        let disabled = ListItem::new("td", "Off")
            .on_press(|| {})
            .disabled(true)
            .build(&t, 360.0, Context::default());
        assert!(!disabled.focusable);
    }

    #[test]
    fn the_text_starts_after_the_leading_element() {
        let t = ListStyle::M3.tokens();
        let item = ListItem::new("ti", "Inset")
            .leading(Leading::Icon(ICON_IMAGE))
            .build(&t, 360.0, Context::default());
        assert_eq!(item.text_x, 16.0 + 24.0 + 16.0);
    }
}
