//! The cards among the results: an answer, the scope chips, and what shows when nothing does.

use super::*;

/// The answer to a sum or a conversion, or the assistant's command, in a
/// card over the list, `width` wide; `selected` rings it as what Enter
/// copies or runs.
pub(crate) fn answer_card(answer: &Entry, width: f32, selected: bool, query: &str) -> Rectangle {
    let clicked = answer.clone();
    let query = query.to_string();
    let accent = sys::color::accent::bg();
    let on = sys::color::accent::fg();
    let copies = matches!(answer.action, Action::Copy(_));
    let big = TextStyle {
        size: ANSWER_TEXT,
        ..if copies {
            sys::r#type::numeric()
        } else {
            sys::r#type::title()
        }
    };
    let mut lines: Vec<Box<dyn Widget>> = Vec::new();
    if let Some(asked) = &answer.subtitle {
        let asked = if copies {
            format!("{asked} =")
        } else {
            asked.clone()
        };
        lines.push(boxed(
            Text::new(asked)
                .style(sys::r#type::caption())
                .color(with_alpha(on, 0.8)),
        ));
    }
    lines.push(boxed(
        Text::new(answer.title.clone()).style(big).color(on).elide(),
    ));
    let hint = match &answer.action {
        Action::Copy(_) => "Return to copy",
        Action::Run(_) if answer.key == super::super::assistant::CONFIRMS => "Return to confirm",
        Action::Run(_) => "Return to run",
        _ => "",
    };
    let copy_hint = Text::new(hint)
        .style(sys::r#type::caption())
        .color(with_alpha(on, 0.8));

    let inset = sys::space::inset::l();
    Rectangle::new()
        .width(width)
        .height(ANSWER_H)
        .radius(sys::radius::popover())
        .fill(accent)
        .border(
            if selected { 2.0 } else { 0.0 },
            with_alpha(on, if selected { 0.7 } else { 0.0 }),
        )
        .padding(pad(inset, 0.0))
        .align_child(Start, Center)
        .on_click(move |_| entries::run(&clicked, &query))
        .child(
            Row::new(vec![
                boxed(
                    Rectangle::new()
                        .width(
                            (width - inset * 2.0 - text_width(hint, sys::r#type::caption()))
                                .max(0.0),
                        )
                        .height(ANSWER_H)
                        .fill(Color::TRANSPARENT)
                        .align_child(Start, Center)
                        .child(Column::new(lines).gap(sys::space::gap::_2xs())),
                ),
                boxed(
                    Rectangle::new()
                        .width(text_width(hint, sys::r#type::caption()))
                        .height(ANSWER_H)
                        .fill(Color::TRANSPARENT)
                        .align_child(End, Center)
                        .child(copy_hint),
                ),
            ])
            .align(Center),
        )
}

/// Up to four places to narrow the search to, as chips of one width
/// across a row `width` wide.
pub(crate) fn scope_chips(scopes: &[Scope], width: f32) -> Rectangle {
    let gap = sys::space::gap::s();
    let count = scopes.len().max(4) as f32;
    let chip_w = (width - gap * (count - 1.0)) / count;
    let chips: Vec<Box<dyn Widget>> = scopes
        .iter()
        .map(|scope| {
            let picked = scope.clone();
            boxed(
                Rectangle::new()
                    .width(chip_w)
                    .height(CHIP_H)
                    .radius(sys::radius::raised())
                    .fill(material::fill_role(FillLevel::Secondary))
                    .align_child(Center, Center)
                    .on_click(move |_| {
                        enter_scope(picked.clone());
                        super::super::Launcher::changed();
                    })
                    // elided text fills its box, so the box is only as wide
                    // as the name: the chip centres it
                    .child(
                        Rectangle::new()
                            .width(
                                text_width(scope.name(), sys::r#type::caption())
                                    .min(chip_w - sys::space::inset::m() * 2.0),
                            )
                            .height(CHIP_H)
                            .fill(Color::TRANSPARENT)
                            .align_child(Center, Center)
                            .clip()
                            .child(
                                Text::new(scope.name())
                                    .style(sys::r#type::caption())
                                    .color(material::label(Level::Secondary))
                                    .elide(),
                            ),
                    ),
            )
        })
        .collect();
    Rectangle::new()
        .width(width)
        .height(CHIP_H)
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .child(Row::new(chips).gap(gap))
}

/// What shows where nothing does, `width` by `height`: a glyph over a note
/// saying why.
pub(crate) fn empty(why: Empty, width: f32, height: f32) -> Rectangle {
    let (glyph, note) = match why {
        Empty::Loading => (ICON_SEARCH, "Loading…"),
        Empty::Category => (ICON_APPS, "Nothing in this category"),
        Empty::NothingCopied => (ICON_CONTENT_PASTE, "Nothing copied yet"),
        Empty::NoMatch => (ICON_SEARCH, "No copies match"),
        Empty::NoFiles => (ICON_FOLDER, "No files found"),
    };
    let secondary = material::label(Level::Secondary);
    Rectangle::new()
        .width(width)
        .height(height)
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .child(
            Column::new(vec![
                boxed(icon(glyph, EMPTY_ICON, secondary)),
                boxed(Text::new(note).style(sys::r#type::label()).color(secondary)),
            ])
            .gap(sys::space::gap::m())
            .align(Center),
        )
}
