//! Penpot's single-file export: top-level keys are set names (`sys/base`),
//! each a tree of groups and tokens; plus `$themes` and `$metadata`.
//!
//! `build.rs` includes this file (with `error.rs`) to generate the token
//! functions, so the build reads the export exactly as the shell does.

use serde_json::{Map, Value};

use super::error::TokenError;

/// Penpot's 20 token types, by their names in the export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenType {
    /// `boolean`: true or false.
    Boolean,
    /// `borderRadius`: a corner radius.
    BorderRadius,
    /// `borderWidth`: a stroke width.
    BorderWidth,
    /// `color`: a color.
    Color,
    /// `dimension`: a length of no more particular kind.
    Dimension,
    /// `fontFamilies`: a stack of font family names.
    FontFamilies,
    /// `fontSizes`: a text size.
    FontSizes,
    /// `fontWeights`: a weight, like 400 or `bold`.
    FontWeights,
    /// `letterSpacing`: space added after every letter.
    LetterSpacing,
    /// `number`: a plain number.
    Number,
    /// `opacity`: from 0 to 1, or a percentage.
    Opacity,
    /// `other`: free text.
    Other,
    /// `rotation`: an angle in degrees.
    Rotation,
    /// `shadow`: one or more shadows.
    Shadow,
    /// `sizing`: a width or height.
    Sizing,
    /// `spacing`: a gap or an inset.
    Spacing,
    /// `string`: text.
    String,
    /// `textCase`: upper, lower or capitalized text.
    TextCase,
    /// `textDecoration`: underline or strike-through.
    TextDecoration,
    /// `typography`: a whole text style.
    Typography,
}

impl TokenType {
    /// The type with this name in the export, also taking the older singular names
    /// (`fontFamily`, `fontSize`, `fontWeight`, `boxShadow`).
    pub fn from_name(name: &str) -> Option<Self> {
        use TokenType::*;

        // Penpot also imports the singular legacy names
        let legacy = match name {
            "fontFamily" => Some(FontFamilies),
            "fontSize" => Some(FontSizes),
            "fontWeight" => Some(FontWeights),
            "boxShadow" => Some(Shadow),
            _ => None,
        };
        legacy.or_else(|| ALL.iter().copied().find(|ty| ty.name() == name))
    }

    /// The type's name in the export.
    pub fn name(self) -> &'static str {
        use TokenType::*;

        match self {
            Boolean => "boolean",
            BorderRadius => "borderRadius",
            BorderWidth => "borderWidth",
            Color => "color",
            Dimension => "dimension",
            FontFamilies => "fontFamilies",
            FontSizes => "fontSizes",
            FontWeights => "fontWeights",
            LetterSpacing => "letterSpacing",
            Number => "number",
            Opacity => "opacity",
            Other => "other",
            Rotation => "rotation",
            Shadow => "shadow",
            Sizing => "sizing",
            Spacing => "spacing",
            String => "string",
            TextCase => "textCase",
            TextDecoration => "textDecoration",
            Typography => "typography",
        }
    }
}

/// Every type, for looking one up by name.
const ALL: [TokenType; 20] = {
    use TokenType::*;
    [
        Boolean,
        BorderRadius,
        BorderWidth,
        Color,
        Dimension,
        FontFamilies,
        FontSizes,
        FontWeights,
        LetterSpacing,
        Number,
        Opacity,
        Other,
        Rotation,
        Shadow,
        Sizing,
        Spacing,
        String,
        TextCase,
        TextDecoration,
        Typography,
    ]
};

/// One token as the export has it, before its references are resolved.
#[derive(Clone, Debug)]
pub struct RawToken {
    /// Its dotted path, like `sys.color.primary`.
    pub name: String,
    /// Its own `$type`, or the nearest group's.
    pub ty: TokenType,
    /// Its `$value`, untouched.
    pub value: Value,
}

/// A token set: one top-level key of the export.
#[derive(Debug)]
pub struct Set {
    /// The set's name, like `sys/base`.
    pub name: String,
    /// Every token in the set, flattened out of its groups.
    pub tokens: Vec<RawToken>,
}

/// A theme from `$themes`: a named choice of sets, one of a group.
#[derive(Debug)]
pub struct Theme {
    /// The group it's one choice of, like `mode`; empty when it has none.
    pub group: String,
    /// Its name within the group, like `dark`.
    pub name: String,
    /// Its description, if the export gives one.
    pub description: Option<String>,
    /// Sets listed as `"enabled"`.
    pub sets: Vec<String>,
    /// Sets listed with any other state; Penpot's import enables these too.
    pub not_enabled: Vec<String>,
}

/// A whole parsed export: its sets and themes.
#[derive(Debug)]
pub struct Library {
    /// In resolution order: `tokenSetOrder`, then any set it doesn't name.
    pub sets: Vec<Set>,
    /// Every theme in `$themes`, in the export's order.
    pub themes: Vec<Theme>,
}

impl Library {
    /// Reads an export. A set or group that isn't an object, a token without a type, or a hand-
    /// added `$extensions` is an error.
    pub fn parse(json: &str) -> Result<Library, TokenError> {
        let root: Value = serde_json::from_str(json)
            .map_err(|e| TokenError::file(format!("invalid JSON: {e}")))?;
        let root = root
            .as_object()
            .ok_or_else(|| TokenError::file("the export is not a JSON object"))?;

        let order: Vec<String> = root
            .get("$metadata")
            .and_then(|m| m.get("tokenSetOrder"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let mut names: Vec<&String> = order.iter().filter(|n| root.contains_key(*n)).collect();
        for key in root.keys() {
            if !key.starts_with('$') && !names.contains(&key) {
                names.push(key);
            }
        }

        let mut sets = Vec::new();
        for name in names {
            let tree = root[name.as_str()]
                .as_object()
                .ok_or_else(|| TokenError::in_set(name, "", "set is not an object"))?;
            let mut tokens = Vec::new();
            flatten(name, tree, "", None, &mut tokens)?;
            sets.push(Set {
                name: name.clone(),
                tokens,
            });
        }

        let themes = match root.get("$themes") {
            None => Vec::new(),
            Some(Value::Array(list)) => list.iter().map(theme).collect::<Result<_, _>>()?,
            Some(_) => return Err(TokenError::file("$themes is not an array")),
        };

        Ok(Library { sets, themes })
    }

    /// The theme named `name` in `group`, if there is one.
    pub fn theme(&self, group: &str, name: &str) -> Option<&Theme> {
        self.themes
            .iter()
            .find(|t| t.group == group && t.name == name)
    }
}

fn flatten(
    set: &str,
    node: &Map<String, Value>,
    path: &str,
    inherited: Option<TokenType>,
    out: &mut Vec<RawToken>,
) -> Result<(), TokenError> {
    if node.contains_key("$extensions") {
        return Err(TokenError::in_set(
            set,
            path,
            "has $extensions (Penpot drops them, so this is a hand edit)",
        ));
    }

    let own_type = match node.get("$type") {
        None => None,
        Some(Value::String(s)) => Some(
            TokenType::from_name(s)
                .ok_or_else(|| TokenError::in_set(set, path, format!("unknown $type `{s}`")))?,
        ),
        Some(other) => {
            return Err(TokenError::in_set(
                set,
                path,
                format!("$type is not a string: {other}"),
            ));
        }
    };
    let ty = own_type.or(inherited);

    if let Some(value) = node.get("$value") {
        let ty = ty.ok_or_else(|| TokenError::in_set(set, path, "token has no $type"))?;
        out.push(RawToken {
            name: path.to_string(),
            ty,
            value: value.clone(),
        });
        return Ok(());
    }

    for (key, child) in node {
        if key.starts_with('$') {
            continue;
        }
        let child_path = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };
        let child = child.as_object().ok_or_else(|| {
            TokenError::in_set(set, &child_path, "is neither a group nor a token")
        })?;
        flatten(set, child, &child_path, ty, out)?;
    }

    Ok(())
}

fn theme(value: &Value) -> Result<Theme, TokenError> {
    let field = |key: &str| value.get(key).and_then(Value::as_str).map(String::from);

    let name = field("name").ok_or_else(|| TokenError::file("a theme has no name"))?;
    let group = field("group").unwrap_or_default();
    let mut sets = Vec::new();
    let mut not_enabled = Vec::new();

    if let Some(selected) = value.get("selectedTokenSets").and_then(Value::as_object) {
        for (set, state) in selected {
            if state.as_str() == Some("enabled") {
                sets.push(set.clone());
            } else {
                not_enabled.push(set.clone());
            }
        }
    }

    Ok(Theme {
        group,
        name,
        description: field("description"),
        sets,
        not_enabled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_follow_token_set_order() {
        let lib = Library::parse(
            r#"{
                "b": {}, "a": {}, "c": {},
                "$metadata": { "tokenSetOrder": ["c", "a"] }
            }"#,
        )
        .unwrap();
        let names: Vec<_> = lib.sets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["c", "a", "b"]);
    }

    #[test]
    fn group_type_is_inherited() {
        let lib = Library::parse(
            r#"{ "s": { "space": { "$type": "spacing", "s": { "$value": "4" },
                        "w": { "$type": "sizing", "$value": "9" } } } }"#,
        )
        .unwrap();
        let tokens = &lib.sets[0].tokens;
        assert_eq!(tokens.len(), 2);
        assert!(
            tokens
                .iter()
                .any(|t| t.name == "space.s" && t.ty == TokenType::Spacing)
        );
        assert!(
            tokens
                .iter()
                .any(|t| t.name == "space.w" && t.ty == TokenType::Sizing)
        );
    }

    #[test]
    fn type_names_round_trip() {
        for ty in ALL {
            assert_eq!(TokenType::from_name(ty.name()), Some(ty));
        }
        assert_eq!(TokenType::from_name("fontSize"), Some(TokenType::FontSizes));
        assert_eq!(TokenType::from_name("pixels"), None);
    }

    #[test]
    fn refuses_hand_edits() {
        assert!(Library::parse(r#"{ "s": { "a": { "$value": "1" } } }"#).is_err());
        assert!(
            Library::parse(r#"{ "s": { "a": { "$type": "pixels", "$value": "1" } } }"#).is_err()
        );
        assert!(
            Library::parse(
                r#"{ "s": { "a": { "$type": "number", "$value": "1", "$extensions": {} } } }"#
            )
            .is_err()
        );
    }
}
