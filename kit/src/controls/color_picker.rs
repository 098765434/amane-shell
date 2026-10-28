//! A colour picker: a saturation and brightness square, a hue strip, an
//! alpha strip over a checkerboard, a hex field and the colours picked
//! last. Colours are red, green, blue and alpha from 0 to 1, not
//! premultiplied, as the Shader Lab's `values` take them.
//!
//! Each square and strip measures its own drag and stays still while the
//! handles on it move, which keeps drags steady (amane measures a drag
//! against the last frame's hit areas).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use amane::{
    Center, Color, Column, Gradient, Point, Radius, Rectangle, Row, Size, Stack, Start, TextInput,
    Widget,
};

use crate::app::{field_box, text_field};
use crate::layout::boxed;
use crate::material;
use crate::tokens::sys;

/// How many picked colours the row under the picker keeps.
const RECENT: usize = 8;

/// The strips' height and the handles' size.
const STRIP: f32 = 14.0;
const HANDLE: f32 = 14.0;

thread_local! {
    // the hue and saturation a grey or black colour came from, by picker id,
    // so dragging through grey doesn't lose them
    static HSV: RefCell<HashMap<String, [f32; 3]>> = RefCell::new(HashMap::new());

    static RECENT_COLORS: RefCell<Vec<[f32; 4]>> = const { RefCell::new(Vec::new()) };
}

pub struct ColorPicker {
    id: String,
    color: [f32; 4],
    width: f32,
    on_change: Rc<dyn Fn([f32; 4])>,
}

impl ColorPicker {
    /// `id` keys the picker's hex field and the hue it remembers, so it must
    /// be unique and stable.
    pub fn new(id: impl Into<String>, color: [f32; 4]) -> Self {
        Self {
            id: id.into(),
            color,
            width: 260.0,
            on_change: Rc::new(|_| {}),
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Runs with the new colour on every change, while dragging too.
    pub fn on_change(mut self, change: impl Fn([f32; 4]) + 'static) -> Self {
        self.on_change = Rc::new(change);
        self
    }

    pub fn build(self) -> Column {
        let Self {
            id,
            color,
            width,
            on_change,
        } = self;

        let [hue, saturation, value] = remembered_hsv(&id, color);
        let alpha = color[3];

        // every change goes through here, so the hue is remembered with it
        let set = {
            let id = id.clone();

            Rc::new(move |hsv: [f32; 3], alpha: f32| {
                HSV.with_borrow_mut(|known| known.insert(id.clone(), hsv));

                let [red, green, blue] = hsv_to_rgb(hsv);

                on_change([red, green, blue, alpha]);
            })
        };

        let square = square(width, [hue, saturation, value], alpha, set.clone());
        let hues = hue_strip(width, [hue, saturation, value], alpha, set.clone());
        let alphas = alpha_strip(width, [hue, saturation, value], alpha, set.clone());

        Column::new(vec![
            boxed(square),
            boxed(hues),
            boxed(alphas),
            boxed(hex_row(&id, color, width, set.clone())),
            boxed(recent_row(width, set)),
        ])
        .gap(sys::space::gap::s())
    }
}

/// Saturation across, brightness up.
fn square(width: f32, hsv: [f32; 3], alpha: f32, set: Rc<dyn Fn([f32; 3], f32)>) -> Rectangle {
    let height = (width * 0.62).round();
    let [hue, saturation, value] = hsv;

    let [red, green, blue] = hsv_to_rgb([hue, 1.0, 1.0]);
    let pure = to_color([red, green, blue, 1.0]);

    let across = Rectangle::new()
        .width(width)
        .height(height)
        .radius(sys::radius::raised())
        .fill(Gradient::linear(90.0, [(0.0, Color::WHITE), (1.0, pure)]));

    let down = Rectangle::new()
        .width(width)
        .height(height)
        .radius(sys::radius::raised())
        .fill(Gradient::linear(
            0.0,
            [(0.0, Color::BLACK), (1.0, Color::rgba(0, 0, 0, 0))],
        ))
        .border(sys::border::hairline(), material::separator());

    let handle = ring().translate(
        saturation * width - HANDLE / 2.0,
        (1.0 - value) * height - HANDLE / 2.0,
    );

    Rectangle::new()
        .width(width)
        .height(height)
        .fill(Color::TRANSPARENT)
        .cursor(amane::Crosshair)
        .on_drag(move |point: Point| {
            let saturation = (point.x / width).clamp(0.0, 1.0);
            let value = 1.0 - (point.y / height).clamp(0.0, 1.0);

            set([hue, saturation, value], alpha);
        })
        .on_release(move |_| remember(hsv, alpha))
        .child(Stack::new(vec![boxed(across), boxed(down), boxed(handle)]))
}

fn hue_strip(width: f32, hsv: [f32; 3], alpha: f32, set: Rc<dyn Fn([f32; 3], f32)>) -> Rectangle {
    let [hue, saturation, value] = hsv;

    let stops: Vec<(f32, Color)> = (0..=6)
        .map(|step| {
            let at = step as f32 / 6.0;
            let [red, green, blue] = hsv_to_rgb([at, 1.0, 1.0]);

            (at, to_color([red, green, blue, 1.0]))
        })
        .collect();

    let strip = Rectangle::new()
        .width(width)
        .height(STRIP)
        .radius(Radius::Full)
        .fill(Gradient::linear(90.0, stops))
        .border(sys::border::hairline(), material::separator());

    strip_input(width, strip, hue, move |hue| {
        set([hue, saturation, value], alpha)
    })
}

fn alpha_strip(width: f32, hsv: [f32; 3], alpha: f32, set: Rc<dyn Fn([f32; 3], f32)>) -> Rectangle {
    let [red, green, blue] = hsv_to_rgb(hsv);

    let fade = Rectangle::new()
        .width(width)
        .height(STRIP)
        .radius(Radius::Full)
        .fill(Gradient::linear(
            90.0,
            [
                (0.0, to_color([red, green, blue, 0.0])),
                (1.0, to_color([red, green, blue, 1.0])),
            ],
        ))
        .border(sys::border::hairline(), material::separator());

    let behind = Rectangle::new()
        .width(width)
        .height(STRIP)
        .radius(Radius::Full)
        .clip()
        .child(checkerboard(width, STRIP));

    let strip = Rectangle::new()
        .width(width)
        .height(STRIP)
        .child(Stack::new(vec![boxed(behind), boxed(fade)]));

    strip_input(width, strip, alpha, move |alpha| set(hsv, alpha))
}

/// A strip with a handle at `at` (0 to 1), dragged across.
fn strip_input(width: f32, strip: Rectangle, at: f32, set: impl Fn(f32) + 'static) -> Rectangle {
    let height = HANDLE + 4.0;

    let handle = ring().translate(at * (width - HANDLE), (height - HANDLE) / 2.0);

    Rectangle::new()
        .width(width)
        .height(height)
        .fill(Color::TRANSPARENT)
        .cursor(amane::Pointer)
        .on_drag(move |point: Point| set((point.x / width).clamp(0.0, 1.0)))
        .child(Stack::new(vec![
            boxed(
                Rectangle::new()
                    .width(width)
                    .height(height)
                    .align_child(Start, Center)
                    .child(strip),
            ),
            boxed(handle),
        ]))
}

/// The handle: a white ring with a dark edge, so it shows on any colour.
fn ring() -> Rectangle {
    Rectangle::new()
        .width(HANDLE)
        .height(HANDLE)
        .radius(Radius::Full)
        .fill(Color::rgba(0, 0, 0, 0))
        .border(2.5, Color::WHITE)
        .child(
            Rectangle::new()
                .width(Size::Parent)
                .height(Size::Parent)
                .radius(Radius::Full)
                .border(0.75, Color::rgba(0, 0, 0, 140)),
        )
}

/// Light and dark squares, for colours that are partly clear.
fn checkerboard(width: f32, height: f32) -> Column {
    let side = height / 2.0;
    let columns = (width / side).ceil() as usize;

    let rows = (0..2)
        .map(|row| {
            let cells = (0..columns)
                .map(|column| {
                    let light = (row + column) % 2 == 0;
                    let shade = if light { 255 } else { 204 };

                    boxed(
                        Rectangle::new()
                            .width(side)
                            .height(side)
                            .fill(Color::rgb(shade, shade, shade)),
                    )
                })
                .collect();

            boxed(Row::new(cells))
        })
        .collect();

    Column::new(rows)
}

fn hex_row(id: &str, color: [f32; 4], width: f32, set: Rc<dyn Fn([f32; 3], f32)>) -> Row {
    let field_id = static_id(format!("{id}.hex"));
    let hex = to_hex(color);

    if !TextInput::has_focus(field_id) && TextInput::text(field_id) != hex {
        TextInput::set_text(field_id, &hex);
    }

    let swatch_side = sys::size::control::height();

    let swatch = Rectangle::new()
        .width(swatch_side)
        .height(swatch_side)
        .radius(sys::radius::raised())
        .clip()
        .child(Stack::new(vec![
            boxed(checkerboard(swatch_side, swatch_side)),
            boxed(
                Rectangle::new()
                    .width(swatch_side)
                    .height(swatch_side)
                    .fill(to_color(color))
                    .border(sys::border::hairline(), material::separator()),
            ),
        ]));

    let typed = move |text: String| {
        if let Some(color) = parse_hex(&text) {
            let [red, green, blue, alpha] = color;

            set(rgb_to_hsv([red, green, blue]), alpha);

            remember(rgb_to_hsv([red, green, blue]), alpha);
        }
    };

    Row::new(vec![
        boxed(swatch),
        boxed(field_box(
            field_id,
            text_field(field_id, "#rrggbb").on_submit(typed),
            width - swatch_side - sys::space::gap::s(),
        )),
    ])
    .gap(sys::space::gap::s())
}

/// The colours picked last; clicking one picks it again.
fn recent_row(width: f32, set: Rc<dyn Fn([f32; 3], f32)>) -> Row {
    let side = ((width - sys::space::gap::xs() * (RECENT as f32 - 1.0)) / RECENT as f32).min(22.0);

    let swatches = RECENT_COLORS.with_borrow(|recent| {
        recent
            .iter()
            .map(|&color| {
                let set = set.clone();
                let [red, green, blue, alpha] = color;

                boxed(
                    Rectangle::new()
                        .width(side)
                        .height(side)
                        .radius(Radius::Full)
                        .fill(to_color(color))
                        .border(sys::border::hairline(), material::separator())
                        .cursor(amane::Pointer)
                        .on_click(move |_| set(rgb_to_hsv([red, green, blue]), alpha)),
                )
            })
            .collect::<Vec<Box<dyn Widget>>>()
    });

    Row::new(swatches).gap(sys::space::gap::xs())
}

/// Puts a picked colour first in the recent row, once.
fn remember(hsv: [f32; 3], alpha: f32) {
    let [red, green, blue] = hsv_to_rgb(hsv);
    let color = [red, green, blue, alpha];

    RECENT_COLORS.with_borrow_mut(|recent| {
        recent.retain(|kept| to_hex(*kept) != to_hex(color));
        recent.insert(0, color);
        recent.truncate(RECENT);
    });
}

/// The hue and saturation kept for this picker when `color` is what they make.
fn remembered_hsv(id: &str, color: [f32; 4]) -> [f32; 3] {
    let [red, green, blue, _] = color;

    let kept = HSV.with_borrow(|known| known.get(id).copied());

    match kept {
        Some(hsv) if to_hex_rgb(hsv_to_rgb(hsv)) == to_hex_rgb([red, green, blue]) => hsv,
        _ => rgb_to_hsv([red, green, blue]),
    }
}

/// A text field's id has to be `'static`; there's one per picker, made once.
fn static_id(id: String) -> &'static str {
    thread_local! {
        static IDS: RefCell<HashMap<String, &'static str>> = RefCell::new(HashMap::new());
    }

    IDS.with_borrow_mut(|ids| {
        *ids.entry(id.clone())
            .or_insert_with(|| Box::leak(id.into_boxed_str()))
    })
}

pub fn to_color(color: [f32; 4]) -> Color {
    let [red, green, blue, alpha] =
        color.map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8);

    Color::rgba(red, green, blue, alpha)
}

/// `#rrggbb` or `#rrggbbaa` as red, green, blue and alpha from 0 to 1.
pub fn parse_hex(text: &str) -> Option<[f32; 4]> {
    let digits = text.trim().trim_start_matches('#');

    if !matches!(digits.len(), 6 | 8) || !digits.is_ascii() {
        return None;
    }

    let channel = |index: usize| {
        u8::from_str_radix(&digits[index * 2..index * 2 + 2], 16)
            .ok()
            .map(|value| f32::from(value) / 255.0)
    };

    let alpha = if digits.len() == 8 { channel(3)? } else { 1.0 };

    Some([channel(0)?, channel(1)?, channel(2)?, alpha])
}

/// `#rrggbb`, or `#rrggbbaa` when it's partly clear.
pub fn to_hex(color: [f32; 4]) -> String {
    let [red, green, blue, alpha] =
        color.map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8);

    if alpha == 255 {
        format!("#{red:02x}{green:02x}{blue:02x}")
    } else {
        format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
    }
}

fn to_hex_rgb(rgb: [f32; 3]) -> String {
    let [red, green, blue] = rgb;

    to_hex([red, green, blue, 1.0])
}

/// Hue, saturation and value from 0 to 1.
pub fn rgb_to_hsv(rgb: [f32; 3]) -> [f32; 3] {
    let [red, green, blue] = rgb.map(|channel| channel.clamp(0.0, 1.0));

    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;

    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == red {
        ((green - blue) / delta).rem_euclid(6.0) / 6.0
    } else if max == green {
        ((blue - red) / delta + 2.0) / 6.0
    } else {
        ((red - green) / delta + 4.0) / 6.0
    };

    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };

    [hue, saturation, max]
}

pub fn hsv_to_rgb(hsv: [f32; 3]) -> [f32; 3] {
    let [hue, saturation, value] = hsv;

    let sector = (hue.rem_euclid(1.0) * 6.0).clamp(0.0, 5.999_999);
    let chroma = value * saturation;
    let x = chroma * (1.0 - ((sector % 2.0) - 1.0).abs());
    let base = value - chroma;

    let (red, green, blue) = match sector as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };

    [red + base, green + base, blue + base]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_goes_there_and_back() {
        for hex in [
            "#ff0000", "#00ff80", "#336699", "#0a5d37", "#ffffff", "#000000", "#808080",
        ] {
            let [red, green, blue, _] = parse_hex(hex).unwrap();
            let back = hsv_to_rgb(rgb_to_hsv([red, green, blue]));

            assert_eq!(to_hex_rgb(back), hex, "{hex}");
        }
    }

    #[test]
    fn primaries_have_their_hues() {
        assert_eq!(rgb_to_hsv([1.0, 0.0, 0.0]), [0.0, 1.0, 1.0]);
        assert!((rgb_to_hsv([0.0, 1.0, 0.0])[0] - 1.0 / 3.0).abs() < 1e-6);
        assert!((rgb_to_hsv([0.0, 0.0, 1.0])[0] - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn hex_reads_and_writes() {
        assert_eq!(parse_hex("#ff0080"), Some([1.0, 0.0, 128.0 / 255.0, 1.0]));
        assert_eq!(parse_hex("00000000"), Some([0.0; 4]));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(to_hex([1.0, 0.0, 128.0 / 255.0, 1.0]), "#ff0080");
        assert_eq!(to_hex([0.0; 4]), "#00000000");
    }

    #[test]
    fn grey_keeps_the_hue_it_came_from() {
        let id = "test.grey";
        let hsv = [0.6, 0.0, 0.5];

        HSV.with_borrow_mut(|known| known.insert(id.to_owned(), hsv));

        let [red, green, blue] = hsv_to_rgb(hsv);

        assert_eq!(remembered_hsv(id, [red, green, blue, 1.0]), hsv);
    }
}
