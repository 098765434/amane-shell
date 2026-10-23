//! Design tokens from Penpot (`design-tokens/tokens.json`), resolved the way
//! Penpot resolves them. Components read them through a [`Reader`] for
//! `sys.*` and one for their own prefix; there is no reader for `ref.*`.
//! The rules live in `docs/design-token-manual.md`.

pub mod accent;
pub mod check;
pub mod color;
pub mod envelope;
pub mod expr;
pub mod resolve;
pub mod rules;
pub mod values;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use amane::Color;

use crate::theme::{Density, Mode};
pub use envelope::{Library, TokenType};
pub use values::{Shadow, TextCase, TextDecoration, Typography, Value};

/// The committed Penpot export.
pub const SOURCE: &str = include_str!("../../design-tokens/tokens.json");

/// The prefixes component tokens may use (manual §3).
pub const COMPONENTS: &[&str] = &["bar", "menu", "wifi", "power", "osd", "notification"];

#[derive(Clone, Debug, PartialEq)]
pub struct TokenError {
    pub token: String,
    pub set: String,
    /// The `mode` × `density` selection, when one applies.
    pub key: String,
    pub message: String,
}

impl TokenError {
    pub fn file(message: impl Into<String>) -> Self {
        Self::in_set("", "", message)
    }

    pub fn in_set(set: &str, token: &str, message: impl Into<String>) -> Self {
        Self {
            token: token.to_string(),
            set: set.to_string(),
            key: String::new(),
            message: message.into(),
        }
    }
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let place: Vec<String> = [
            (!self.set.is_empty()).then(|| format!("set {}", self.set)),
            (!self.key.is_empty()).then(|| self.key.clone()),
        ]
        .into_iter()
        .flatten()
        .collect();

        match (self.token.is_empty(), place.is_empty()) {
            (true, true) => write!(f, "{}", self.message),
            (true, false) => write!(f, "({}): {}", place.join(", "), self.message),
            (false, true) => write!(f, "{}: {}", self.token, self.message),
            (false, false) => write!(f, "{} ({}): {}", self.token, place.join(", "), self.message),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedToken {
    pub ty: TokenType,
    pub set: String,
    pub value: Value,
}

/// Every active token for one selection, resolved.
#[derive(Debug)]
pub struct Resolved {
    pub key: String,
    pub tokens: HashMap<String, ResolvedToken>,
    /// Multiplies font sizes when read (manual §8.4).
    pub text_scale: f32,
}

impl Resolved {
    pub fn sys(&self) -> Reader<'_> {
        Reader::new(self, "sys")
    }

    pub fn component(&self, name: &str) -> Reader<'_> {
        assert!(
            COMPONENTS.contains(&name),
            "`{name}` is not a component prefix; code may read only sys.* and its own component"
        );
        Reader::new(self, name)
    }
}

/// What the runtime varies on top of Penpot. Resolution is cached per key.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Key {
    pub mode: Mode,
    pub density: Density,
    /// The system accent as 8-bit RGB, replacing `ref.color.accent.*`.
    pub accent: Option<[u8; 3]>,
    /// Text scale in hundredths.
    pub text_scale: u16,
}

impl Key {
    pub fn new(mode: Mode, density: Density) -> Self {
        Self {
            mode,
            density,
            accent: None,
            text_scale: 100,
        }
    }
}

pub fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    // SOURCE is embedded, and the contract tests parse it in every
    // combination, so a failure here can't reach a passing build.
    LIBRARY.get_or_init(|| {
        Library::parse(SOURCE).unwrap_or_else(|e| panic!("design-tokens/tokens.json: {e}"))
    })
}

/// Resolve the bundled tokens for `key`, once per key.
pub fn resolved(key: Key) -> &'static Resolved {
    static CACHE: OnceLock<Mutex<HashMap<Key, &'static Resolved>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    cache.entry(key).or_insert_with(|| {
        let resolved = resolve_key(library(), key).unwrap_or_else(|errors| {
            let list: Vec<String> = errors.iter().map(ToString::to_string).collect();
            panic!("design-tokens/tokens.json:\n  {}", list.join("\n  "))
        });
        Box::leak(Box::new(resolved))
    })
}

pub fn resolve_key(library: &Library, key: Key) -> Result<Resolved, Vec<TokenError>> {
    let overrides: Vec<(&str, String)> = key
        .accent
        .map(|[r, g, b]| {
            let seed = color::Rgba::opaque(r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
            accent::derive(seed).to_vec()
        })
        .unwrap_or_default();

    let mut resolved = library
        .select(key.mode, key.density)
        .map_err(|e| vec![e])?
        .resolve(&overrides)?;
    resolved.text_scale = key.text_scale as f32 / 100.0;
    Ok(resolved)
}

/// Reads tokens under one prefix, checking each against the Penpot types
/// that Penpot's own inputs accept for that property (manual §10.4).
pub struct Reader<'r> {
    resolved: &'r Resolved,
    prefix: &'static str,
    reads: RefCell<Vec<String>>,
}

impl<'r> Reader<'r> {
    fn new(resolved: &'r Resolved, prefix: &str) -> Self {
        let prefix = COMPONENTS
            .iter()
            .chain(&["sys"])
            .find(|p| **p == prefix)
            .copied()
            .unwrap_or("sys");
        Self {
            resolved,
            prefix,
            reads: RefCell::new(Vec::new()),
        }
    }

    /// Every full token name read so far (for the contract tests).
    pub fn read_paths(&self) -> Vec<String> {
        self.reads.borrow().clone()
    }

    fn get(
        &self,
        path: &str,
        accepts: &[TokenType],
        reader: &str,
    ) -> Result<&'r Value, TokenError> {
        let name = format!("{}.{path}", self.prefix);
        self.reads.borrow_mut().push(name.clone());

        let error = |set: &str, message: String| TokenError {
            token: name.clone(),
            set: set.to_string(),
            key: self.resolved.key.clone(),
            message,
        };

        let token = self
            .resolved
            .tokens
            .get(&name)
            .ok_or_else(|| error("", format!("the {reader} reader found no such token")))?;

        if !accepts.contains(&token.ty) {
            let names: Vec<&str> = accepts.iter().map(|t| t.name()).collect();
            return Err(error(
                &token.set,
                format!("expected {}, found {}", names.join(" or "), token.ty.name()),
            ));
        }

        Ok(&token.value)
    }

    fn px(&self, path: &str, accepts: &[TokenType], reader: &str) -> Result<f32, TokenError> {
        match self.get(path, accepts, reader)? {
            Value::Number(n) => Ok(n.value as f32),
            other => unreachable!("{path}: number types resolve to numbers, got {other:?}"),
        }
    }

    pub fn color(&self, path: &str) -> Result<Color, TokenError> {
        match self.get(path, &[TokenType::Color], "color")? {
            Value::Color(c) => Ok(c.to_color()),
            other => unreachable!("{path}: {other:?}"),
        }
    }

    pub fn spacing(&self, path: &str) -> Result<f32, TokenError> {
        self.px(path, &[TokenType::Spacing, TokenType::Dimension], "spacing")
    }

    pub fn sizing(&self, path: &str) -> Result<f32, TokenError> {
        self.px(path, &[TokenType::Sizing, TokenType::Dimension], "sizing")
    }

    pub fn radius(&self, path: &str) -> Result<f32, TokenError> {
        self.px(
            path,
            &[TokenType::BorderRadius, TokenType::Dimension],
            "radius",
        )
    }

    pub fn stroke_width(&self, path: &str) -> Result<f32, TokenError> {
        self.px(
            path,
            &[TokenType::BorderWidth, TokenType::Dimension],
            "stroke_width",
        )
    }

    pub fn font_size(&self, path: &str) -> Result<f32, TokenError> {
        Ok(self.px(path, &[TokenType::FontSizes], "font_size")? * self.resolved.text_scale)
    }

    pub fn font_family(&self, path: &str) -> Result<Vec<String>, TokenError> {
        match self.get(path, &[TokenType::FontFamilies], "font_family")? {
            Value::FontFamily(f) => Ok(f.clone()),
            other => unreachable!("{path}: {other:?}"),
        }
    }

    pub fn font_weight(&self, path: &str) -> Result<(u16, bool), TokenError> {
        match self.get(path, &[TokenType::FontWeights], "font_weight")? {
            Value::FontWeight { weight, italic } => Ok((*weight, *italic)),
            other => unreachable!("{path}: {other:?}"),
        }
    }

    pub fn letter_spacing(&self, path: &str) -> Result<f32, TokenError> {
        self.px(path, &[TokenType::LetterSpacing], "letter_spacing")
    }

    pub fn text_case(&self, path: &str) -> Result<TextCase, TokenError> {
        match self.get(path, &[TokenType::TextCase], "text_case")? {
            Value::TextCase(c) => Ok(*c),
            other => unreachable!("{path}: {other:?}"),
        }
    }

    pub fn text_decoration(&self, path: &str) -> Result<TextDecoration, TokenError> {
        match self.get(path, &[TokenType::TextDecoration], "text_decoration")? {
            Value::TextDecoration(d) => Ok(*d),
            other => unreachable!("{path}: {other:?}"),
        }
    }

    /// A type role, with its font size (and nothing else) times the text scale.
    pub fn typography(&self, path: &str) -> Result<Typography, TokenError> {
        match self.get(path, &[TokenType::Typography], "typography")? {
            Value::Typography(t) => Ok(Typography {
                size: t.size.map(|s| s * self.resolved.text_scale),
                ..t.clone()
            }),
            other => unreachable!("{path}: {other:?}"),
        }
    }

    pub fn opacity(&self, path: &str) -> Result<f32, TokenError> {
        self.px(path, &[TokenType::Opacity], "opacity")
    }

    pub fn rotation(&self, path: &str) -> Result<f32, TokenError> {
        self.px(path, &[TokenType::Rotation, TokenType::Number], "rotation")
    }

    pub fn number(&self, path: &str) -> Result<f32, TokenError> {
        self.px(path, &[TokenType::Number], "number")
    }

    /// Milliseconds, from a `number` token under `motion`.
    pub fn duration(&self, path: &str) -> Result<Duration, TokenError> {
        let ms = self.px(path, &[TokenType::Number], "duration")?;
        Ok(Duration::from_secs_f32(ms.max(0.0) / 1000.0))
    }

    pub fn shadow(&self, path: &str) -> Result<Vec<Shadow>, TokenError> {
        match self.get(path, &[TokenType::Shadow], "shadow")? {
            Value::Shadow(s) => Ok(s.clone()),
            other => unreachable!("{path}: {other:?}"),
        }
    }

    pub fn boolean(&self, path: &str) -> Result<bool, TokenError> {
        match self.get(path, &[TokenType::Boolean], "boolean")? {
            Value::Bool(b) => Ok(*b),
            other => unreachable!("{path}: {other:?}"),
        }
    }

    pub fn string(&self, path: &str) -> Result<String, TokenError> {
        match self.get(path, &[TokenType::String, TokenType::Other], "string")? {
            Value::Str(s) => Ok(s.clone()),
            other => unreachable!("{path}: {other:?}"),
        }
    }
}
