//! Files: what file search finds, by kind, as results.

use super::*;

/// The files `query` finds, of `kind` and under `folder`, in the order
/// found.
pub(super) fn files(
    query: &Query,
    kind: Option<FileKind>,
    folder: Option<PathBuf>,
    out: &mut Vec<Candidate>,
) {
    let request = Request {
        text: query.text.to_string(),
        kind,
        folder,
    };
    let (hits, _) = file_search::find(&request);
    let now = emaki_kit::time::unix_s();
    for hit in hits {
        let mut entry = file_entry(hit, now);
        if query.text.is_empty() && kind.is_none() {
            entry.section = Some(String::from(RECENTS));
        }
        out.push(Candidate {
            entry,
            matched: 1.0,
            group: Group::Listed,
        });
    }
}

/// The kinds a search can be narrowed to, named like `name`: "/" and a
/// kind's name.
pub(super) fn kinds(name: &str, out: &mut Vec<Candidate>) {
    for kind in FileKind::ALL {
        if rank::fuzzy(name, kind.label()).is_none() && !name.is_empty() {
            continue;
        }
        out.push(Candidate {
            entry: Entry::new(
                format!("kind:{}", kind.label()),
                kind.label(),
                Icon::Glyph(ICON_DESCRIPTION),
                Action::Ask(Scope::Kind(kind)),
            )
            .subtitle("Kind"),
            matched: 1.0,
            group: Group::Listed,
        });
    }
}

/// A file's row: its kind's glyph (or the image itself, small), its name,
/// what it is, how big and where, its whole path under Super, and how long
/// ago it changed. A folder narrows the search to it with Tab.
pub(super) fn file_entry(hit: Hit, now: u64) -> Entry {
    let icon = if hit.dir {
        Icon::Glyph(ICON_FOLDER)
    } else if shows_as_image(&hit.path) {
        Icon::Image(hit.path.clone())
    } else {
        Icon::Glyph(ICON_DESCRIPTION)
    };
    let mut line = vec![kind_name(&hit)];
    if !hit.dir {
        line.push(emaki_kit::format::bytes(hit.size));
    }
    if let Some(parent) = hit
        .path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
    {
        line.push(format!("▸ {parent}"));
    }
    let mut entry = Entry::new(
        format!("file:{}", hit.path.display()),
        hit.name.clone(),
        icon,
        Action::OpenFile(hit.path.clone()),
    )
    .subtitle(line.join(" · "))
    .detail(hit.path.display().to_string());
    entry.trailing = Some(emaki_kit::format::ago(std::time::Duration::from_secs(
        now.saturating_sub(hit.modified),
    )));
    entry.preview = Some(Preview::File(hit.path.clone()));
    if hit.dir {
        entry = entry.scope(Scope::Folder(hit.path));
    }
    entry
}

/// Whether amane can draw the file at `path` as an image.
pub(in crate::surfaces::launcher) fn shows_as_image(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "svg"))
}

/// What a file is, in words: "PDF document", "Folder", "MD file".
pub(super) fn kind_name(hit: &Hit) -> String {
    if hit.dir {
        return String::from("Folder");
    }
    match FileKind::of(&hit.name) {
        Some(FileKind::Pdf) => String::from("PDF document"),
        Some(FileKind::Image) => String::from("Image"),
        Some(FileKind::Document) => String::from("Document"),
        Some(FileKind::Spreadsheet) => String::from("Spreadsheet"),
        Some(FileKind::Presentation) => String::from("Presentation"),
        Some(FileKind::Audio) => String::from("Audio"),
        Some(FileKind::Video) => String::from("Movie"),
        Some(FileKind::Archive) => String::from("Archive"),
        Some(FileKind::Folder) | None => match hit.name.rsplit_once('.') {
            Some((_, ext)) if !ext.is_empty() => format!("{} file", ext.to_uppercase()),
            _ => String::from("File"),
        },
    }
}

impl Provider for Files {
    fn id(&self) -> &'static str {
        "files"
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Home, Mode::Files]
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        if query.mode == Mode::Home {
            if query.needle.chars().count() < FILES_FROM_PILL_CHARS {
                return;
            }
            let request = Request {
                text: query.text.to_string(),
                ..Request::default()
            };
            let (hits, _) = file_search::find(&request);
            let now = emaki_kit::time::unix_s();
            let mut matched: Vec<(f32, Hit)> = hits
                .into_iter()
                .filter_map(|hit| Some((rank::fuzzy(&query.needle, &hit.name)?, hit)))
                .collect();
            matched.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (score, hit) in matched.into_iter().take(FILES_FROM_PILL) {
                out.push(Candidate {
                    entry: file_entry(hit, now),
                    matched: score * FILE_WEIGHT,
                    group: Group::Ranked,
                });
            }
            return;
        }
        if let Some(kind) = query.needle.strip_prefix('/') {
            kinds(kind.trim(), out);
            return;
        }
        files(query, None, None, out);
    }
}
