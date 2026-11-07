//! Opening a file, a folder or a link in the app it opens with.

/// Opens `target` (a path or a URL) with `xdg-open`, handing on the
/// activation token so the app that shows it is focused. A failure is
/// logged; there's nothing else to tell the person who pressed.
pub fn open(target: impl AsRef<str>) {
    amane::Launch::argv(&["xdg-open", target.as_ref()]).activated();
}
