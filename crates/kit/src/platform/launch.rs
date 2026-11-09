//! Opening a file, a folder or a link in the app it opens with.

/// Opens `target` (a path or a URL) with `xdg-open`, handing on the
/// activation token so the app that shows it is focused. A failure is
/// logged; there's nothing else to tell the person who pressed.
pub fn open(target: impl AsRef<str>) {
    // a target starting with `-` would be read as one of xdg-open's options
    if target.as_ref().starts_with('-') {
        log::warn!("not opening {:?}: it looks like an option", target.as_ref());
        return;
    }
    let xdg_open = crate::tools::path("xdg-open");
    amane::Launch::argv(&[&xdg_open.to_string_lossy(), target.as_ref()]).activated();
}
