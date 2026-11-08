//! The Drop Bar under the tiles: files held for later, a chip a stack.

use super::*;

/// The files held, a stack a drop: drag one out to another program or onto
/// a tile; a right-click lets it go.
pub(super) fn shelf(grid: &str, width: f32) -> Rectangle {
    let stacks = state::stacks(grid);
    let hovered = state::ui().hover == Some(Target::Bar);
    let secondary = material::label(Level::Secondary);
    let accent = sys::color::accent::bg();

    let content: Box<dyn Widget> = if stacks.is_empty() {
        boxed(label(
            "Drop Bar: drop files here to hold them for later",
            sys::r#type::caption(),
            secondary,
            width - 24.0,
        ))
    } else {
        let mut chips: Vec<Box<dyn Widget>> = stacks
            .into_iter()
            .enumerate()
            .take(((width - 48.0) / 52.0) as usize)
            .map(|(index, files)| boxed(stack_chip(grid, index, files)))
            .collect();
        let cleared = grid.to_string();
        chips.push(boxed(
            IconButton::new("emaki-drop.clear", ICON_CLOSE)
                .on_press(move || state::let_go(&cleared, None))
                .build(),
        ));
        boxed(Row::new(chips).gap(4.0).align(Center))
    };
    let held = grid.to_string();
    Rectangle::new()
        .width(width)
        .height(SHELF)
        .radius(12.0)
        .fill(if hovered {
            emaki_kit::color::with_alpha(accent, 0.2)
        } else {
            material::fill_role(FillLevel::Tertiary)
        })
        .border(if hovered { 2.0 } else { 0.0 }, accent)
        .padding(Padding {
            left: 8.0,
            right: 8.0,
            ..Padding::default()
        })
        .on_files_over(|at, _| {
            state::hover(Target::Bar, at.is_some());
            true
        })
        .on_drop(move |files| {
            state::hover(Target::Bar, false);
            state::hold(&held, files);
        })
        .align_child(Start, Center)
        .child(Row::new(vec![content]))
}

pub(super) fn stack_chip(grid: &str, index: usize, files: Vec<PathBuf>) -> Rectangle {
    let gone = files.iter().all(|file| !file.exists());
    let first = files.first().cloned().unwrap_or_default();
    let picture: Box<dyn Widget> = if emaki_kit::mime::is_image(&first) && Image::loaded(&first) {
        boxed(
            Rectangle::new()
                .width(Size::Fixed(40.0))
                .height(Size::Fixed(40.0))
                .radius(6.0)
                .fill(Image::cover(first.clone())),
        )
    } else {
        let glyph = if first.is_dir() {
            ICON_FOLDER_FILLED
        } else if emaki_kit::mime::is_image(&first) {
            ICON_IMAGE
        } else {
            ICON_DESCRIPTION
        };
        boxed(icon(glyph, 30.0, sys::color::accent::bg()))
    };
    let count = files.len();
    let mut layers: Vec<Box<dyn Widget>> = vec![boxed(
        Rectangle::new()
            .width(Size::Fixed(48.0))
            .height(Size::Fixed(48.0))
            .align_child(Center, Center)
            .child(Row::new(vec![picture])),
    )];
    if count > 1 {
        layers.push(boxed(
            Rectangle::new()
                .width(Size::Fixed(48.0))
                .height(Size::Fixed(18.0))
                .align_child(amane::End, Start)
                .child(
                    Rectangle::new()
                        .width(Size::Fixed(18.0))
                        .height(Size::Fixed(18.0))
                        .radius(9.0)
                        .fill(sys::color::accent::bg())
                        .align_child(Center, Center)
                        .child(
                            Text::new(count.to_string())
                                .style(sys::r#type::caption())
                                .color(Color::WHITE),
                        ),
                ),
        ));
    }
    let grid = grid.to_string();
    let dragged = files.clone();
    let chip = Rectangle::new()
        .width(Size::Fixed(48.0))
        .height(Size::Fixed(48.0))
        .drag_out(move || {
            DragData::Files(
                dragged
                    .iter()
                    .filter(|file| file.exists())
                    .cloned()
                    .collect(),
            )
        })
        .on_click(move |button| {
            if button == Button::Right {
                state::let_go(&grid, Some(index));
            }
        })
        .child(Stack::new(layers));
    if gone { chip.opacity(0.4) } else { chip }
}
