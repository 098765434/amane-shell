//! The effects the lab can show: one folder each, holding an `effect.json`
//! manifest and the WGSL passes it names. The manifest says what each pass
//! reads and which `values` slot each parameter fills, the way a Figma
//! shader's `defineProperties` does, so a Figma shader ports by translating
//! one table.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use amane::serde::Deserialize;
use amane::{Effect, Input, Service};

// the most rows amane hands a pass
const VALUE_ROWS: usize = 64;

// effects built into the binary, written out when no folder is given
const BUILT_IN: &[(&str, &str, &str)] = &[
    (
        "chrome",
        "effect.json",
        include_str!("../effects/chrome/effect.json"),
    ),
    (
        "chrome",
        "blur_x.wgsl",
        include_str!("../effects/chrome/blur_x.wgsl"),
    ),
    (
        "chrome",
        "blur_y.wgsl",
        include_str!("../effects/chrome/blur_y.wgsl"),
    ),
    (
        "chrome",
        "chrome.wgsl",
        include_str!("../effects/chrome/chrome.wgsl"),
    ),
    (
        "spectral",
        "effect.json",
        include_str!("../effects/spectral/effect.json"),
    ),
    (
        "spectral",
        "spectral.wgsl",
        include_str!("../effects/spectral/spectral.wgsl"),
    ),
    (
        "grainy-gradient",
        "effect.json",
        include_str!("../effects/grainy-gradient/effect.json"),
    ),
    (
        "grainy-gradient",
        "grainy.wgsl",
        include_str!("../effects/grainy-gradient/grainy.wgsl"),
    ),
    (
        "rings",
        "effect.json",
        include_str!("../effects/rings/effect.json"),
    ),
    (
        "rings",
        "rings.wgsl",
        include_str!("../effects/rings/rings.wgsl"),
    ),
    (
        "hatching",
        "effect.json",
        include_str!("../effects/hatching/effect.json"),
    ),
    (
        "hatching",
        "hatching.wgsl",
        include_str!("../effects/hatching/hatching.wgsl"),
    ),
    (
        "moving-blobs",
        "effect.json",
        include_str!("../effects/moving-blobs/effect.json"),
    ),
    (
        "moving-blobs",
        "field.wgsl",
        include_str!("../effects/moving-blobs/field.wgsl"),
    ),
    (
        "moving-blobs",
        "composite.wgsl",
        include_str!("../effects/moving-blobs/composite.wgsl"),
    ),
    (
        "moving-blobs",
        "shadow.wgsl",
        include_str!("../effects/moving-blobs/shadow.wgsl"),
    ),
    (
        "chrome-window",
        "effect.json",
        include_str!("../effects/chrome-window/effect.json"),
    ),
    (
        "chrome-wallpaper",
        "effect.json",
        include_str!("../effects/chrome-wallpaper/effect.json"),
    ),
];

#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub margin: f32,
    pub passes: Vec<PassSpec>,
    #[serde(default)]
    pub params: Vec<Param>,
    pub gradient: Option<GradientSpec>,
    pub animate: Option<Animate>,
    /// Values no control changes.
    #[serde(default)]
    pub constants: Vec<Constant>,
    /// Named sets of parameter values, picked from a dropdown.
    #[serde(default)]
    pub presets: Vec<ParamPreset>,
    /// Presses on the subject, handed to the shader.
    pub clicks: Option<Clicks>,
    /// Colours, each filling a row with red, green, blue and alpha from 0 to 1.
    #[serde(default)]
    pub colors: Vec<ColorParam>,
    /// What the lab switches to when the effect is picked.
    #[serde(default)]
    pub suggest: Suggest,
}

#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct ColorParam {
    pub key: String,
    pub label: String,
    /// `#rrggbb` or `#rrggbbaa`.
    pub default: String,
    pub row: usize,
}

#[derive(Deserialize, Clone, Default)]
#[serde(crate = "amane::serde")]
pub struct Suggest {
    /// The index of a subject, as the Subject dropdown lists them.
    pub subject: Option<usize>,
    pub backdrop: Option<usize>,
}

/// `#rrggbb` or `#rrggbbaa` as red, green, blue and alpha from 0 to 1.
pub fn parse_hex(text: &str) -> Option<[f32; 4]> {
    let digits = text.trim().trim_start_matches('#');

    if !matches!(digits.len(), 6 | 8) || !digits.is_ascii() {
        return None;
    }

    let channel = |index: usize| {
        u8::from_str_radix(&digits[index * 2..index * 2 + 2], 16)
            .ok()
            .map(|value| f32::from(value) / 255.0)
    };

    let alpha = if digits.len() == 8 { channel(3)? } else { 1.0 };

    Some([channel(0)?, channel(1)?, channel(2)?, alpha])
}

pub fn to_hex(color: [f32; 4]) -> String {
    let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;

    let [red, green, blue, alpha] = color.map(byte);

    if alpha == 255 {
        format!("#{red:02x}{green:02x}{blue:02x}")
    } else {
        format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
    }
}

/// Each recent press is a row from `rows_from`: x and y from 0 to 1 across
/// the subject, from its top left, and seconds since it; how many there are
/// goes in `count_slot`. A press is let go after `seconds`.
#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct Clicks {
    pub rows_from: usize,
    pub max: usize,
    pub seconds: f32,
    pub count_slot: [usize; 2],
}

#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct ParamPreset {
    pub name: String,
    /// By parameter key; those left out keep their value.
    #[serde(default)]
    pub values: std::collections::HashMap<String, f32>,
    /// By colour key, as `#rrggbb`.
    #[serde(default)]
    pub colors: std::collections::HashMap<String, String>,
}

#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct Constant {
    pub slot: [usize; 2],
    pub value: f32,
}

#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct PassSpec {
    pub shader: String,
    pub inputs: Vec<InputSpec>,
}

/// `"content"`, `"backdrop"`, the index of an earlier pass, or an image
/// file as `{ "image": path, "blur": pixels }`; a path may start with `~/`.
#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde", untagged)]
pub enum InputSpec {
    Pass(usize),
    Image {
        image: String,
        #[serde(default)]
        blur: u32,
    },
    Named(String),
}

impl InputSpec {
    pub fn input(&self) -> Input {
        match self {
            Self::Pass(earlier) => Input::Pass(*earlier),
            Self::Image { image, blur } => Input::blurred_image(home(image), *blur),
            Self::Named(name) if name == "backdrop" => Input::Backdrop,
            Self::Named(_) => Input::Content,
        }
    }

    /// The same input written as Rust.
    pub fn rust(&self) -> String {
        match self {
            Self::Pass(earlier) => format!("Input::Pass({earlier})"),
            Self::Image { image, blur } => {
                format!(
                    "Input::blurred_image({:?}, {blur})",
                    home(image).display().to_string()
                )
            }
            Self::Named(name) if name == "backdrop" => "Input::Backdrop".into(),
            Self::Named(_) => "Input::Content".into(),
        }
    }
}

fn home(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct Param {
    pub key: String,
    pub label: String,
    pub default: f32,
    pub min: f32,
    pub max: f32,
    #[serde(default)]
    pub step: f32,
    #[serde(default)]
    pub unit: String,
    /// The row and lane of `values` it fills.
    pub slot: [usize; 2],
    /// Names for 0, 1, 2…, shown as a dropdown instead of a slider.
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct GradientSpec {
    pub label: String,
    /// Eight rows of stop colors start here.
    pub colors_row: usize,
    /// Two rows of stop positions, four to a row, start here.
    pub positions_row: usize,
    pub count_slot: [usize; 2],
    pub presets: Vec<Preset>,
}

#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct Preset {
    pub name: String,
    /// Position, then red, green, blue and alpha from 0 to 1.
    pub stops: Vec<[f32; 5]>,
}

/// A parameter the Animate switch sweeps, looping every `seconds`.
#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct Animate {
    pub key: String,
    pub from: f32,
    pub to: f32,
    pub seconds: f32,
}

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
            Err(_) => format!("{} (broken)", folder_name(&self.dir)),
        }
    }
}

/// The effect folders, read again when a manifest is saved. The shaders
/// reload in amane itself.
pub struct Effects {
    pub list: Vec<Entry>,
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

/// Where the effect folders are: `--effects <dir>`, so edits to a source
/// tree show at once, or else the built-in ones written into the runtime dir.
pub fn folder() -> &'static Path {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();

    FOLDER.get_or_init(|| {
        let mut args = std::env::args().skip(1);

        while let Some(arg) = args.next() {
            if arg == "--effects"
                && let Some(dir) = args.next()
            {
                return PathBuf::from(dir);
            }
        }

        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("amane-shader-lab");

        for (effect, file, source) in BUILT_IN {
            let path = dir.join(effect).join(file);

            let written =
                fs::create_dir_all(dir.join(effect)).and_then(|()| fs::write(&path, source));

            if let Err(error) = written {
                eprintln!("amane-shader-lab: {}: {error}", path.display());
            }
        }

        dir
    })
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
            let mut modified = Vec::new();

            let manifest = read_manifest(&dir, &mut modified, 0)
                .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
                .and_then(|manifest: Manifest| manifest.check().map(|()| manifest))
                .map_err(|error| format!("{}: {error}", dir.join("effect.json").display()));

            Entry {
                dir,
                manifest,
                modified,
            }
        })
        .collect()
}

/// The manifest as JSON, with what it `extends` filled in: every field it
/// leaves out comes from the folder it names, and its own `params` go before
/// that folder's.
fn read_manifest(
    dir: &Path,
    modified: &mut Vec<Option<SystemTime>>,
    depth: usize,
) -> Result<serde_json::Value, String> {
    let path = dir.join("effect.json");

    modified.push(fs::metadata(&path).and_then(|meta| meta.modified()).ok());

    let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut manifest: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| error.to_string())?;

    let Some(base) = manifest.get("extends").and_then(|base| base.as_str()) else {
        return Ok(manifest);
    };

    if depth >= 4 {
        return Err("\"extends\" goes more than 4 deep".into());
    }

    let mut base = read_manifest(&dir.join(base), modified, depth + 1)
        .map_err(|error| format!("extends {base}: {error}"))?;

    let (Some(own), Some(inherited)) = (manifest.as_object_mut(), base.as_object_mut()) else {
        return Err("a manifest is a JSON object".into());
    };

    let own_params = own.remove("params");
    let inherited_params = inherited.remove("params");

    for (key, value) in std::mem::take(own) {
        if key != "extends" {
            inherited.insert(key, value);
        }
    }

    let mut params: Vec<serde_json::Value> = Vec::new();

    for list in [own_params, inherited_params].into_iter().flatten() {
        if let serde_json::Value::Array(list) = list {
            params.extend(list);
        }
    }

    inherited.insert("params".into(), serde_json::Value::Array(params));

    Ok(base)
}

fn folder_name(dir: &Path) -> String {
    dir.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl Manifest {
    /// What amane would panic on, said as an error instead.
    fn check(&self) -> Result<(), String> {
        if self.passes.is_empty() {
            return Err("an effect needs at least one pass".into());
        }

        for (index, pass) in self.passes.iter().enumerate() {
            if pass.inputs.len() > 4 {
                return Err(format!("pass {index} reads more than 4 inputs"));
            }

            for input in &pass.inputs {
                match input {
                    InputSpec::Pass(earlier) if *earlier >= index => {
                        return Err(format!(
                            "pass {index} reads pass {earlier}, which isn't before it"
                        ));
                    }

                    InputSpec::Named(name) if name != "content" && name != "backdrop" => {
                        return Err(format!(
                            "pass {index} reads \"{name}\": an input is \"content\", \"backdrop\", a pass number or an image"
                        ));
                    }

                    _ => {}
                }
            }
        }

        for color in &self.colors {
            if parse_hex(&color.default).is_none() {
                return Err(format!(
                    "colour {} isn't #rrggbb: {}",
                    color.key, color.default
                ));
            }
        }

        let color_slots = self.colors.iter().map(|color| [color.row, 3]);
        let slots = self
            .params
            .iter()
            .map(|param| param.slot)
            .chain(color_slots);
        let slots: Vec<[usize; 2]> = slots
            .chain(self.constants.iter().map(|constant| constant.slot))
            .collect();

        let mut last_row = slots.iter().map(|slot| slot[0]).max();

        if let Some(gradient) = &self.gradient {
            let rows = [
                gradient.colors_row + 7,
                gradient.positions_row + 1,
                gradient.count_slot[0],
            ];

            last_row = last_row.max(rows.into_iter().max());
        }

        if let Some(clicks) = &self.clicks {
            let rows = [
                clicks.rows_from + clicks.max.max(1) - 1,
                clicks.count_slot[0],
            ];

            last_row = last_row.max(rows.into_iter().max());
        }

        if last_row.is_some_and(|row| row >= VALUE_ROWS) {
            return Err(format!("values only have {VALUE_ROWS} rows"));
        }

        let lanes = slots.iter().map(|slot| slot[1]);

        if lanes
            .chain(self.gradient.iter().map(|g| g.count_slot[1]))
            .any(|lane| lane > 3)
        {
            return Err("a row has 4 lanes, 0 to 3".into());
        }

        Ok(())
    }

    /// The amane effect, with its shader paths under `dir`.
    pub fn effect(&self, dir: &Path, values: Vec<[f32; 4]>) -> Effect {
        let mut effect = Effect::new();

        for pass in &self.passes {
            let inputs: Vec<Input> = pass.inputs.iter().map(InputSpec::input).collect();

            effect = effect.pass(dir.join(&pass.shader), inputs);
        }

        effect.values(values).margin(self.margin)
    }

    /// The `values` rows: each parameter in its slot, the gradient in its rows.
    pub fn pack(&self, value: impl Fn(&Param) -> f32, preset: Option<&Preset>) -> Vec<[f32; 4]> {
        let mut rows = vec![[0.0; 4]; VALUE_ROWS];
        let mut used = 0;

        let mut put = |row: usize, lane: usize, number: f32| {
            rows[row][lane] = number;
            used = used.max(row + 1);
        };

        for constant in &self.constants {
            put(constant.slot[0], constant.slot[1], constant.value);
        }

        for param in &self.params {
            put(param.slot[0], param.slot[1], value(param));
        }

        if let (Some(gradient), Some(preset)) = (&self.gradient, preset) {
            let count = preset.stops.len().min(8);

            for (index, stop) in preset.stops.iter().take(8).enumerate() {
                for lane in 0..4 {
                    put(gradient.colors_row + index, lane, stop[lane + 1]);
                }

                put(gradient.positions_row + index / 4, index % 4, stop[0]);
            }

            put(gradient.count_slot[0], gradient.count_slot[1], count as f32);
        }

        rows.truncate(used);
        rows
    }
}

impl Manifest {
    /// Each colour's row, as `color` says it is now.
    pub fn put_colors(&self, rows: &mut Vec<[f32; 4]>, color: impl Fn(&ColorParam) -> [f32; 4]) {
        for param in &self.colors {
            if rows.len() <= param.row {
                rows.resize(param.row + 1, [0.0; 4]);
            }

            rows[param.row] = color(param);
        }
    }

    /// The presses, newest last, as rows from `clicks.rows_from`, the oldest
    /// left out past `clicks.max`.
    pub fn put_clicks(&self, rows: &mut Vec<[f32; 4]>, presses: &[[f32; 3]]) {
        let Some(clicks) = &self.clicks else {
            return;
        };

        let kept = &presses[presses.len().saturating_sub(clicks.max)..];
        let needed = (clicks.rows_from + kept.len()).max(clicks.count_slot[0] + 1);

        if rows.len() < needed {
            rows.resize(needed, [0.0; 4]);
        }

        for (row, press) in rows[clicks.rows_from..].iter_mut().zip(kept) {
            *row = [press[0], press[1], press[2], 0.0];
        }

        rows[clicks.count_slot[0]][clicks.count_slot[1]] = kept.len() as f32;
    }
}

impl Param {
    /// Snapped to its step and kept in its range.
    pub fn snap(&self, value: f32) -> f32 {
        let snapped = if self.step > 0.0 {
            self.min + ((value - self.min) / self.step).round() * self.step
        } else {
            value
        };

        snapped.clamp(self.min, self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chrome() -> Manifest {
        serde_json::from_str(include_str!("../effects/chrome/effect.json")).unwrap()
    }

    #[test]
    fn the_chrome_manifest_is_sound() {
        chrome().check().unwrap();
    }

    #[test]
    fn packs_params_and_the_gradient_into_their_slots() {
        let manifest = chrome();
        let preset = &manifest.gradient.as_ref().unwrap().presets[0];
        let rows = manifest.pack(|param| param.default, Some(preset));

        assert_eq!(rows.len(), 14);
        // rounding, read by both blurs
        assert_eq!(rows[0][0], 7.5);
        // the first stop's color, and the last stop's position
        assert_eq!(rows[1], [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(rows[10][2], 0.70);
        // count, depth, rgb split, scale
        assert_eq!(rows[11], [7.0, 100.0, 10.0, 50.0]);
        // phase, evolution, angle, stretch
        assert_eq!(rows[12], [0.0, 0.0, 30.0, 200.0]);
        // offset, repeats, roughness
        assert_eq!(rows[13], [0.0, 2.0, 0.0, 0.0]);
    }

    #[test]
    fn snaps_to_the_step_inside_the_range() {
        let rounding = chrome().params.remove(0);

        assert_eq!(rounding.snap(7.3), 7.5);
        assert_eq!(rounding.snap(12.0), 10.0);
    }

    #[test]
    fn a_variant_takes_what_it_leaves_out_from_what_it_extends() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("effects/chrome-wallpaper");
        let mut modified = Vec::new();

        let value = read_manifest(&dir, &mut modified, 0).unwrap();
        let manifest: Manifest = serde_json::from_value(value).unwrap();

        manifest.check().unwrap();
        assert_eq!(manifest.name, "Reflective chrome (wallpaper)");
        assert_eq!(modified.len(), 2);
        // its own params first, then chrome's
        assert_eq!(manifest.params[0].key, "reflect");
        assert_eq!(manifest.params.last().unwrap().key, "evolution");
        assert!(manifest.gradient.is_some());
        assert!(matches!(
            manifest.passes[2].inputs[2].input(),
            Input::Image { blur: 4, .. }
        ));

        let rows = manifest.pack(|param| param.default, None);
        assert_eq!(rows[14], [70.0, 24.0, 1.0, 2.0]);
    }

    #[test]
    fn the_spectral_presets_name_real_params() {
        let manifest: Manifest =
            serde_json::from_str(include_str!("../effects/spectral/effect.json")).unwrap();

        manifest.check().unwrap();

        for preset in &manifest.presets {
            for key in preset.values.keys() {
                assert!(
                    manifest.params.iter().any(|param| &param.key == key),
                    "{} sets {key}, which isn't a param",
                    preset.name
                );
            }
        }
    }

    #[test]
    fn presses_fill_their_rows_and_count() {
        let manifest: Manifest =
            serde_json::from_str(include_str!("../effects/grainy-gradient/effect.json")).unwrap();

        manifest.check().unwrap();

        let mut rows = manifest.pack(|param| param.default, None);
        let presses: Vec<[f32; 3]> = (0..12).map(|index| [0.5, 0.25, index as f32]).collect();

        manifest.put_clicks(&mut rows, &presses);

        // only the newest ten, the count beside the wave scales
        assert_eq!(rows.len(), 24);
        assert_eq!(rows[1][3], 10.0);
        assert_eq!(rows[14], [0.5, 0.25, 2.0, 0.0]);
        assert_eq!(rows[23], [0.5, 0.25, 11.0, 0.0]);
    }

    #[test]
    fn every_built_in_manifest_is_sound() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("effects");

        for (effect, file, _) in BUILT_IN {
            if *file != "effect.json" {
                continue;
            }

            let mut modified = Vec::new();
            let value = read_manifest(&dir.join(effect), &mut modified, 0).unwrap();
            let manifest: Manifest = serde_json::from_value(value).unwrap();

            manifest
                .check()
                .unwrap_or_else(|error| panic!("{effect}: {error}"));

            for preset in &manifest.presets {
                for key in preset.values.keys() {
                    assert!(
                        manifest.params.iter().any(|param| &param.key == key),
                        "{effect}: {} sets {key}, which isn't a param",
                        preset.name
                    );
                }
            }
        }
    }

    #[test]
    fn hex_colours_read_and_write() {
        assert_eq!(parse_hex("#ff0080"), Some([1.0, 0.0, 128.0 / 255.0, 1.0]));
        assert_eq!(parse_hex("00000000"), Some([0.0; 4]));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(to_hex([1.0, 0.0, 128.0 / 255.0, 1.0]), "#ff0080");
        assert_eq!(to_hex([0.0, 0.0, 0.0, 0.0]), "#00000000");
    }

    #[test]
    fn a_pass_reading_a_later_pass_is_an_error() {
        let mut manifest = chrome();
        manifest.passes[0].inputs = vec![InputSpec::Pass(2)];

        assert!(manifest.check().is_err());
    }
}
