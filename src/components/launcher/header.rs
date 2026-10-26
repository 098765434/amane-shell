//! The search row along the top of the glass, the round mode buttons
//! beside the pill, and Applications' bar of category pills.
//!
//! The search row's text, caret and selection's wash are drawn by hand.
//! It leads with the mode's glyph, which turns into a back button under
//! the pointer once a mode is picked, and a token for the scope the
//! search is narrowed to; it ends with a button that clears the query, the
//! keys that pick the mode under the pointer, or the mode's ⋯ menu.
//!
//! The picked pill's fill is one piece sliding under the pills, so it
//! follows the fingers as they turn the category; the pills' names take
//! the accent's colour as it reaches them.

use amane::{Blend, Center, Color, Rectangle, Row, Stack, Start, Text, Widget};
use amane_kit::settings::{AppsLayout, change_launcher};

use crate::components::color::with_alpha;
use crate::components::context_menu::{self, ContextMenu, Icon, MenuItem};
use crate::components::icons::{
    ICON_CHECK, ICON_CHEVRON_LEFT, ICON_CLOSE, ICON_MORE_HORIZ, ICON_SEARCH,
};
use crate::components::kinetic::Axis;
use crate::components::layout::{boxed, cursor, icon, pad, slot, text_width};
use crate::components::material::{self, FillLevel, Level, Material};
use crate::components::motion;
use crate::components::scroll_view::ScrollView;
use crate::components::text_edit::Field;
use crate::services::clipboard_history;
use crate::tokens::{Styled, TextStyle, sys};

use super::entries::Scope;
use super::mode::Mode;
use super::shape::{CIRCLE, Geometry, PILL_H};
use super::{
    LAUNCHER_NS, PILL_SCROLL, PILLS, View, back, model, on_pill_scroll, switch_mode, turn_to,
    type_into, with,
};

// TODO(token): `launcher.search.*`, `launcher.keycap.*`, `launcher.token.*`
// and `launcher.focus-ring.*` once Penpot has them (manual §1.1).
/// How big the search text is.
const SEARCH_TEXT: f32 = 22.0;
/// The caret's width and height.
const CARET: (f32, f32) = (2.0, 26.0);
/// How big the glyph leading the field is, and the buttons ending it.
const LEADING: f32 = 30.0;
const GLYPH: f32 = 22.0;
/// The room either end of the search row.
const ROW_INSET: f32 = 16.0;
/// How tall a keycap or the scope's token is.
const KEYCAP: f32 = 24.0;
/// How big a mode button's glyph is, and how much it lifts under the
/// pointer.
const MODE_GLYPH: f32 = 24.0;
const MODE_LIFT: f32 = 1.06;
/// How far the mode buttons slide in towards the pill as they go.
const MODE_SLIDE: f32 = 12.0;
/// The keyboard focus ring round the picked pill: how thick, how far out,
/// and how strong.
const RING: f32 = 2.0;
const RING_GAP: f32 = 2.0;
const RING_ALPHA: f32 = 0.55;

/// Registry key for a mode's ⋯ menu.
const MORE_MENU_KEY: &str = "amane.launcher.more-menu";

fn search_style() -> TextStyle {
    TextStyle {
        size: SEARCH_TEXT,
        ..sys::r#type::title()
    }
}

/// The search row: the mode's glyph, the scope's token and the field, and
/// what ends the row. The caret shows while the keys go to the field
/// (`focused`); `hover_mode` is the mode button under the pointer, whose
/// name the empty pill shows, with the keys that pick it.
pub(super) fn search_row(
    view: &View,
    focused: bool,
    hover_mode: Option<Mode>,
    back_hover: bool,
) -> Rectangle {
    let gap = sys::space::gap::m();
    let width = view.width() - ROW_INSET * 2.0;
    let field = &view.field;
    let typed = !field.text().is_empty();

    let mut leading: Vec<Box<dyn Widget>> = vec![boxed(leading_glyph(view.mode, back_hover))];
    let mut used = LEADING + gap;
    if let Some(scope) = &view.scope {
        let token = token(scope.name());
        used += token_width(scope.name()) + gap;
        leading.push(boxed(token));
    }

    let placeholder = match hover_mode {
        Some(mode) if view.mode == Mode::Home && !typed => mode.title(),
        _ => view
            .scope
            .as_ref()
            .and_then(Scope::placeholder)
            .unwrap_or(view.mode.title()),
    };
    let trailing: Option<Box<dyn Widget>> = if typed {
        used += LEADING + gap;
        Some(boxed(round_button(ICON_CLOSE, || {
            type_into(|field| {
                *field = Field::new();
                true
            });
            super::Launcher::changed();
        })))
    } else if let Some(digit) = hover_mode
        .and_then(Mode::digit)
        .filter(|_| view.mode == Mode::Home)
    {
        let digit = digit.to_string();
        used += keycap_width("Ctrl") + keycap_width(&digit) + sys::space::gap::_2xs() + gap;
        Some(boxed(
            Row::new(vec![boxed(keycap("Ctrl")), boxed(keycap(&digit))])
                .gap(sys::space::gap::_2xs())
                .align(Center),
        ))
    } else if view.mode == Mode::Apps {
        used += LEADING + gap;
        Some(boxed(round_button(ICON_MORE_HORIZ, more_menu)))
    } else if view.mode == Mode::Clipboard && amane_kit::settings::launcher().clipboard.enabled {
        used += LEADING + gap;
        Some(boxed(round_button(ICON_MORE_HORIZ, clipboard_menu)))
    } else {
        None
    };

    let mut parts = leading;
    parts.push(boxed(search_field(
        field,
        focused,
        placeholder,
        (width - used).max(0.0),
    )));
    if let Some(trailing) = trailing {
        parts.push(trailing);
    }
    Rectangle::new()
        .width(view.width())
        .height(PILL_H)
        .fill(Color::TRANSPARENT)
        .padding(pad(ROW_INSET, 0.0))
        .align_child(Start, Center)
        .child(Row::new(parts).gap(gap).align(Center))
}

/// The mode's glyph, the magnifier on the pill. In a mode it's a back
/// button: a chevron on a soft square under the pointer, as Spotlight's.
fn leading_glyph(mode: Mode, hovered: bool) -> Rectangle {
    let label = material::label(Level::Secondary);
    if mode == Mode::Home {
        return slot(LEADING).child(icon(ICON_SEARCH, GLYPH, label));
    }
    let (glyph, fill) = if hovered {
        (ICON_CHEVRON_LEFT, material::fill_role(FillLevel::Secondary))
    } else {
        (mode.glyph(), Color::TRANSPARENT)
    };
    slot(LEADING)
        .radius(sys::radius::raised())
        .fill(fill)
        .on_hover(|inside| {
            if with(|state| std::mem::replace(&mut state.back_hover, inside)) != inside {
                super::Launcher::changed();
            }
        })
        .on_click(|_| {
            back_to_pill();
            super::Launcher::changed();
        })
        .child(icon(glyph, GLYPH, label))
}

/// The back button: out of the scope, else out of the mode.
fn back_to_pill() {
    let scoped = with(|state| {
        state.back_hover = false;
        state.scope.is_some()
    });
    if scoped {
        with(|state| state.field = Field::new());
        back();
    } else {
        switch_mode(Mode::Home);
    }
}

/// The scope the search is narrowed to, as a token before the field.
fn token(name: &str) -> Rectangle {
    let style = sys::r#type::label();
    Rectangle::new()
        .width(token_width(name))
        .height(KEYCAP + 4.0)
        .radius(sys::radius::raised())
        .fill(with_alpha(sys::color::accent::bg(), 0.18))
        .align_child(Center, Center)
        .child(Text::new(name).style(style).color(sys::color::accent::bg()))
}

fn token_width(name: &str) -> f32 {
    text_width(name, sys::r#type::label()) + sys::space::inset::s() * 2.0
}

/// A key, drawn as a small cap: the keys that pick a mode.
pub(super) fn keycap(key: &str) -> Rectangle {
    let style = sys::r#type::caption();
    Rectangle::new()
        .width(keycap_width(key))
        .height(KEYCAP)
        .radius(sys::radius::raised() / 2.0)
        .fill(material::fill_role(FillLevel::Secondary))
        .align_child(Center, Center)
        .child(
            Text::new(key)
                .style(style)
                .color(material::label(Level::Secondary)),
        )
}

pub(super) fn keycap_width(key: &str) -> f32 {
    (text_width(key, sys::r#type::caption()) + sys::space::inset::s() * 2.0).max(KEYCAP)
}

/// A round button ending the row.
fn round_button(glyph: char, on_click: fn()) -> Rectangle {
    slot(LEADING)
        .radius(LEADING / 2.0)
        .fill(material::fill_role(FillLevel::Secondary))
        .on_click(move |_| on_click())
        .child(icon(glyph, GLYPH * 0.75, material::label(Level::Secondary)))
}

/// Applications' ⋯ menu: the apps as a grid or a list.
fn more_menu() {
    let picked = amane_kit::settings::launcher().apps_layout;
    let item = |name: &str, layout: AppsLayout| {
        let mut item = MenuItem::label(name).on_select(move || {
            change_launcher(|settings| settings.apps_layout = layout);
            super::Launcher::changed();
        });
        if picked == layout {
            item = item.icon(Icon::glyph_with_font(
                ICON_CHECK.to_string(),
                sys::r#type::icon(),
            ));
        }
        item
    };
    let menu = ContextMenu::new()
        .item(MenuItem::label("View Content As").disabled())
        .item(item("Grid", AppsLayout::Grid))
        .item(item("List", AppsLayout::List));
    context_menu::show_under(MORE_MENU_KEY, menu, LAUNCHER_NS, cursor());
}

/// Clipboard's ⋯ menu: forgetting every copy, or turning the history off.
fn clipboard_menu() {
    let menu = ContextMenu::new()
        .item(
            MenuItem::label("Clear History")
                .destructive()
                .on_select(|| {
                    clipboard_history::clear();
                }),
        )
        .item(MenuItem::label("Turn Off Clipboard History").on_select(|| {
            change_launcher(|settings| settings.clipboard.enabled = false);
            super::Launcher::changed();
        }));
    context_menu::show_under(MORE_MENU_KEY, menu, LAUNCHER_NS, cursor());
}

/// The query as runs of text with the caret between them and the
/// selection washed behind its run, or `placeholder` while it's empty.
fn search_field(field: &Field, focused: bool, placeholder: &str, width: f32) -> Rectangle {
    let style = search_style();
    let fg = material::label(Level::Primary);
    let run = |text: String| Text::new(text).style(style).color(fg);
    let caret = || {
        Rectangle::new()
            .width(CARET.0)
            .height(CARET.1)
            .radius(CARET.0 / 2.0)
            .fill(if focused {
                sys::color::accent::bg()
            } else {
                Color::TRANSPARENT
            })
    };

    let mut parts: Vec<Box<dyn Widget>> = Vec::new();
    if field.text().is_empty() {
        parts.push(boxed(caret()));
        parts.push(boxed(
            Text::new(placeholder)
                .style(style)
                .color(material::label(Level::Tertiary)),
        ));
    } else {
        let len = field.len();
        let caret_at = field.caret();
        let (from, to) = field.selection();
        if from > 0 {
            parts.push(boxed(run(field.slice(0, from))));
        }
        if caret_at <= from {
            parts.push(boxed(caret()));
        }
        if to > from {
            let selected = field.slice(from, to);
            parts.push(boxed(
                Rectangle::new()
                    .width(text_width(&selected, style) + sys::space::gap::_2xs())
                    .height(CARET.1)
                    .radius(sys::radius::raised() / 2.0)
                    .fill(sys::color::state::selected())
                    .align_child(Center, Center)
                    .child(run(selected)),
            ));
            if caret_at >= to {
                parts.push(boxed(caret()));
            }
        }
        if to < len {
            parts.push(boxed(run(field.slice(to, len))));
        }
    }

    Rectangle::new()
        .width(width)
        .height(PILL_H)
        .fill(Color::TRANSPARENT)
        .clip()
        .align_child(Start, Center)
        .child(Row::new(parts).gap(sys::space::gap::_2xs()).align(Center))
}

/// The round mode buttons where `geometry` puts them, `picked` the mode
/// showing: they fade and slide in under the pill as it grows.
pub(super) fn mode_buttons(geometry: &Geometry, picked: Mode) -> Vec<Box<dyn Widget>> {
    let shown = geometry.shown();
    let hovered = with(|state| state.hover_mode);
    Mode::PICKED
        .iter()
        .zip(geometry.circles)
        .map(|(&mode, circle)| {
            let lifted = hovered == Some(mode) || picked == mode;
            let glass = Rectangle::new()
                .width(CIRCLE)
                .height(CIRCLE)
                .radius(CIRCLE / 2.0)
                .fill(Material::Control.fill())
                .align_child(Center, Center)
                .on_hover(move |inside| {
                    let changed = with(|state| {
                        let before = state.hover_mode;
                        if inside {
                            state.hover_mode = Some(mode);
                        } else if before == Some(mode) {
                            state.hover_mode = None;
                        }
                        before != state.hover_mode
                    });
                    if changed {
                        super::Launcher::changed();
                    }
                })
                .on_click(move |_| {
                    switch_mode(mode);
                    super::Launcher::changed();
                })
                .child(icon(
                    mode.glyph(),
                    MODE_GLYPH,
                    material::label(Level::Primary),
                ));
            let slide = (1.0 - shown) * motion::travel(MODE_SLIDE);
            boxed(
                Material::Control
                    .lit(glass, CIRCLE / 2.0)
                    .opacity(shown)
                    .scale(if lifted {
                        MODE_LIFT
                    } else {
                        f32::blend(0.85, 1.0, shown)
                    })
                    .translate(circle.x - slide, circle.y),
            )
        })
        .collect()
}

/// The pills worth showing (`buckets`, indices into the categories), the
/// fill sliding under them to `offset` (a page, between two while they
/// turn), in a bar `width` wide. `focused` rings it while the arrow keys
/// pick the category.
pub(super) fn pill_bar(buckets: &[usize], offset: f32, focused: bool, width: f32) -> Rectangle {
    let gap = sys::space::gap::s();
    let height = sys::size::control::height();
    let widths: Vec<f32> = buckets
        .iter()
        .map(|&bucket| pill_width(model::CATEGORIES[bucket]))
        .collect();
    let lefts: Vec<f32> = widths
        .iter()
        .scan(0.0, |x, width| {
            let left = *x;
            *x += width + gap;
            Some(left)
        })
        .collect();
    let total = lefts.last().zip(widths.last()).map_or(0.0, |(x, w)| x + w);

    // the fill, between the two pills either side of `offset`
    let last = buckets.len().saturating_sub(1);
    let below = (offset.floor().max(0.0) as usize).min(last);
    let above = (below + 1).min(last);
    let along = (offset - below as f32).clamp(0.0, 1.0);
    let blend = |values: &[f32]| match (values.get(below), values.get(above)) {
        (Some(&from), Some(&to)) => f32::blend(from, to, along),
        _ => 0.0,
    };
    let (fill_x, fill_w) = (blend(&lefts), blend(&widths));

    let scroll = PILL_SCROLL.with_borrow_mut(|scroll| {
        scroll.set_max(total - width);
        scroll.settle_stale();
        // the pill picked stays in view
        let picked = (offset.round().max(0.0) as usize).min(last);
        let moved_on = with(|state| state.pills_for.replace(picked)) != Some(picked);
        if moved_on && !scroll.swiping() && !buckets.is_empty() {
            let margin = gap * 2.0;
            let (left, right) = (
                lefts[picked] - margin,
                lefts[picked] + widths[picked] + margin,
            );
            let at = scroll.target();
            if left < at {
                scroll.go(left);
            } else if right > at + width {
                scroll.go(right - width);
            }
        }
        scroll.value()
    });

    let backs: Vec<Box<dyn Widget>> = buckets
        .iter()
        .enumerate()
        .map(|(page, _)| {
            boxed(
                Rectangle::new()
                    .width(widths[page])
                    .height(height)
                    .radius(sys::radius::raised())
                    .fill(material::fill_role(FillLevel::Secondary))
                    .on_click(move |_| turn_to(page)),
            )
        })
        .collect();
    let names: Vec<Box<dyn Widget>> = buckets
        .iter()
        .enumerate()
        .map(|(page, &bucket)| {
            // the accent's colour, as much as the fill is under it
            let near = (1.0 - (page as f32 - offset).abs()).clamp(0.0, 1.0);
            let fg = Color::blend(
                material::label(Level::Primary),
                sys::color::accent::fg(),
                near,
            );
            boxed(
                Rectangle::new()
                    .width(widths[page])
                    .height(height)
                    .fill(Color::TRANSPARENT)
                    .align_child(Center, Center)
                    .child(
                        Text::new(model::CATEGORIES[bucket])
                            .style(sys::r#type::caption())
                            .color(fg),
                    ),
            )
        })
        .collect();

    let mut layers: Vec<Box<dyn Widget>> = vec![boxed(Row::new(backs).gap(gap))];
    if !buckets.is_empty() {
        layers.push(boxed(
            Rectangle::new()
                .width(fill_w)
                .height(height)
                .radius(sys::radius::raised())
                .fill(sys::color::accent::bg())
                .translate(fill_x, 0.0),
        ));
        if focused {
            let out = RING + RING_GAP;
            layers.push(boxed(
                Rectangle::new()
                    .width(fill_w + out * 2.0)
                    .height(height + out * 2.0)
                    .radius(sys::radius::raised() + out)
                    .fill(Color::TRANSPARENT)
                    .border(RING, with_alpha(sys::color::accent::bg(), RING_ALPHA))
                    .translate(fill_x - out, -out),
            ));
        }
    }
    layers.push(boxed(Row::new(names).gap(gap)));

    ScrollView::new(Axis::Horizontal, width, PILLS)
        .content(total)
        .offset(scroll)
        .on_scroll(on_pill_scroll)
        // as tall as a pill, so the focus ring round one doesn't move them
        .build(Stack::new(layers).height(height))
}

fn pill_width(name: &str) -> f32 {
    text_width(name, sys::r#type::caption()) + sys::space::inset::m() * 2.0
}
