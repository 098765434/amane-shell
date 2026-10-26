//! The effect folders the lab can show, read from `--effects <dir>` or
//! from the copies kit embeds, and read again when a manifest is saved.
//! What a manifest says is kit's (`amane_kit::lab::manifest`), so the shell
//! reads saved looks the same way.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use amane::Service;
use amane_kit::lab::manifest::{Manifest, read_manifest};

pub struct Entry {
    pub dir: PathBuf,
    pub manifest: Result<Manifest, String>,
    /// When its manifest, and any it extends, were last saved.
    modified: Vec<Option<SystemTime>>,
}

impl Entry {
    pub fn title(&self) -> String {
        match &self.manifest {
            Ok(manifest) => manifest.name.clone(),
            Err(_) => format!("{} (broken)", self.folder()),
        }
    }

    /// The folder's name, which configs name the effect by.
    pub fn folder(&self) -> String {
        self.dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// The effect folders. The shaders reload in amane itself.
pub struct Effects {
    pub list: Vec<Entry>,
}

impl Effects {
    pub fn position(&self, folder: &str) -> Option<usize> {
        self.list.iter().position(|entry| entry.folder() == folder)
    }
}

impl Service for Effects {
    fn new() -> Self {
        Self { list: scan() }
    }

    fn interval() -> Duration {
        Duration::from_millis(700)
    }

    fn update(&mut self) -> bool {
        let stamps = |list: &[Entry]| -> Vec<(PathBuf, Vec<Option<SystemTime>>)> {
            list.iter()
                .map(|entry| (entry.dir.clone(), entry.modified.clone()))
                .collect()
        };

        let now = scan();

        if stamps(&now) == stamps(&self.list) {
            return false;
        }

        self.list = now;
        true
    }
}

/// The value after `name` on the command line.
pub fn flag(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }

    None
}

/// Where the effect folders are: `--effects <dir>`, so edits to a source
/// tree show at once, or else kit's embedded ones written into the runtime dir.
pub fn folder() -> &'static Path {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();

    FOLDER.get_or_init(|| {
        if let Some(dir) = flag("--effects") {
            return PathBuf::from(dir);
        }

        let root = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("amane-shader-lab");

        if let Err(error) = amane_kit::lab::write_effects(&root) {
            eprintln!("amane-shader-lab: {}: {error}", root.display());
        }

        root.join("effects")
    })
}

/// Whether the lab is reading a source tree, where saved configs go into git.
pub fn from_source() -> bool {
    flag("--effects").is_some()
}

fn scan() -> Vec<Entry> {
    let Ok(read) = fs::read_dir(folder()) else {
        return Vec::new();
    };

    let mut dirs: Vec<PathBuf> = read
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("effect.json").is_file())
        .collect();

    dirs.sort();

    dirs.into_iter()
        .map(|dir| {
            let modified = std::cell::RefCell::new(Vec::new());

            let read = |path: &Path| -> Result<String, String> {
                let stamp = fs::metadata(path).and_then(|meta| meta.modified()).ok();

                modified.borrow_mut().push(stamp);

                fs::read_to_string(path).map_err(|error| error.to_string())
            };

            let manifest = read_manifest(&dir, &read, 0)
                .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
                .and_then(|manifest: Manifest| manifest.check().map(|()| manifest))
                .map_err(|error| format!("{}: {error}", dir.join("effect.json").display()));

            Entry {
                dir,
                manifest,
                modified: modified.into_inner(),
            }
        })
        .collect()
}
