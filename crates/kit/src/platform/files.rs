//! Writing a file whole: readers see the old contents or the new, never half
//! of either.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// Writes `contents` to `file` through `<file>.part` and a rename, making
/// its folder first. A crash or a full disk leaves the old file as it was.
pub fn write_whole(file: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let partial = part_of(file);
    std::fs::write(&partial, contents)?;
    std::fs::rename(partial, file)
}

/// The part file [`write_whole`] writes `file` through.
pub fn part_of(file: &Path) -> PathBuf {
    let mut name = file.file_name().map(OsString::from).unwrap_or_default();
    name.push(".part");
    file.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_written_whole_and_no_part_file_is_left() {
        let folder = std::env::temp_dir().join(format!("amane-files-{}", std::process::id()));
        let file = folder.join("deep").join("state.json");
        write_whole(&file, "one").unwrap();
        write_whole(&file, "two").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
        assert!(!part_of(&file).exists());
        assert_eq!(part_of(&file).file_name().unwrap(), "state.json.part");
        std::fs::remove_dir_all(folder).unwrap();
    }
}
