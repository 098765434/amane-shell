//! Set selection and reference resolution, as Penpot does with one `mode`
//! and one `density` theme active.

use std::collections::{HashMap, HashSet};

use serde_json::Value as Json;

use super::envelope::{Library, RawToken, TokenType};
use super::values::{self, Value};
use super::{Resolved, ResolvedToken, TokenError};
use crate::theme::{Density, Mode, Surface};

/// Penpot gives up on reference chains this deep.
const MAX_DEPTH: usize = 16;

/// The active tokens for one `mode` × `density`, later sets winning.
pub struct Selection<'a> {
    /// The themes selected, like `mode/dark + density/comfortable`, for messages.
    pub key: String,
    /// Every active token by its dotted name, with the name of the set it came from.
    pub tokens: HashMap<&'a str, (&'a str, &'a RawToken)>,
}

impl Library {
    /// The tokens of the sets the `mode` and `density` themes enable, later sets winning; an
    /// error if either theme is missing.
    pub fn select(
        &self,
        mode: Mode,
        density: Density,
        surface: Surface,
    ) -> Result<Selection<'_>, TokenError> {
        let mut active = HashSet::new();
        // black only over dark; standard enables nothing, so needn't be in the export
        let surface = (mode == Mode::Dark && surface != Surface::Standard)
            .then(|| ("surface", surface.theme()));

        for (group, name) in [("mode", mode.theme()), ("density", density.theme())]
            .into_iter()
            .chain(surface)
        {
            let theme = self.theme(group, name).ok_or_else(|| {
                TokenError::file(format!("the export has no theme {group}/{name}"))
            })?;
            active.extend(theme.sets.iter().map(String::as_str));
        }

        let mut tokens = HashMap::new();
        for set in self
            .sets
            .iter()
            .filter(|s| active.contains(s.name.as_str()))
        {
            for token in &set.tokens {
                tokens.insert(token.name.as_str(), (set.name.as_str(), token));
            }
        }

        Ok(Selection {
            key: match surface {
                Some((_, name)) => format!(
                    "mode/{} + density/{} + surface/{name}",
                    mode.theme(),
                    density.theme()
                ),
                None => format!("mode/{} + density/{}", mode.theme(), density.theme()),
            },
            tokens,
        })
    }
}

impl Selection<'_> {
    /// Resolve every token. `overrides` replace raw values first (the
    /// runtime accent, manual §8.3).
    pub fn resolve(&self, overrides: &[(&str, String)]) -> Result<Resolved, Vec<TokenError>> {
        let mut tokens = HashMap::new();
        let mut errors = Vec::new();

        for (name, result) in self.resolve_each(overrides) {
            match result {
                Ok(token) => {
                    tokens.insert(name, token);
                }
                Err(e) => errors.push(e),
            }
        }

        if !errors.is_empty() {
            // a broken token is reported once, not once per token referencing it
            errors.sort_by(|a, b| a.token.cmp(&b.token));
            errors.dedup_by(|a, b| a.token == b.token && a.message == b.message);
            return Err(errors);
        }

        Ok(Resolved {
            key: self.key.clone(),
            tokens,
            text_scale: 1.0,
        })
    }

    /// Every token's own result, so one bad token doesn't hide the others
    /// (Penpot shows a red pill on each).
    pub fn resolve_each(
        &self,
        overrides: &[(&str, String)],
    ) -> HashMap<String, Result<ResolvedToken, TokenError>> {
        let mut resolver = Resolver {
            selection: self,
            overrides: overrides
                .iter()
                .map(|(name, value)| (*name, Json::String(value.clone())))
                .collect(),
            done: HashMap::new(),
            stack: Vec::new(),
        };

        for name in self.tokens.keys() {
            // results are memoised in `done`
            let _ = resolver.get(name);
        }

        resolver.done
    }
}

struct Resolver<'s, 'a> {
    selection: &'s Selection<'a>,
    overrides: HashMap<&'s str, Json>,
    done: HashMap<String, Result<ResolvedToken, TokenError>>,
    stack: Vec<String>,
}

impl Resolver<'_, '_> {
    fn get(&mut self, name: &str) -> Result<ResolvedToken, TokenError> {
        if let Some(done) = self.done.get(name) {
            return done.clone();
        }

        let &(set, raw) = self
            .selection
            .tokens
            .get(name)
            .ok_or_else(|| self.error(name, "", "is not in any active set"))?;

        if self.stack.iter().any(|n| n == name) {
            let chain = [self.stack.as_slice(), &[name.to_string()]]
                .concat()
                .join(" → ");
            return Err(self.error(name, set, format!("reference cycle: {chain}")));
        }
        if self.stack.len() >= MAX_DEPTH {
            return Err(self.error(
                name,
                set,
                format!("references nest deeper than {MAX_DEPTH}"),
            ));
        }

        self.stack.push(name.to_string());
        let raw_value = self
            .overrides
            .get(name)
            .cloned()
            .unwrap_or_else(|| raw.value.clone());
        let result = self
            .substitute(&raw_value, Some(raw.ty), name, set)
            .and_then(|json| values::parse(raw.ty, &json).map_err(|e| self.error(name, set, e)))
            .map(|value| ResolvedToken { ty: raw.ty, value });
        self.stack.pop();

        self.done.insert(name.to_string(), result.clone());
        result
    }

    /// Replace every `{reference}` in `json`. `ty` is the type the result
    /// must parse as, when known (members of composites are checked by the
    /// composite's parser instead).
    fn substitute(
        &mut self,
        json: &Json,
        ty: Option<TokenType>,
        name: &str,
        set: &str,
    ) -> Result<Json, TokenError> {
        match json {
            Json::String(s) => self.substitute_str(s, ty, name, set),
            Json::Array(items) => items
                .iter()
                .map(|i| self.substitute(i, None, name, set))
                .collect::<Result<_, _>>()
                .map(Json::Array),
            Json::Object(map) => map
                .iter()
                .map(|(k, v)| Ok((k.clone(), self.substitute(v, None, name, set)?)))
                .collect::<Result<_, _>>()
                .map(Json::Object),
            other => Ok(other.clone()),
        }
    }

    fn substitute_str(
        &mut self,
        s: &str,
        ty: Option<TokenType>,
        name: &str,
        set: &str,
    ) -> Result<Json, TokenError> {
        let refs = references(s);
        if refs.is_empty() {
            return Ok(Json::String(s.to_string()));
        }

        // a whole-value reference takes the target's value as is
        if refs.len() == 1 && s.trim() == format!("{{{}}}", refs[0]) {
            let target = self.resolve_ref(refs[0], name, set)?;
            if let Some(ty) = ty
                && !compatible(ty, &target.value)
            {
                return Err(self.error(
                    name,
                    set,
                    format!(
                        "reference {{{}}} is a {} token, which a {} token can't use",
                        refs[0],
                        target.ty.name(),
                        ty.name()
                    ),
                ));
            }
            return Ok(target.value.to_json());
        }

        let mut out = s.to_string();
        for r in refs {
            let target = self.resolve_ref(r, name, set)?;
            let inline = target.value.to_inline().ok_or_else(|| {
                self.error(
                    name,
                    set,
                    format!(
                        "{{{r}}} is a {} token and can't appear inside an expression",
                        target.ty.name()
                    ),
                )
            })?;
            out = out.replace(&format!("{{{r}}}"), &inline);
        }
        Ok(Json::String(out))
    }

    fn resolve_ref(
        &mut self,
        target: &str,
        name: &str,
        set: &str,
    ) -> Result<ResolvedToken, TokenError> {
        if !self.selection.tokens.contains_key(target) {
            return Err(self.error(name, set, format!("reference {{{target}}} has no target")));
        }
        self.get(target)
    }

    fn error(&self, token: &str, set: &str, message: impl Into<String>) -> TokenError {
        TokenError {
            token: token.to_string(),
            set: set.to_string(),
            key: self.selection.key.clone(),
            message: message.into(),
        }
    }
}

/// Names inside `{…}` in a value string.
pub fn references(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;

    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        out.push(rest[open + 1..open + close].trim());
        rest = &rest[open + close + 1..];
    }

    out
}

fn compatible(ty: TokenType, value: &Value) -> bool {
    use TokenType::*;

    match ty {
        Dimension | Sizing | Spacing | BorderRadius | BorderWidth | FontSizes | LetterSpacing
        | Number | Rotation | Opacity => matches!(value, Value::Number(_)),
        Color => matches!(value, Value::Color(_)),
        FontFamilies => matches!(value, Value::FontFamily(_)),
        FontWeights => matches!(value, Value::FontWeight { .. }),
        TextCase => matches!(value, Value::TextCase(_)),
        TextDecoration => matches!(value, Value::TextDecoration(_)),
        Typography => matches!(value, Value::Typography(_)),
        Shadow => matches!(value, Value::Shadow(_)),
        Boolean => matches!(value, Value::Bool(_)),
        String | Other => value.to_inline().is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIDELITY: &str = include_str!("../../tests/fixtures/penpot/fidelity.json");
    const EXPECTED: &str = include_str!("../../tests/fixtures/penpot/fidelity.expected.json");

    /// One token per type and rule, valid and invalid; amane must produce
    /// Penpot's values and fail on the tokens Penpot shows a red pill on.
    #[test]
    fn resolver_fidelity() {
        let lib = Library::parse(FIDELITY).expect("fixture parses");
        let expected: Json = serde_json::from_str(EXPECTED).unwrap();
        let mut failures = Vec::new();

        for mode in Mode::ALL {
            let key = format!("mode/{}", mode.theme());
            let results = lib
                .select(mode, Density::Comfortable, Surface::Standard)
                .unwrap()
                .resolve_each(&[]);

            for (name, want) in expected[&key].as_object().unwrap() {
                let got = results.get(name);
                let ok = match (want.get("error"), got) {
                    (Some(_), Some(Err(_))) => true,
                    (None, Some(Ok(token))) => token.value.to_json() == *want,
                    _ => false,
                };
                if !ok {
                    let got = match got {
                        Some(Ok(t)) => t.value.to_json().to_string(),
                        Some(Err(e)) => format!("error: {e}"),
                        None => String::from("not resolved"),
                    };
                    failures.push(format!("[{key}] {name}: want {want}, got {got}"));
                }
            }
        }

        assert!(failures.is_empty(), "\n  {}", failures.join("\n  "));
    }

    /// The bundled export's black surfaces override dark's, and only dark's.
    #[test]
    fn black_turns_darks_surfaces_black() {
        let lib = super::super::library();
        let value = |mode, surface, name: &str| {
            lib.select(mode, Density::Comfortable, surface)
                .unwrap()
                .resolve(&[])
                .unwrap()
                .tokens[name]
                .value
                .to_json()
        };
        let bg = "sys.color.window.bg";
        assert_eq!(value(Mode::Dark, Surface::Black, bg), "#000000");
        assert_ne!(value(Mode::Dark, Surface::Standard, bg), "#000000");
        // light ignores it
        assert_eq!(
            value(Mode::Light, Surface::Black, bg),
            value(Mode::Light, Surface::Standard, bg)
        );
        // what black leaves alone stays dark's
        let accent = "sys.color.status.critical.text";
        assert_eq!(
            value(Mode::Dark, Surface::Black, accent),
            value(Mode::Dark, Surface::Standard, accent)
        );
    }

    #[test]
    fn a_missing_theme_is_named() {
        let lib = Library::parse(r#"{ "s": {}, "$themes": [] }"#).unwrap();
        let Err(e) = lib.select(Mode::Dark, Density::Comfortable, Surface::Standard) else {
            panic!("selected without themes");
        };
        assert!(e.message.contains("mode/dark"), "{e}");
    }
}
