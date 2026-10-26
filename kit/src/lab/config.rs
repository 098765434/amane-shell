//! A saved Shader Lab look: which effect, every value its controls were
//! set to, the subject it was tried on and its shadows. Saved as
//! `apps/shader-lab/configs/<name>.json`; the lab edits one, and
//! [`super::look`] builds one for the shell.

use std::collections::BTreeMap;

use amane::serde::{Deserialize, Serialize};

use super::manifest::{ColorParam, Manifest, Param};
use crate::color_picker::parse_hex;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(crate = "amane::serde", default)]
pub struct Config {
    pub name: String,
    pub notes: String,
    /// The effect's folder, like `chrome` or `liquid-glass`.
    pub effect: String,
    pub subject: Subject,
    pub backdrop: Backdrop,
    /// By param key; one left out is at the manifest's default.
    pub values: BTreeMap<String, f32>,
    /// By colour key, as `#rrggbb` or `#rrggbbaa`.
    pub colors: BTreeMap<String, String>,
    pub gradient: GradientChoice,
    /// Drawn in order: drop shadows behind the subject, inner ones over it.
    pub shadows: Vec<ShadowSpec>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Subject {
    pub kind: Kind,
    /// The size every subject's measurements follow, in logical pixels.
    pub size: f32,
    pub text: String,
    pub image: String,
    pub photo: String,
    /// How far it's moved from the preview's centre.
    pub x: f32,
    pub y: f32,
}

impl Default for Subject {
    fn default() -> Self {
        Self {
            kind: Kind::Rounded,
            size: 120.0,
            text: "chrome".into(),
            image: String::new(),
            photo: String::new(),
            x: 0.0,
            y: 0.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Rounded,
    Pill,
    Circle,
    Text,
    /// An image fitted inside, like an icon.
    Image,
    /// The whole preview, for effects that are backgrounds.
    Fill,
    /// A picture covering a 16:10 box.
    Photo,
}

impl Kind {
    pub const ALL: [Kind; 7] = [
        Kind::Rounded,
        Kind::Pill,
        Kind::Circle,
        Kind::Text,
        Kind::Image,
        Kind::Fill,
        Kind::Photo,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Rounded => "Rounded rectangle",
            Kind::Pill => "Pill",
            Kind::Circle => "Circle",
            Kind::Text => "Text",
            Kind::Image => "Image",
            Kind::Fill => "Fill preview",
            Kind::Photo => "Photo",
        }
    }

    pub fn index(self) -> usize {
        Kind::ALL.iter().position(|kind| *kind == self).unwrap_or(0)
    }

    pub fn from_index(index: usize) -> Self {
        Kind::ALL.get(index).copied().unwrap_or_default()
    }

    /// The shapes amane can cast shadows from.
    pub fn is_shape(self) -> bool {
        matches!(self, Kind::Rounded | Kind::Pill | Kind::Circle)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Backdrop {
    #[default]
    Dark,
    Light,
    Grey,
    Wallpaper,
}

impl Backdrop {
    pub const ALL: [Backdrop; 4] = [
        Backdrop::Dark,
        Backdrop::Light,
        Backdrop::Grey,
        Backdrop::Wallpaper,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Backdrop::Dark => "Dark",
            Backdrop::Light => "Light",
            Backdrop::Grey => "Grey",
            Backdrop::Wallpaper => "Wallpaper",
        }
    }

    pub fn index(self) -> usize {
        Backdrop::ALL
            .iter()
            .position(|backdrop| *backdrop == self)
            .unwrap_or(0)
    }

    pub fn from_index(index: usize) -> Self {
        Backdrop::ALL.get(index).copied().unwrap_or_default()
    }
}

/// A gradient preset by its index, or stops of the config's own.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(crate = "amane::serde", default)]
pub struct GradientChoice {
    pub preset: usize,
    /// Position, then red, green, blue and alpha from 0 to 1; these win over the preset.
    pub stops: Option<Vec<[f32; 5]>>,
}

/// One shadow, as CSS's box-shadow has it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct ShadowSpec {
    pub inner: bool,
    /// `#rrggbb` or `#rrggbbaa`.
    pub color: String,
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub opacity: f32,
}

impl Default for ShadowSpec {
    fn default() -> Self {
        Self {
            inner: false,
            color: "#00000059".into(),
            x: 0.0,
            y: 8.0,
            blur: 24.0,
            spread: 0.0,
            opacity: 1.0,
        }
    }
}

impl ShadowSpec {
    pub fn rgba(&self) -> [f32; 4] {
        parse_hex(&self.color).unwrap_or([0.0, 0.0, 0.0, 1.0])
    }
}

impl Config {
    /// A fresh config for an effect, at its defaults.
    pub fn new(effect: &str) -> Self {
        Self {
            effect: effect.to_owned(),
            ..Self::default()
        }
    }

    pub fn value(&self, param: &Param) -> f32 {
        self.values
            .get(&param.key)
            .copied()
            .unwrap_or(param.default)
    }

    pub fn color(&self, param: &ColorParam) -> [f32; 4] {
        self.colors
            .get(&param.key)
            .and_then(|hex| parse_hex(hex))
            .or_else(|| parse_hex(&param.default))
            .unwrap_or([0.0, 0.0, 0.0, 1.0])
    }

    /// The gradient's stops: the config's own, or its preset's.
    pub fn stops(&self, manifest: &Manifest) -> Option<Vec<[f32; 5]>> {
        if let Some(stops) = &self.gradient.stops {
            return Some(stops.clone());
        }

        let gradient = manifest.gradient.as_ref()?;
        let preset = gradient
            .presets
            .get(self.gradient.preset)
            .or(gradient.presets.first())?;

        Some(preset.stops.clone())
    }

    /// The effect's `values`, with each param as `value` says (the lab
    /// sweeps one while animating); the surface and presses are added apart.
    pub fn rows_with(&self, manifest: &Manifest, value: impl Fn(&Param) -> f32) -> Vec<[f32; 4]> {
        let stops = self.stops(manifest);

        let mut rows = manifest.pack(value, stops.as_deref());

        manifest.put_colors(&mut rows, |param| self.color(param));

        rows
    }

    pub fn rows(&self, manifest: &Manifest) -> Vec<[f32; 4]> {
        self.rows_with(manifest, |param| self.value(param))
    }

    /// Every param and colour written out, so the saved file says what it
    /// looked like even if a manifest's default changes later.
    pub fn complete(&mut self, manifest: &Manifest) {
        for param in &manifest.params {
            let value = self.value(param);

            self.values.insert(param.key.clone(), value);
        }

        for param in &manifest.colors {
            let hex = crate::color_picker::to_hex(self.color(param));

            self.colors.insert(param.key.clone(), hex);
        }
    }
}

/// A file name for a config's name: lowercase letters, digits and dashes.
pub fn slug(name: &str) -> String {
    let mut slug = String::new();

    for character in name.trim().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }

    let slug = slug.trim_end_matches('-').to_owned();

    if slug.is_empty() {
        "untitled".into()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_goes_to_json_and_back() {
        let mut config = Config::new("liquid-glass");
        config.name = "Dock glass".into();
        config.subject.kind = Kind::Pill;
        config.subject.x = 12.0;
        config.values.insert("refraction".into(), 24.0);
        config.colors.insert("tint".into(), "#ffffff1a".into());
        config.gradient.stops = Some(vec![[0.0, 1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0, 1.0]]);
        config.shadows.push(ShadowSpec::default());
        config.shadows.push(ShadowSpec {
            inner: true,
            ..ShadowSpec::default()
        });

        let json = serde_json::to_string_pretty(&config).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(back, config);
        assert!(json.contains("\"kind\": \"pill\""));
    }

    #[test]
    fn a_config_with_fields_left_out_reads_as_defaults() {
        let config: Config = serde_json::from_str(r#"{ "effect": "chrome" }"#).unwrap();

        assert_eq!(config.subject, Subject::default());
        assert!(config.shadows.is_empty());
    }

    #[test]
    fn names_become_file_names() {
        assert_eq!(slug("Dock glass, dark!"), "dock-glass-dark");
        assert_eq!(slug("  "), "untitled");
        assert_eq!(slug("Chrome 2"), "chrome-2");
    }
}
