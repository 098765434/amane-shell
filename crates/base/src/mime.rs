//! A file's type from its name, for the programs that send or share one.

use std::path::Path;

/// The media type a file of this name is sent as: `image/png`,
/// `text/plain; charset=utf-8`; `application/octet-stream` for a name it
/// doesn't know.
pub fn of(file: &Path) -> &'static str {
    let extension = file
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "3gp" => "video/3gpp",
        "mp3" => "audio/mpeg",
        "m4a" | "aac" => "audio/aac",
        "ogg" | "opus" => "audio/ogg",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "json" => "application/json",
        "txt" | "md" | "log" => "text/plain; charset=utf-8",
        "html" | "htm" => "text/html; charset=utf-8",
        "vcf" => "text/vcard",
        _ => "application/octet-stream",
    }
}

/// Whether the file is a picture, by its name.
pub fn is_image(file: &Path) -> bool {
    of(file).starts_with("image/")
}

/// Whether the file is a film, by its name.
pub fn is_video(file: &Path) -> bool {
    of(file).starts_with("video/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_by_name() {
        assert_eq!(of(Path::new("a/Photo.JPG")), "image/jpeg");
        assert_eq!(of(Path::new("notes.md")), "text/plain; charset=utf-8");
        assert_eq!(of(Path::new("noname")), "application/octet-stream");
        assert!(is_image(Path::new("x.heic")));
        assert!(is_video(Path::new("x.mov")));
        assert!(!is_image(Path::new("x.pdf")));
    }
}
