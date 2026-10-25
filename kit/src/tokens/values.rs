//! Per-type value rules, mirroring Penpot's validators: what a token of each
//! `$type` may hold once its references are substituted.

use serde_json::{Value as Json, json};

use super::color::{self, Rgba};
use super::envelope::TokenType;
use super::expr::{self, Num};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextCase {
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDecoration {
    None,
    Underline,
    StrikeThrough,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    pub families: Vec<String>,
    pub size: Option<f32>,
    pub weight: Option<(u16, bool)>,
    pub letter_spacing: Option<f32>,
    /// A multiple of the font size.
    pub line_height: Option<f32>,
    pub text_case: Option<TextCase>,
    pub text_decoration: Option<TextDecoration>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Rgba,
    pub inset: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Color(Rgba),
    /// Every number-like type: dimensions, sizes, opacity, rotation, number.
    Number(Num),
    FontFamily(Vec<String>),
    FontWeight {
        weight: u16,
        italic: bool,
    },
    TextCase(TextCase),
    TextDecoration(TextDecoration),
    Typography(Typography),
    Shadow(Vec<Shadow>),
    Bool(bool),
    Str(String),
}

impl Value {
    /// The value in a form [`parse`] reads back, for substituting references.
    pub fn to_json(&self) -> Json {
        match self {
            Value::Color(c) => Json::String(c.hex()),
            Value::Number(n) => Json::String(num_text(*n)),
            Value::FontFamily(f) => json!(f),
            Value::FontWeight { weight, italic } => {
                Json::String(format!("{weight}{}", if *italic { " italic" } else { "" }))
            }
            Value::TextCase(c) => Json::String(text_case_name(*c).into()),
            Value::TextDecoration(d) => Json::String(text_decoration_name(*d).into()),
            Value::Typography(t) => {
                let mut map = serde_json::Map::new();
                map.insert("fontFamilies".into(), json!(t.families));
                if let Some(s) = t.size {
                    map.insert("fontSizes".into(), json!(s.to_string()));
                }
                if let Some((w, italic)) = t.weight {
                    let weight = Value::FontWeight { weight: w, italic };
                    map.insert("fontWeights".into(), weight.to_json());
                }
                if let Some(l) = t.letter_spacing {
                    map.insert("letterSpacing".into(), json!(l.to_string()));
                }
                if let Some(l) = t.line_height {
                    map.insert("lineHeights".into(), json!(l.to_string()));
                }
                if let Some(c) = t.text_case {
                    map.insert("textCase".into(), json!(text_case_name(c)));
                }
                if let Some(d) = t.text_decoration {
                    map.insert("textDecoration".into(), json!(text_decoration_name(d)));
                }
                Json::Object(map)
            }
            Value::Shadow(layers) => Json::Array(
                layers
                    .iter()
                    .map(|s| {
                        json!({
                            "offsetX": s.x.to_string(), "offsetY": s.y.to_string(),
                            "blur": s.blur.to_string(), "spread": s.spread.to_string(),
                            "color": s.color.hex(), "inset": s.inset,
                        })
                    })
                    .collect(),
            ),
            Value::Bool(b) => Json::Bool(*b),
            Value::Str(s) => Json::String(s.clone()),
        }
    }

    /// The value inside a longer string (`{a} * 2`, `rgba({c}, 0.5)`).
    pub fn to_inline(&self) -> Option<String> {
        match self {
            // parenthesised so a negative value can't merge with an operator
            Value::Number(n) => Some(format!("({})", num_text(*n))),
            Value::Color(c) => Some(c.hex()),
            Value::Str(s) => Some(s.clone()),
            Value::FontWeight { .. } | Value::TextCase(_) | Value::TextDecoration(_) => {
                self.to_json().as_str().map(String::from)
            }
            Value::FontFamily(f) => Some(f.join(", ")),
            _ => None,
        }
    }
}

fn num_text(n: Num) -> String {
    format!("{}{}", n.value, if n.px { "px" } else { "" })
}

/// Parse a token's value (references already substituted) by its type.
pub fn parse(ty: TokenType, value: &Json) -> Result<Value, String> {
    use TokenType::*;

    match ty {
        Color => color::parse(text(value)?).map(Value::Color),
        Dimension | Sizing | Spacing | BorderRadius | FontSizes => length(value).map(Value::Number),
        BorderWidth => {
            let n = length(value)?;
            if n.value < 0.0 {
                return Err(format!("border width {} is negative", n.value));
            }
            Ok(Value::Number(n))
        }
        LetterSpacing => {
            if text_or_number(value)?.trim_end().ends_with('%') {
                return Err("letter spacing in % is not supported".into());
            }
            length(value).map(Value::Number)
        }
        Number | Rotation => {
            let n = number(value)?;
            if n.px {
                return Err(format!("{} takes a unitless number", ty.name()));
            }
            Ok(Value::Number(n))
        }
        Opacity => opacity(value).map(|v| {
            Value::Number(Num {
                value: v,
                px: false,
            })
        }),
        FontFamilies => families(value).map(Value::FontFamily),
        FontWeights => {
            let (weight, italic) = font_weight(value)?;
            Ok(Value::FontWeight { weight, italic })
        }
        TextCase => text_case(text(value)?).map(Value::TextCase),
        TextDecoration => text_decoration(text(value)?).map(Value::TextDecoration),
        Typography => typography(value).map(Value::Typography),
        Shadow => shadow(value).map(Value::Shadow),
        Boolean => match value {
            Json::Bool(b) => Ok(Value::Bool(*b)),
            Json::String(s) if s.trim() == "true" => Ok(Value::Bool(true)),
            Json::String(s) if s.trim() == "false" => Ok(Value::Bool(false)),
            other => Err(format!("{other} is not true or false")),
        },
        String | Other => text(value).map(|s| Value::Str(s.to_string())),
    }
}

fn text(value: &Json) -> Result<&str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("expected a string, found {value}"))
}

fn text_or_number(value: &Json) -> Result<String, String> {
    match value {
        Json::String(s) => Ok(s.clone()),
        Json::Number(n) => Ok(n.to_string()),
        other => Err(format!("expected a number, found {other}")),
    }
}

fn number(value: &Json) -> Result<Num, String> {
    expr::eval(&text_or_number(value)?)
}

/// px or unitless (which Penpot reads as px).
fn length(value: &Json) -> Result<Num, String> {
    number(value).map(|n| Num { px: true, ..n })
}

fn opacity(value: &Json) -> Result<f64, String> {
    let raw = text_or_number(value)?;
    let v = match percent(&raw) {
        Some(fraction) => fraction?,
        None => {
            let n = number(value)?;
            if n.px {
                return Err(String::from("opacity takes no unit"));
            }
            n.value
        }
    };

    if !(0.0..=1.0).contains(&v) {
        return Err(format!("opacity {raw} is outside 0–1 (0%–100%)"));
    }
    Ok(v)
}

fn families(value: &Json) -> Result<Vec<String>, String> {
    let list: Vec<String> = match value {
        Json::String(s) => s.split(',').map(|f| f.trim().to_string()).collect(),
        Json::Array(items) => items
            .iter()
            .map(|i| text(i).map(|s| s.trim().to_string()))
            .collect::<Result<_, _>>()?,
        other => return Err(format!("expected font families, found {other}")),
    };

    let list: Vec<String> = list.into_iter().filter(|f| !f.is_empty()).collect();
    if list.is_empty() {
        return Err(String::from("empty font family list"));
    }
    Ok(list)
}

/// Penpot's names, matched case- and separator-insensitively.
const WEIGHT_NAMES: &[(&str, u16)] = &[
    ("thin", 100),
    ("hairline", 100),
    ("extralight", 200),
    ("ultralight", 200),
    ("light", 300),
    ("regular", 400),
    ("normal", 400),
    ("book", 400),
    ("medium", 500),
    ("semibold", 600),
    ("demibold", 600),
    ("bold", 700),
    ("extrabold", 800),
    ("ultrabold", 800),
    ("black", 900),
    ("heavy", 900),
    ("extrablack", 950),
    ("ultrablack", 950),
];

fn font_weight(value: &Json) -> Result<(u16, bool), String> {
    let raw = text_or_number(value)?;
    let lower = raw.trim().to_ascii_lowercase();
    let (base, italic) = match lower.strip_suffix("italic") {
        Some(base) => (base.trim(), true),
        None => (lower.as_str(), false),
    };

    if let Ok(n) = base.parse::<u16>() {
        if (n % 100 == 0 && (100..=900).contains(&n)) || n == 950 {
            return Ok((n, italic));
        }
        return Err(format!(
            "font weight {n} is not accepted by Penpot (100–900 in hundreds, or 950)"
        ));
    }

    let key: String = base.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    WEIGHT_NAMES
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, w)| (*w, italic))
        .ok_or_else(|| format!("`{raw}` is not a font weight"))
}

/// Penpot's names for text cases and decorations, read both ways.
const TEXT_CASES: [(&str, TextCase); 4] = [
    ("none", TextCase::None),
    ("uppercase", TextCase::Uppercase),
    ("lowercase", TextCase::Lowercase),
    ("capitalize", TextCase::Capitalize),
];
const TEXT_DECORATIONS: [(&str, TextDecoration); 3] = [
    ("none", TextDecoration::None),
    ("underline", TextDecoration::Underline),
    ("strike-through", TextDecoration::StrikeThrough),
];

fn by_name<T: Copy>(table: &[(&'static str, T)], name: &str, what: &str) -> Result<T, String> {
    table
        .iter()
        .find(|(n, _)| *n == name.trim())
        .map(|(_, v)| *v)
        .ok_or_else(|| format!("`{}` is not a {what}", name.trim()))
}

fn name_of<T: Copy + PartialEq>(table: &[(&'static str, T)], value: T) -> &'static str {
    table
        .iter()
        .find(|(_, v)| *v == value)
        .map(|(n, _)| *n)
        .expect("every variant is in its table")
}

fn text_case(s: &str) -> Result<TextCase, String> {
    by_name(&TEXT_CASES, s, "text case")
}

fn text_case_name(c: TextCase) -> &'static str {
    name_of(&TEXT_CASES, c)
}

fn text_decoration(s: &str) -> Result<TextDecoration, String> {
    by_name(&TEXT_DECORATIONS, s, "text decoration")
}

fn text_decoration_name(d: TextDecoration) -> &'static str {
    name_of(&TEXT_DECORATIONS, d)
}

/// A typography member key with its spelling evened out: `font-size`,
/// `fontSize` and `fontSizes` all read `fontsize…`.
fn member_key(key: &str) -> String {
    key.replace('-', "").to_ascii_lowercase()
}

/// A typography member's type, in any of the spellings Penpot imports.
fn member(key: &str) -> Option<TokenType> {
    Some(match member_key(key).as_str() {
        "fontfamilies" | "fontfamily" => TokenType::FontFamilies,
        "fontsizes" | "fontsize" => TokenType::FontSizes,
        "fontweights" | "fontweight" => TokenType::FontWeights,
        "letterspacing" => TokenType::LetterSpacing,
        "textcase" => TokenType::TextCase,
        "textdecoration" => TokenType::TextDecoration,
        _ => return None,
    })
}

/// Line height has no token type of its own: it's a ratio of the size.
fn is_line_height(key: &str) -> bool {
    matches!(member_key(key).as_str(), "lineheights" | "lineheight")
}

/// `raw` as a fraction when written as a percentage (`"50%"` is 0.5).
fn percent(raw: &str) -> Option<Result<f64, String>> {
    let pct = raw.trim().strip_suffix('%')?;
    Some(number(&Json::String(pct.into())).map(|n| n.value / 100.0))
}

fn typography(value: &Json) -> Result<Typography, String> {
    let map = value
        .as_object()
        .ok_or_else(|| format!("expected a typography object, found {value}"))?;

    let mut t = Typography {
        families: Vec::new(),
        size: None,
        weight: None,
        letter_spacing: None,
        line_height: None,
        text_case: None,
        text_decoration: None,
    };
    let mut line_height = None;

    for (key, member_value) in map {
        if is_line_height(key) {
            line_height = Some(member_value);
            continue;
        }
        let ty = member(key).ok_or_else(|| format!("unknown typography member `{key}`"))?;
        let parsed = parse(ty, member_value).map_err(|e| format!("{key}: {e}"))?;
        match parsed {
            Value::FontFamily(f) => t.families = f,
            Value::FontWeight { weight, italic } => t.weight = Some((weight, italic)),
            Value::TextCase(c) => t.text_case = Some(c),
            Value::TextDecoration(d) => t.text_decoration = Some(d),
            Value::Number(n) if ty == TokenType::FontSizes => t.size = Some(n.value as f32),
            Value::Number(n) => t.letter_spacing = Some(n.value as f32),
            _ => unreachable!("member types parse to these variants"),
        }
    }

    if let Some(lh) = line_height {
        let size = t
            .size
            .ok_or_else(|| String::from("lineHeights needs fontSizes in the same typography"))?;
        let raw = text_or_number(lh)?;
        t.line_height = Some(match percent(&raw) {
            Some(fraction) => fraction? as f32,
            None => {
                let n = number(lh)?;
                if n.px {
                    n.value as f32 / size
                } else {
                    n.value as f32
                }
            }
        });
    }

    Ok(t)
}

fn shadow(value: &Json) -> Result<Vec<Shadow>, String> {
    let layers: Vec<&Json> = match value {
        Json::Array(items) => items.iter().collect(),
        Json::Object(_) => vec![value],
        other => return Err(format!("expected a shadow, found {other}")),
    };

    layers
        .into_iter()
        .map(|layer| {
            let get = |a: &str, b: &str| layer.get(a).or_else(|| layer.get(b));
            let px = |a: &str, b: &str| -> Result<f32, String> {
                get(a, b)
                    .map(|v| length(v).map(|n| n.value as f32))
                    .unwrap_or(Ok(0.0))
            };

            let blur = px("blur", "blur")?;
            if blur < 0.0 {
                return Err(String::from("shadow blur is negative"));
            }
            let color = layer
                .get("color")
                .ok_or_else(|| String::from("shadow has no color"))?;
            let inset = match layer.get("inset") {
                Some(Json::Bool(b)) => *b,
                Some(Json::String(s)) => s == "true",
                _ => layer.get("type").and_then(Json::as_str) == Some("innerShadow"),
            };

            Ok(Shadow {
                x: px("offsetX", "x")?,
                y: px("offsetY", "y")?,
                blur,
                spread: px("spread", "spread")?,
                color: color::parse(text(color)?)?,
                inset,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenType::*;

    fn ok(ty: TokenType, v: Json) -> Value {
        parse(ty, &v).unwrap_or_else(|e| panic!("{} {v}: {e}", ty.name()))
    }

    fn err(ty: TokenType, v: Json) {
        assert!(parse(ty, &v).is_err(), "{} {v} should fail", ty.name());
    }

    fn n(v: f64) -> Value {
        Value::Number(Num { value: v, px: true })
    }

    #[test]
    fn lengths() {
        assert_eq!(ok(Spacing, json!("8")), n(8.0));
        assert_eq!(ok(Sizing, json!("8px")), n(8.0));
        assert_eq!(ok(Dimension, json!(-4)), n(-4.0));
        assert_eq!(ok(BorderRadius, json!("round(10 * 0.875)")), n(9.0));
        err(Sizing, json!("1rem"));
        err(Sizing, json!("2em"));
        err(Spacing, json!("50%"));
        err(BorderWidth, json!("-1"));
        err(LetterSpacing, json!("5%"));
    }

    #[test]
    fn unitless() {
        assert_eq!(
            ok(Number, json!("0.56")),
            Value::Number(Num {
                value: 0.56,
                px: false
            })
        );
        err(Number, json!("4px"));
        err(Rotation, json!("90px"));
    }

    #[test]
    fn opacity() {
        assert_eq!(
            ok(Opacity, json!("0.72")),
            Value::Number(Num {
                value: 0.72,
                px: false
            })
        );
        assert_eq!(
            ok(Opacity, json!("50%")),
            Value::Number(Num {
                value: 0.5,
                px: false
            })
        );
        err(Opacity, json!("1.5"));
        err(Opacity, json!("120%"));
    }

    #[test]
    fn fonts() {
        assert_eq!(
            ok(FontFamilies, json!("Inter, sans-serif")),
            Value::FontFamily(vec!["Inter".into(), "sans-serif".into()])
        );
        assert_eq!(
            ok(FontWeights, json!("semi-bold italic")),
            Value::FontWeight {
                weight: 600,
                italic: true
            }
        );
        assert_eq!(
            ok(FontWeights, json!(500)),
            Value::FontWeight {
                weight: 500,
                italic: false
            }
        );
        err(FontWeights, json!("510"));
        err(FontFamilies, json!([1]));
        err(TextCase, json!("small-caps"));
        err(TextDecoration, json!("overline"));
    }

    #[test]
    fn typography_members() {
        let t = ok(
            Typography,
            json!({"fontFamilies": ["Inter"], "font-size": "13", "fontWeight": "500",
                   "lineHeights": "150%"}),
        );
        let Value::Typography(t) = t else { panic!() };
        assert_eq!(t.size, Some(13.0));
        assert_eq!(t.weight, Some((500, false)));
        assert_eq!(t.line_height, Some(1.5));
        err(Typography, json!({"lineHeights": "1.2"}));
    }

    #[test]
    fn shadows() {
        let v = ok(
            Shadow,
            json!({"x": "0", "y": "4", "blur": "12", "color": "#00000033"}),
        );
        let Value::Shadow(layers) = v else { panic!() };
        assert_eq!(layers[0].y, 4.0);
        err(
            Shadow,
            json!({"offsetX": "0", "offsetY": "4", "blur": "12"}),
        );
        err(Shadow, json!({"blur": "-1", "color": "#000"}));
    }

    #[test]
    fn round_trips_through_json() {
        for (ty, v) in [
            (Color, json!("#ff453a2e")),
            (Spacing, json!("6")),
            (FontFamilies, json!(["SF Pro Text", "Inter"])),
            (FontWeights, json!("500")),
            (
                Typography,
                json!({"fontFamilies": ["Inter"], "fontSizes": "13"}),
            ),
            (
                Shadow,
                json!([{"offsetX": "0", "offsetY": "1", "blur": "2", "color": "#000"}]),
            ),
        ] {
            let value = ok(ty, v);
            assert_eq!(parse(ty, &value.to_json()), Ok(value));
        }
    }
}
