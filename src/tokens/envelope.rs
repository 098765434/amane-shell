//! Penpot's single-file export: top-level keys are set names (`sys/base`),
//! each a tree of groups and tokens; plus `$themes` and `$metadata`.

use serde_json::{Map, Value};

use super::TokenError;

/// Penpot's 20 token types, by their names in the export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenType {
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
}

impl TokenType {
    pub fn from_name(name: &str) -> Option<Self> {
        use TokenType::*;

        Some(match name {
            "boolean" => Boolean,
            "borderRadius" => BorderRadius,
            "borderWidth" => BorderWidth,
            "color" => Color,
            "dimension" => Dimension,
            // Penpot also imports the singular legacy names
            "fontFamilies" | "fontFamily" => FontFamilies,
            "fontSizes" | "fontSize" => FontSizes,
            "fontWeights" | "fontWeight" => FontWeights,
            "letterSpacing" => LetterSpacing,
            "number" => Number,
            "opacity" => Opacity,
            "other" => Other,
            "rotation" => Rotation,
            "shadow" | "boxShadow" => Shadow,
            "sizing" => Sizing,
            "spacing" => Spacing,
            "string" => String,
            "textCase" => TextCase,
            "textDecoration" => TextDecoration,
            "typography" => Typography,
            _ => return None,
        })
    }

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

#[derive(Clone, Debug)]
pub struct RawToken {
    pub name: String,
    pub ty: TokenType,
    pub value: Value,
}

#[derive(Debug)]
pub struct Set {
    pub name: String,
    pub tokens: Vec<RawToken>,
}

#[derive(Debug)]
pub struct Theme {
    pub group: String,
    pub name: String,
    pub description: Option<String>,
    /// Sets listed as `"enabled"`.
    pub sets: Vec<String>,
    /// Sets listed with any other state; Penpot's import enables these too.
    pub not_enabled: Vec<String>,
}

#[derive(Debug)]
pub struct Library {
    /// In resolution order: `tokenSetOrder`, then any set it doesn't name.
    pub sets: Vec<Set>,
    pub themes: Vec<Theme>,
}

impl Library {
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
