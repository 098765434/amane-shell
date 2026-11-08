//! Writing a file whole: readers see the old contents or the new, never
//! half of either.

use std::ffi::OsString;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Writes `contents` to `file` through a part file and a rename, making its
/// folder first. A crash or a full disk leaves the old file as it was.
pub fn write_whole(file: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    Whole::new().write(file, contents)
}

/// Writes `value` to `file` whole, as pretty JSON with a closing newline.
pub fn save_json<T: Serialize + ?Sized>(file: &Path, value: &T) -> io::Result<()> {
    let mut text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    text.push('\n');
    write_whole(file, text)
}

/// How [`write_whole`] writes, for the files that need more than its
/// default.
#[derive(Clone, Copy, Debug, Default)]
#[must_use]
pub struct Whole {
    private: bool,
    durable: bool,
}

impl Whole {
    /// Readable by anyone who can read the folder, and on disk when the
    /// system gets to it.
    pub const fn new() -> Self {
        Self {
            private: false,
            durable: false,
        }
    }

    /// Readable and writable by its owner only (`0600`), from the moment it
    /// exists: for secrets.
    pub const fn private(mut self) -> Self {
        self.private = true;
        self
    }

    /// On disk before the rename, and the rename on disk after it: a crash
    /// in between could otherwise leave the file empty (ext4 and btrfs both
    /// may). For a file whose loss costs more than a moment's work.
    pub const fn durable(mut self) -> Self {
        self.durable = true;
        self
    }

    /// Writes `contents` to `file`.
    pub fn write(self, file: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
        let folder = file
            .parent()
            .filter(|folder| !folder.as_os_str().is_empty());
        if let Some(folder) = folder {
            std::fs::create_dir_all(folder)?;
        }
        let part = part_of(file);
        let written = self.write_part(&part, contents.as_ref());
        if written.is_err() {
            // what's left of the part file is no use to anyone
            let _ = std::fs::remove_file(&part);
        }
        written?;
        std::fs::rename(&part, file)?;
        if self.durable
            && let Some(folder) = folder
        {
            // the file is in place; an unsynced folder only risks the rename
            let _ = std::fs::File::open(folder).and_then(|folder| folder.sync_all());
        }
        Ok(())
    }

    fn write_part(self, part: &Path, contents: &[u8]) -> io::Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        if self.private {
            use std::os::unix::fs::OpenOptionsExt as _;
            // a part file left by another run may have other modes
            let _ = std::fs::remove_file(part);
            options.create_new(true).mode(0o600);
        }
        let mut out = options.open(part)?;
        out.write_all(contents)?;
        if self.durable {
            out.sync_all()?;
        }
        Ok(())
    }
}

/// The part file [`write_whole`] writes `file` through: `<file>.part<pid>`,
/// so two programs writing the same file never write the same part.
pub fn part_of(file: &Path) -> PathBuf {
    let mut name = file.file_name().map(OsString::from).unwrap_or_default();
    name.push(format!(".part{}", std::process::id()));
    file.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    fn folder(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("emaki-base-{name}-{}", std::process::id()))
    }

    #[test]
    fn a_file_is_written_whole_and_no_part_file_is_left() {
        let folder = folder("whole");
        let file = folder.join("deep").join("state.json");
        write_whole(&file, "one").unwrap();
        write_whole(&file, "two").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
        assert!(!part_of(&file).exists());
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn private_and_durable_files_are_written_too() {
        let folder = folder("private");
        let file = folder.join("secrets.json");
        Whole::new().private().durable().write(&file, "s").unwrap();
        Whole::new().private().write(&file, "t").unwrap();
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "t");
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn json_is_pretty_and_ends_in_a_newline() {
        let folder = folder("json");
        let file = folder.join("list.json");
        save_json(&file, &[1, 2]).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "[\n  1,\n  2\n]\n");
        std::fs::remove_dir_all(folder).unwrap();
    }
}
