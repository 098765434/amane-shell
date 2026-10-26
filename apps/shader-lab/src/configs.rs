//! Saved looks: one `<name>.json` each in the configs folder, which is
//! `apps/shader-lab/configs` when the lab reads a source tree, so they go
//! into git and kit embeds them for the shell (`amane_kit::lab::look`).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use amane::Service;
use amane_kit::lab::config::{Config, slug};

use crate::effects;

pub struct Saved {
    /// The file's name without `.json`, which the kit loader takes.
    pub slug: String,
    pub config: Result<Config, String>,
    modified: Option<SystemTime>,
}

impl Saved {
    pub fn title(&self) -> String {
        match &self.config {
            Ok(config) if !config.name.is_empty() => config.name.clone(),
            Ok(_) => self.slug.clone(),
            Err(_) => format!("{} (broken)", self.slug),
        }
    }
}

/// The saved looks, read again when the folder changes.
pub struct Configs {
    pub list: Vec<Saved>,
}

impl Service for Configs {
    fn new() -> Self {
        Self { list: scan() }
    }

    fn interval() -> Duration {
        Duration::from_millis(700)
    }

    fn update(&mut self) -> bool {
        let stamps = |list: &[Saved]| -> Vec<(String, Option<SystemTime>)> {
            list.iter()
                .map(|saved| (saved.slug.clone(), saved.modified))
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

/// `--configs <dir>`; beside the effects when they're a source tree;
/// otherwise in `~/.config`, where the kit loader doesn't see them.
pub fn folder() -> &'static Path {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();

    FOLDER.get_or_init(|| {
        if let Some(dir) = effects::flag("--configs") {
            return PathBuf::from(dir);
        }

        if effects::from_source()
            && let Some(parent) = effects::folder().parent()
        {
            return parent.join("configs");
        }

        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .unwrap_or_else(std::env::temp_dir);

        config.join("amane-shader-lab").join("configs")
    })
}

/// Whether the shell's kit loader will see what's saved here.
pub fn reaches_the_shell() -> bool {
    effects::from_source() && effects::flag("--configs").is_none()
}

/// Writes the config as `<slug of its name>.json` and says which slug that was.
pub fn save(config: &Config) -> Result<String, String> {
    let slug = slug(&config.name);
    let path = folder().join(format!("{slug}.json"));

    let text = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;

    fs::create_dir_all(folder()).map_err(|error| error.to_string())?;

    // written whole and moved into place, so a reader never sees half of it
    let partial = path.with_extension("json.partial");

    fs::write(&partial, format!("{text}\n")).map_err(|error| error.to_string())?;
    fs::rename(&partial, &path).map_err(|error| error.to_string())?;

    Ok(slug)
}

pub fn delete(slug: &str) -> Result<(), String> {
    fs::remove_file(folder().join(format!("{slug}.json"))).map_err(|error| error.to_string())
}

/// Saves under the new name and removes the old file, unless the name makes the same file.
pub fn rename(old: &str, config: &Config) -> Result<String, String> {
    let new = save(config)?;

    if new != old {
        delete(old)?;
    }

    Ok(new)
}

fn scan() -> Vec<Saved> {
    let Ok(read) = fs::read_dir(folder()) else {
        return Vec::new();
    };

    let mut saved: Vec<Saved> = read
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .map(|path| {
            let slug = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();

            let modified = fs::metadata(&path).and_then(|meta| meta.modified()).ok();

            let config = fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()));

            Saved {
                slug,
                config,
                modified,
            }
        })
        .collect();

    saved.sort_by_key(|saved| saved.title().to_lowercase());

    saved
}
