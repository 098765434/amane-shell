//! An effect folder's manifest, `effect.json`: the passes or rectangle
//! shader it runs, the params, colours and gradient its controls set, and
//! which row and lane of `values` each fills — the way a Figma shader's
//! `defineProperties` does, so a Figma shader ports by translating one
//! table. The Shader Lab edits effects through it, and [`super::look`]
//! builds a saved one for the shell.

use std::path::{Component, Path, PathBuf};

use amane::serde::Deserialize;
use amane::{Effect, Input};

use crate::color_picker::parse_hex;

/// The most rows amane hands a pass.
pub const VALUE_ROWS: usize = 64;

#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub margin: f32,
    #[serde(default)]
    pub passes: Vec<PassSpec>,
    /// A rectangle shader drawn over the subject's fill, in place of passes.
    pub shader: Option<String>,
    /// Draws the subject as a translucent surface for the shader to light.
    pub surface: Option<SurfaceSpec>,
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

/// The subject as a surface like the shell's: filled with the `tint` colour
/// over the backdrop frosted by the `frost` param (a stand-in for niri's
/// blur), with an inner `glow` colour blurred by `glow_blur`, its corners
/// rounded by `radius` on a rounded rectangle. Its rectangle (0, 0, width,
/// height) goes in `rect_row` and its corner radius in `radius_slot`.
#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct SurfaceSpec {
    pub tint: String,
    pub glow: String,
    pub glow_blur: String,
    pub frost: String,
    pub radius: String,
    pub rect_row: usize,
    pub radius_slot: [usize; 2],
}

#[derive(Deserialize, Clone, Default)]
#[serde(crate = "amane::serde")]
pub struct Suggest {
    /// The index of a subject, as the Subject dropdown lists them.
    pub subject: Option<usize>,
    pub backdrop: Option<usize>,
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

pub fn home(path: &str) -> PathBuf {
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

/// The manifest as JSON, with what it `extends` filled in: every field it
/// leaves out comes from the folder it names, and its own `params` go before
/// that folder's.
///
/// `read` gives a file's text by its path: the disk for the lab, the
/// embedded copies for the shell.
pub fn read_manifest(
    dir: &Path,
    read: &dyn Fn(&Path) -> Result<String, String>,
    depth: usize,
) -> Result<serde_json::Value, String> {
    let text = read(&dir.join("effect.json"))?;
    let mut manifest: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| error.to_string())?;

    let Some(base) = manifest.get("extends").and_then(|base| base.as_str()) else {
        return Ok(manifest);
    };

    if depth >= 4 {
        return Err("\"extends\" goes more than 4 deep".into());
    }

    let mut base = read_manifest(&normal(&dir.join(base)), read, depth + 1)
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

impl Manifest {
    /// What amane would panic on, said as an error instead.
    pub fn check(&self) -> Result<(), String> {
        if self.passes.is_empty() == self.shader.is_none() {
            return Err("an effect has passes or a shader, one or the other".into());
        }

        if let Some(surface) = &self.surface {
            let is_param = |key: &String| self.params.iter().any(|param| &param.key == key);
            let is_color = |key: &String| self.colors.iter().any(|color| &color.key == key);

            let params = [&surface.glow_blur, &surface.frost, &surface.radius];

            if let Some(key) = params.into_iter().find(|key| !is_param(key)) {
                return Err(format!("the surface's {key} isn't a param"));
            }

            if let Some(key) = [&surface.tint, &surface.glow]
                .into_iter()
                .find(|key| !is_color(key))
            {
                return Err(format!("the surface's {key} isn't a colour"));
            }
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
    /// None for a manifest with a rectangle shader instead of passes.
    pub fn effect(&self, dir: &Path, values: Vec<[f32; 4]>) -> Option<Effect> {
        if self.passes.is_empty() {
            return None;
        }

        let mut effect = Effect::new();

        for pass in &self.passes {
            let inputs: Vec<Input> = pass.inputs.iter().map(InputSpec::input).collect();

            effect = effect.pass(dir.join(&pass.shader), inputs);
        }

        Some(effect.values(values).margin(self.margin))
    }

    /// The `values` rows: each parameter in its slot, the gradient's stops
    /// (position, red, green, blue, alpha) in their rows.
    pub fn pack(&self, value: impl Fn(&Param) -> f32, stops: Option<&[[f32; 5]]>) -> Vec<[f32; 4]> {
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

        if let (Some(gradient), Some(stops)) = (&self.gradient, stops) {
            let count = stops.len().min(8);

            for (index, stop) in stops.iter().take(8).enumerate() {
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
    /// The surface's rectangle and corner radius, in its rows.
    pub fn put_surface(
        &self,
        rows: &mut Vec<[f32; 4]>,
        surface: &SurfaceSpec,
        width: f32,
        height: f32,
        radius: f32,
    ) {
        let needed = surface.rect_row.max(surface.radius_slot[0]) + 1;

        if rows.len() < needed {
            rows.resize(needed, [0.0; 4]);
        }

        rows[surface.rect_row] = [0.0, 0.0, width, height];
        rows[surface.radius_slot[0]][surface.radius_slot[1]] = radius;
    }

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

/// `a/b/../c` as `a/c`, so files embedded by name are found through `extends`.
pub fn normal(path: &Path) -> PathBuf {
    let mut parts: Vec<Component> = Vec::new();

    for part in path.components() {
        match part {
            Component::ParentDir if matches!(parts.last(), Some(Component::Normal(_))) => {
                parts.pop();
            }
            Component::CurDir => {}
            _ => parts.push(part),
        }
    }

    parts.iter().collect()
}
