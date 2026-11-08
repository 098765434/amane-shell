//! Clipboard: the copies kept, as results.

use super::*;

/// A copy's row: its start or its image, what it is, and how long ago.
pub(super) fn clip_entry(clip: Clip, now: u64) -> Entry {
    let (title, subtitle, icon, preview) = match clip.kind {
        ClipKind::Text => (
            entries::truncate(&clip.preview, CLIP_TITLE),
            format!("Text · {}", emaki_kit::format::bytes(clip.bytes as u64)),
            Icon::Glyph(ICON_CONTENT_COPY),
            Preview::Text(clip.preview.clone()),
        ),
        ClipKind::Image => (
            String::from("Image"),
            format!(
                "{} image · {}",
                if clip.mime == "image/png" {
                    "PNG"
                } else {
                    "JPEG"
                },
                emaki_kit::format::bytes(clip.bytes as u64)
            ),
            Icon::Image(clip.path()),
            Preview::Image(clip.path()),
        ),
        ClipKind::Files => (
            entries::truncate(&clip.preview, CLIP_TITLE),
            String::from("Files"),
            Icon::Glyph(ICON_FOLDER),
            Preview::Text(clip.preview.clone()),
        ),
    };
    let subtitle = match &clip.app {
        Some(app) => format!("{subtitle} · {app}"),
        None => subtitle,
    };
    let mut entry =
        Entry::new(format!("clip:{:016x}", clip.id), title, icon, Action::Hint).subtitle(subtitle);
    entry.trailing = Some(emaki_kit::format::ago(std::time::Duration::from_secs(
        now.saturating_sub(clip.at),
    )));
    entry.preview = Some(preview);
    entry.action = Action::Paste(Box::new(clip));
    entry
}

impl Provider for Clipboard {
    fn id(&self) -> &'static str {
        "clipboard"
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Clipboard]
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        let now = emaki_kit::time::unix_s();
        for clip in clipboard_history::clips() {
            if !query.needle.is_empty() && !clip.preview.to_lowercase().contains(&query.needle) {
                continue;
            }
            out.push(Candidate {
                entry: clip_entry(clip, now),
                matched: 1.0,
                group: Group::Listed,
            });
        }
    }
}
