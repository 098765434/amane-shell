//! The preview beside the results: a file's, a copy's or a password's, and the ask to turn the
//! clipboard history on.

use super::*;
use emaki_kit::layout::frame;

/// The copy picked in Clipboard, `width` by `height` beside the list: its
/// text wrapped, or its image fitted.
pub(crate) fn preview(preview: Option<&Preview>, width: f32, height: f32) -> Rectangle {
    let inset = sys::space::inset::l();
    let card = Rectangle::new()
        .width(width)
        .height(height)
        .radius(sys::radius::popover())
        .fill(material::fill_role(FillLevel::Tertiary))
        .clip();
    match preview {
        Some(Preview::Image(path)) => card.padding(pad(inset, inset)).child(
            Rectangle::new()
                .width(width - inset * 2.0)
                .height(height - inset * 2.0)
                .fill(Image::contain(path.clone())),
        ),
        Some(Preview::Text(text)) => {
            let style = sys::r#type::label();
            let lines = (height - inset * 2.0) / text_height(style);
            let wrapped = wrapped_lines(text, style, width - inset * 2.0, lines.max(1.0) as usize);
            card.padding(pad(inset, inset))
                .align_child(Start, Start)
                .child(Column::new(
                    wrapped
                        .into_iter()
                        .map(|line| {
                            boxed(
                                Text::new(line)
                                    .style(style)
                                    .color(material::label(Level::Primary)),
                            )
                        })
                        .collect(),
                ))
        }
        Some(Preview::File(path)) => {
            let shown = file_preview(path);
            let inner = width - inset * 2.0;
            let centred = matches!(shown, FilePreview::About(_));
            let body: Box<dyn Widget> = match shown {
                FilePreview::Image => boxed(
                    Rectangle::new()
                        .width(inner)
                        .height(height - inset * 2.0)
                        .fill(Image::contain(path.clone())),
                ),
                FilePreview::Lines(lines, glyph) => {
                    let style = sys::r#type::label();
                    let line_h = text_height(style);
                    let fit = ((height - inset * 2.0) / line_h).max(1.0) as usize;
                    let gap = sys::space::gap::s();
                    let rows: Vec<Box<dyn Widget>> = lines
                        .iter()
                        .take(fit)
                        .map(|line| {
                            let text = Text::new(line.clone())
                                .style(style)
                                .color(material::label(Level::Primary))
                                .elide();
                            let mut parts: Vec<Box<dyn Widget>> = Vec::new();
                            let mut text_w = inner;
                            if let Some(glyph) = glyph {
                                parts.push(boxed(icon(
                                    glyph,
                                    line_h,
                                    material::label(Level::Secondary),
                                )));
                                text_w -= line_h + gap;
                            }
                            parts.push(boxed(frame(text_w.max(0.0), line_h).clip().child(text)));
                            boxed(Row::new(parts).gap(gap).align(Center))
                        })
                        .collect();
                    boxed(Column::new(rows))
                }
                FilePreview::About(name) => boxed(
                    Column::new(vec![
                        boxed(icon(
                            ICON_DESCRIPTION,
                            PREVIEW_ICON,
                            material::label(Level::Secondary),
                        )),
                        boxed(
                            Text::new(name)
                                .style(sys::r#type::label())
                                .color(material::label(Level::Primary)),
                        ),
                    ])
                    .gap(sys::space::gap::m())
                    .align(Center),
                ),
            };
            let card = card.padding(pad(inset, inset));
            if centred {
                card.align_child(Center, Center)
                    .child(Stack::new(vec![body]))
            } else {
                card.align_child(Start, Start).child(Stack::new(vec![body]))
            }
        }
        None => card,
    }
}

/// What the preview shows of the file at `path`: an image amane draws, a
/// folder's files, the start of a text, or its name.
pub(super) fn file_preview(path: &Path) -> FilePreview {
    if let Some(known) = PREVIEWED.with_borrow(|seen| {
        seen.as_ref()
            .filter(|(seen, _)| seen == path)
            .map(|(_, preview)| preview.clone())
    }) {
        return known;
    }
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let preview = if provider::shows_as_image(path) {
        FilePreview::Image
    } else if path.is_dir() {
        let mut names: Vec<String> = std::fs::read_dir(path)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .filter(|name| !name.starts_with('.'))
                    .collect()
            })
            .unwrap_or_default();
        names.sort_by_key(|name| name.to_lowercase());
        names.truncate(PREVIEW_ENTRIES);
        FilePreview::Lines(names, Some(ICON_DESCRIPTION))
    } else {
        let mut start = vec![0; PREVIEW_BYTES];
        let read = std::fs::File::open(path)
            .and_then(|mut file| file.read(&mut start))
            .unwrap_or(0);
        start.truncate(read);
        match std::str::from_utf8(&start) {
            // text, and not a binary that happens to start like it
            Ok(text) if read > 0 && !text.contains('\0') => FilePreview::Lines(
                text.lines()
                    .map(|line| line.replace('\t', "    "))
                    .collect(),
                None,
            ),
            _ => FilePreview::About(name),
        }
    };
    PREVIEWED.with_borrow_mut(|seen| *seen = Some((path.to_path_buf(), preview.clone())));
    preview
}

/// Clipboard before its history is turned on, `width` by `height`: what
/// it would keep, and buttons to turn it on or go back.
pub(crate) fn ask_clipboard(width: f32, height: f32) -> Rectangle {
    let style = sys::r#type::label();
    let note = wrapped_lines(ASK_NOTE, style, (width * 0.8).min(ASK_W), 4);
    let mut lines: Vec<Box<dyn Widget>> = note
        .into_iter()
        .map(|line| {
            boxed(
                Text::new(line)
                    .style(style)
                    .color(material::label(Level::Secondary)),
            )
        })
        .collect();
    // a little more room over the buttons than between the lines
    lines.push(boxed(frame(1.0, sys::space::gap::s())));
    lines.push(boxed(
        Row::new(vec![
            boxed(
                PushButton::new("emaki.launcher.clipboard.not-now", "Not Now")
                    .style(Style::Tonal)
                    .on_press(|| {
                        super::super::switch_mode(Mode::Home);
                        super::super::Launcher::changed();
                    })
                    .build(),
            ),
            boxed(
                PushButton::new("emaki.launcher.clipboard.enable", "Enable")
                    .style(Style::Filled)
                    .on_press(|| change_launcher(|settings| settings.clipboard.enabled = true))
                    .build(),
            ),
        ])
        .gap(sys::space::gap::m())
        .align(Center),
    ));
    frame(width, height)
        .align_child(Center, Center)
        .child(Column::new(lines).gap(sys::space::gap::s()).align(Center))
}
