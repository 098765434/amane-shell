//! Copying and moving files, folders whole, a chunk at a time so the
//! progress shows and a cancel lands between chunks; and the names a file
//! takes when another is already there ("photo 2.jpg").

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// How much is copied between two looks at the cancel flag.
const CHUNK: usize = 256 * 1024;

/// How many bytes `path` holds, folders counted whole; links count nothing.
pub fn size_of(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_dir() {
        fs::read_dir(path)
            .map(|entries| entries.flatten().map(|entry| size_of(&entry.path())).sum())
            .unwrap_or(0)
    } else if meta.is_file() {
        meta.len()
    } else {
        0
    }
}

/// The names of `files` already taken in `folder`.
pub fn conflicts(files: &[PathBuf], folder: &Path) -> Vec<String> {
    files
        .iter()
        .filter_map(|file| file.file_name())
        .filter(|name| folder.join(name).exists())
        .map(|name| name.to_string_lossy().into_owned())
        .collect()
}

/// A name for `name` in `folder` that's free: itself, or "name 2.ext",
/// "name 3.ext" and on, as a file manager keeps both.
pub fn free_name(folder: &Path, name: &str) -> PathBuf {
    let first = folder.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, extension) = split(name);
    (2..)
        .map(|n| folder.join(format!("{stem} {n}{extension}")))
        .find(|path| !path.exists())
        .unwrap_or(first)
}

/// "photo.jpg" as ("photo", ".jpg"); a dotfile or a folder has no extension.
fn split(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    }
}

/// Where `file` lands in `folder`: its own name, or a free one when
/// `keep_both` and the name is taken. Replacing removes what's there first.
pub fn target(file: &Path, folder: &Path, keep_both: bool) -> io::Result<PathBuf> {
    let name = file
        .file_name()
        .ok_or_else(|| io::Error::other("a file without a name"))?
        .to_string_lossy()
        .into_owned();
    let to = folder.join(&name);
    if !to.exists() && fs::symlink_metadata(&to).is_err() {
        return Ok(to);
    }
    if keep_both {
        return Ok(free_name(folder, &name));
    }
    if same(file, &to) {
        return Err(io::Error::other(format!("{name} is already there")));
    }
    remove(&to)?;
    Ok(to)
}

/// Whether two paths are the one file.
fn same(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn remove(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path)?.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Copies `from` to `to`, a folder with everything in it, telling `done`
/// each chunk's size; stops with `Interrupted` once `cancel` is set.
pub fn copy(
    from: &Path,
    to: &Path,
    done: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(io::ErrorKind::Interrupted.into());
    }
    let meta = fs::symlink_metadata(from)?;
    if meta.file_type().is_symlink() {
        let link = fs::read_link(from)?;
        return std::os::unix::fs::symlink(link, to);
    }
    if meta.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy(&entry.path(), &to.join(entry.file_name()), done, cancel)?;
        }
        fs::set_permissions(to, meta.permissions())?;
        return Ok(());
    }
    let result = copy_file(from, to, done, cancel);
    if result.is_err() {
        // half a file is worse than none
        let _ = fs::remove_file(to);
    }
    result?;
    fs::set_permissions(to, meta.permissions())?;
    if let Ok(modified) = meta.modified() {
        // the time is a nicety; the copy is whole without it
        let _ = File::options()
            .write(true)
            .open(to)
            .and_then(|file| file.set_modified(modified));
    }
    Ok(())
}

fn copy_file(
    from: &Path,
    to: &Path,
    done: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> io::Result<()> {
    let mut reader = File::open(from)?;
    let mut writer = File::create(to)?;
    let mut buffer = vec![0; CHUNK];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        done(read as u64);
    }
    writer.sync_all()
}

/// Moves `from` to `to`: a rename on the same disk, else a copy and then
/// the original removed. A move that's cancelled halfway leaves the
/// original whole.
pub fn move_to(
    from: &Path,
    to: &Path,
    done: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => {
            done(size_of(to));
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
            if let Err(error) = copy(from, to, done, cancel) {
                // what was copied goes; the copy's error is the one reported
                let _ = remove(to);
                return Err(error);
            }
            remove(from)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("emaki-drop-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        let folder = scratch("names");
        assert_eq!(free_name(&folder, "a.txt"), folder.join("a.txt"));
        fs::write(folder.join("a.txt"), "1").unwrap();
        fs::write(folder.join("a 2.txt"), "2").unwrap();
        assert_eq!(free_name(&folder, "a.txt"), folder.join("a 3.txt"));
        fs::write(folder.join(".bashrc"), "").unwrap();
        assert_eq!(free_name(&folder, ".bashrc"), folder.join(".bashrc 2"));
        fs::create_dir(folder.join("Photos")).unwrap();
        assert_eq!(free_name(&folder, "Photos"), folder.join("Photos 2"));
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn folders_copy_whole_and_count_their_bytes() {
        let folder = scratch("copy");
        let from = folder.join("from");
        fs::create_dir_all(from.join("inner")).unwrap();
        fs::write(from.join("a"), vec![1u8; 1000]).unwrap();
        fs::write(from.join("inner/b"), vec![2u8; 300_000]).unwrap();
        assert_eq!(size_of(&from), 301_000);

        let mut total = 0;
        copy(
            &from,
            &folder.join("to"),
            &mut |n| total += n,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(total, 301_000);
        assert_eq!(fs::read(folder.join("to/inner/b")).unwrap().len(), 300_000);
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn a_cancelled_copy_leaves_no_half_file() {
        let folder = scratch("cancel");
        fs::write(folder.join("big"), vec![0u8; 10]).unwrap();
        let result = copy(
            &folder.join("big"),
            &folder.join("copy"),
            &mut |_| {},
            &AtomicBool::new(true),
        );
        assert!(result.is_err());
        assert!(!folder.join("copy").exists());
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn replacing_and_keeping_both() {
        let folder = scratch("target");
        let into = folder.join("into");
        fs::create_dir_all(&into).unwrap();
        fs::write(folder.join("a.txt"), "new").unwrap();
        fs::write(into.join("a.txt"), "old").unwrap();
        assert_eq!(conflicts(&[folder.join("a.txt")], &into), ["a.txt"]);
        assert_eq!(
            target(&folder.join("a.txt"), &into, true).unwrap(),
            into.join("a 2.txt")
        );
        assert_eq!(
            target(&folder.join("a.txt"), &into, false).unwrap(),
            into.join("a.txt")
        );
        assert!(!into.join("a.txt").exists());
        // a file never replaces itself
        assert!(target(&folder.join("a.txt"), &folder, false).is_err());
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn moving_on_one_disk_renames() {
        let folder = scratch("move");
        fs::write(folder.join("a"), vec![0u8; 42]).unwrap();
        let mut total = 0;
        move_to(
            &folder.join("a"),
            &folder.join("b"),
            &mut |n| total += n,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(!folder.join("a").exists());
        assert_eq!(total, 42);
        let _ = fs::remove_dir_all(folder);
    }
}
