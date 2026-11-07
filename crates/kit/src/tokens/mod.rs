//! Design tokens from Penpot (`design-tokens/tokens.json`), resolved the way
//! Penpot resolves them. Code reads them through functions generated from
//! the export (`build.rs`), named after the tokens:
//!
//! ```ignore
//! use crate::tokens::{Styled, menu, sys};
//!
//! Rectangle::new()
//!     .fill(sys::color::surface::popover::bg())
//!     .radius(sys::radius::popover())
//!     .height(menu::row::height());
//! Text::new("Open").style(sys::r#type::label());
//! ```
//!
//! Each returns the value for the current mode, density, accent and text
//! scale. There is no `ref` module: code never reads primitives. The rules
//! live in `docs/design-system/tokens.md`.

pub mod accent;
/// The manual's rules over a token file, and the `tokens check` / `tokens normalize` tools
/// (`cargo xtask`); only built with the `tools` feature, which the shell leaves out.
#[cfg(any(test, feature = "tools"))]
pub mod check;
#[cfg(test)]
mod codegen;
pub mod color;
pub mod envelope;
pub mod error;
pub mod expr;
pub mod font;
#[cfg(any(test, feature = "tools"))]
pub mod lint;
pub mod resolve;
pub mod values;

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use amane::{Case, Decoration, Text};

use crate::theme::{self, Density, Mode};
pub use envelope::{Library, TokenType};
pub use error::TokenError;
pub use values::{Shadow, TextCase, TextDecoration, Value};

/// The committed Penpot export.
pub const SOURCE: &str = include_str!("../../design-tokens/tokens.json");

/// The prefixes component tokens may use (manual §3).
pub const COMPONENTS: &[&str] = &[
    "bar",
    "menu",
    "wifi",
    "power",
    "osd",
    "notification",
    "quick",
    "dialog",
    // niri's materials, for the config generator (src/tokens/layer_rules.rs)
    "material",
];

/// One token after resolution: its type and its final value.
#[derive(Clone, Debug)]
pub struct ResolvedToken {
    /// The token's `$type`.
    pub ty: TokenType,
    /// Its value with every reference substituted.
    pub value: Value,
}

/// Every active token for one selection, resolved.
#[derive(Debug)]
pub struct Resolved {
    /// The themes it was resolved for, like `mode/dark + density/comfortable`, for messages.
    pub key: String,
    /// Every active token by its dotted name.
    pub tokens: HashMap<String, ResolvedToken>,
    /// Multiplies font sizes when read (manual §8.4).
    pub text_scale: f32,
}

/// What the runtime varies on top of Penpot. Resolution is cached per key.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Key {
    /// The light or dark theme.
    pub mode: Mode,
    /// The density theme.
    pub density: Density,
    /// The system accent as 8-bit RGB, replacing `ref.color.accent.*`.
    pub accent: Option<[u8; 3]>,
    /// Text scale in hundredths.
    pub text_scale: u16,
}

thread_local! {
    static OVERRIDE: Cell<Option<Key>> = const { Cell::new(None) };
}

/// Run `f` with the generated token functions reading `key` instead of the
/// runtime's inputs (tests, and tools that need the default look).
pub fn with_key<T>(key: Key, f: impl FnOnce() -> T) -> T {
    let before = OVERRIDE.with(|o| o.replace(Some(key)));
    let out = f();
    OVERRIDE.with(|o| o.set(before));
    out
}

impl Key {
    /// Every `mode` × `density` combination, with the default accent and
    /// text scale.
    pub fn all() -> impl Iterator<Item = Key> {
        Mode::ALL
            .into_iter()
            .flat_map(|m| Density::ALL.into_iter().map(move |d| Key::new(m, d)))
    }

    /// The runtime inputs as they are now.
    pub fn current() -> Self {
        if let Some(key) = OVERRIDE.with(Cell::get) {
            return key;
        }

        Self {
            accent: theme::accent().map(|c| [c.red(), c.green(), c.blue()]),
            text_scale: (theme::text_scale() * 100.0).round() as u16,
            ..Self::new(theme::mode(), theme::density())
        }
    }

    /// `mode` and `density` with no accent and the text at 100 %.
    pub fn new(mode: Mode, density: Density) -> Self {
        Self {
            mode,
            density,
            accent: None,
            text_scale: 100,
        }
    }
}

/// The bundled export, parsed once.
pub fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    // SOURCE is embedded, and the contract tests parse it in every
    // combination, so a failure here can't reach a passing build.
    LIBRARY.get_or_init(|| {
        Library::parse(SOURCE).unwrap_or_else(|e| panic!("design-tokens/tokens.json: {e}"))
    })
}

/// Resolve the bundled tokens for `key`, once per key.
/// The table the generated functions read now. Working out the [`Key`] reads the appearance
/// settings and service, so it's kept per thread and worked out again only after one of
/// them changed; such a change redraws every window, since a view that only reads tokens
/// hasn't read them itself.
fn current() -> &'static Resolved {
    use std::sync::Once;
    use std::sync::atomic::{AtomicU64, Ordering};

    use amane::{Appearance, Config, Service};

    static GENERATION: AtomicU64 = AtomicU64::new(1);
    static WATCH: Once = Once::new();
    thread_local! {
        static CURRENT: Cell<Option<(u64, &'static Resolved)>> = const { Cell::new(None) };
    }

    if let Some(key) = OVERRIDE.with(Cell::get) {
        return resolved(key);
    }

    WATCH.call_once(|| {
        let changed = || {
            GENERATION.fetch_add(1, Ordering::Release);
            amane::mark_all();
        };
        Config::<crate::settings::Appearance>::subscribe(move |_| changed()).forever();
        Appearance::subscribe(move |_| changed()).forever();
    });

    let generation = GENERATION.load(Ordering::Acquire);
    if let Some((seen, table)) = CURRENT.with(Cell::get)
        && seen == generation
    {
        return table;
    }
    let table = resolved(Key::current());
    CURRENT.with(|current| current.set(Some((generation, table))));
    table
}

/// Resolve the bundled tokens for `key`, once per key; the table lives for the rest of the run.
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

/// Resolve `library` for `key`: select its mode and density sets, put the accent's derived
/// colors in place of `ref.color.accent.*`, and set the text scale.
pub fn resolve_key(library: &Library, key: Key) -> Result<Resolved, Vec<TokenError>> {
    let overrides: Vec<(&str, String)> = key
        .accent
        .map(|[r, g, b]| accent::derive(color::Rgba::from_rgb8([r, g, b])).to_vec())
        .unwrap_or_default();

    let mut resolved = library
        .select(key.mode, key.density)
        .map_err(|e| vec![e])?
        .resolve(&overrides)?;
    resolved.text_scale = key.text_scale as f32 / 100.0;
    Ok(resolved)
}

#[allow(dead_code, clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/tokens.rs"));
}
pub use generated::*;

/// A `sys.type.*` role, ready for a `Text`: the first installed family of
/// its stack, and its size and letter spacing times the text scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// The first installed family of the stack.
    pub family: &'static str,
    /// Text size in logical pixels, times the text scale.
    pub size: f32,
    /// Weight, 100 to 950; 400 is regular.
    pub weight: u16,
    /// Whether the style is italic.
    pub italic: bool,
    /// A multiple of the size.
    pub line_height: Option<f32>,
    /// Pixels after every letter.
    pub letter_spacing: Option<f32>,
    /// How the case of the letters changes; `None` keeps amane's default.
    pub text_case: Option<TextCase>,
    /// A line drawn through or under the text; `None` keeps amane's default.
    pub text_decoration: Option<TextDecoration>,
}

/// `Text::new(…).style(sys::r#type::label())` sets everything a Penpot
/// typography holds: family, size, weight, italic, line height, letter
/// spacing, case and decoration. Members the typography leaves out keep
/// amane's defaults (the font's own line spacing, no tracking).
pub trait Styled {
    /// Sets the text's family, size, weight, italic and whatever else `style` holds.
    fn style(self, style: TextStyle) -> Self;
}

impl Styled for Text {
    fn style(self, style: TextStyle) -> Self {
        let mut text = self
            .font(style.family)
            .size(style.size)
            .weight(style.weight)
            .italic(style.italic);
        if let Some(multiple) = style.line_height {
            text = text.line_height(multiple);
        }
        if let Some(pixels) = style.letter_spacing {
            text = text.letter_spacing(pixels);
        }
        if let Some(case) = style.text_case {
            text = text.case(match case {
                TextCase::None => Case::AsWritten,
                TextCase::Uppercase => Case::Upper,
                TextCase::Lowercase => Case::Lower,
                TextCase::Capitalize => Case::Capitalize,
            });
        }
        if let Some(decoration) = style.text_decoration {
            text = text.decoration(match decoration {
                TextDecoration::None => Decoration::None,
                TextDecoration::Underline => Decoration::Underline,
                TextDecoration::StrikeThrough => Decoration::StrikeThrough,
            });
        }
        text
    }
}

/// What the generated functions call: the token's value under the current
/// [`Key`]. Every generated name resolves with its generated type in every
/// combination (the contract tests check it), so the panics are unreachable
/// in a passing build. There is one function per Penpot type, used or not.
#[allow(dead_code)]
pub mod read {
    use amane::Color;

    use super::{Resolved, Shadow, TextCase, TextDecoration, TextStyle, Value, font};

    fn get(name: &str) -> (&'static Resolved, &'static Value) {
        let r = super::current();
        let token = r
            .tokens
            .get(name)
            .unwrap_or_else(|| panic!("token {name} is not in the active sets ({})", r.key));
        (r, &token.value)
    }

    fn wrong(name: &str, value: &Value) -> ! {
        panic!("token {name} resolved to {value:?}, not the type it was generated with")
    }

    /// A `color` token.
    pub fn color(name: &str) -> Color {
        match get(name).1 {
            Value::Color(c) => c.to_color(),
            other => wrong(name, other),
        }
    }

    /// A number-like token, in its own units (pixels for lengths).
    pub fn number(name: &str) -> f32 {
        match get(name).1 {
            Value::Number(n) => n.value as f32,
            other => wrong(name, other),
        }
    }

    /// A `fontSizes` token, times the text scale.
    pub fn font_size(name: &str) -> f32 {
        let (r, value) = get(name);
        match value {
            Value::Number(n) => n.value as f32 * r.text_scale,
            other => wrong(name, other),
        }
    }

    /// A `fontFamilies` token: the first installed family of its stack.
    pub fn font_family(name: &str) -> &'static str {
        match get(name).1 {
            Value::FontFamily(stack) => font::first_installed(stack),
            other => wrong(name, other),
        }
    }

    /// A `fontWeights` token's weight, without its italic.
    pub fn font_weight(name: &str) -> u16 {
        match get(name).1 {
            Value::FontWeight { weight, .. } => *weight,
            other => wrong(name, other),
        }
    }

    /// A `typography` token as a [`TextStyle`], scaled by the text scale.
    pub fn text_style(name: &str) -> TextStyle {
        let (r, value) = get(name);
        let Value::Typography(t) = value else {
            wrong(name, value)
        };
        // the manual's rules require all three members (manual §6.3)
        let (weight, italic) = t.weight.unwrap_or((400, false));
        TextStyle {
            family: font::first_installed(&t.families),
            size: t.size.unwrap_or_default() * r.text_scale,
            weight,
            italic,
            line_height: t.line_height,
            letter_spacing: t.letter_spacing.map(|pixels| pixels * r.text_scale),
            text_case: t.text_case,
            text_decoration: t.text_decoration,
        }
    }

    /// A `shadow` token's layers.
    pub fn shadow(name: &str) -> Vec<Shadow> {
        match get(name).1 {
            Value::Shadow(s) => s.clone(),
            other => wrong(name, other),
        }
    }

    /// A `boolean` token.
    pub fn boolean(name: &str) -> bool {
        match get(name).1 {
            Value::Bool(b) => *b,
            other => wrong(name, other),
        }
    }

    /// A `string` or `other` token.
    pub fn string(name: &str) -> String {
        match get(name).1 {
            Value::Str(s) => s.clone(),
            other => wrong(name, other),
        }
    }

    /// A `textCase` token.
    pub fn text_case(name: &str) -> TextCase {
        match get(name).1 {
            Value::TextCase(c) => *c,
            other => wrong(name, other),
        }
    }

    /// A `textDecoration` token.
    pub fn text_decoration(name: &str) -> TextDecoration {
        match get(name).1 {
            Value::TextDecoration(d) => *d,
            other => wrong(name, other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dark(accent: Option<[u8; 3]>, text_scale: u16) -> Resolved {
        let key = Key {
            accent,
            text_scale,
            ..Key::new(Mode::Dark, Density::Comfortable)
        };
        resolve_key(library(), key).expect("resolves")
    }

    #[test]
    fn default_accent_is_the_derivation_of_its_seed() {
        let raw = |name: &str| {
            library()
                .sets
                .iter()
                .flat_map(|s| &s.tokens)
                .find(|t| t.name == name)
                .and_then(|t| t.value.as_str())
                .map(|v| color::parse(v).unwrap().to_color())
                .unwrap_or_else(|| panic!("{name} missing"))
        };
        let seed = raw(accent::SEED);
        let seed = color::Rgba::from_rgb8([seed.red(), seed.green(), seed.blue()]);

        for (name, derived) in accent::derive(seed) {
            let (want, got) = (raw(name), color::parse(&derived).unwrap().to_color());
            assert!(
                color::byte_distance(want, got) <= 1,
                "{name}: Penpot has {}, the runtime derives {derived}",
                want.hex()
            );
        }
    }

    #[test]
    fn a_system_accent_changes_exactly_the_accent_dependents() {
        let base = dark(None, 100);
        let yellow = dark(Some([0xc8, 0x88, 0x00]), 100);
        let dependents = lint::dependents(
            library(),
            &[accent::SEED, accent::ON_SEED, accent::DIM, accent::BRIGHT],
        );

        let changed: Vec<&String> = base
            .tokens
            .keys()
            .filter(|n| base.tokens[*n].value != yellow.tokens[*n].value)
            .collect();

        for name in &changed {
            assert!(dependents.contains(*name), "{name} changed with the accent");
        }
        for role in ["sys.color.accent.bg", "sys.color.accent.text"] {
            assert!(
                changed.iter().any(|n| *n == role),
                "{role} didn't follow the accent"
            );
        }
    }

    #[test]
    fn compact_tightens_spacing_and_rows_only() {
        let read = |density| {
            with_key(Key::new(Mode::Dark, density), || {
                (
                    [
                        sys::space::gap::m(),
                        sys::space::inset::l(),
                        sys::size::row::height(),
                    ],
                    [
                        sys::size::icon::m(),
                        sys::radius::popover(),
                        sys::r#type::label().size,
                    ],
                    menu::row::height(),
                    sys::size::control::height(),
                )
            })
        };
        let (scaled_a, fixed_a, menu_a, _) = read(Density::Comfortable);
        let (scaled_b, fixed_b, menu_b, control_b) = read(Density::Compact);

        for (a, b) in scaled_a.iter().zip(&scaled_b) {
            assert!(b < a, "{b} isn't tighter than {a}");
            // Penpot rounds density-scaled values, so the shell lands on whole pixels
            assert_eq!(*b, b.round());
        }
        assert_eq!(fixed_a, fixed_b);
        assert_eq!((menu_a, menu_b), (24.0, 21.0));
        assert_eq!(control_b, scaled_b[2] - 10.0);
    }

    #[test]
    fn text_scale_multiplies_font_sizes_only() {
        let read = |text_scale| {
            let key = Key {
                text_scale,
                ..Key::new(Mode::Dark, Density::Comfortable)
            };
            with_key(key, || {
                (
                    [
                        sys::r#type::label().size,
                        sys::r#type::title().size,
                        sys::r#type::numeric().size,
                        sys::r#type::caption().size,
                    ],
                    [
                        sys::size::icon::m(),
                        sys::size::row::height(),
                        sys::space::gap::m(),
                        menu::row::height(),
                    ],
                )
            })
        };
        let ((text_a, other_a), (text_b, other_b)) = (read(100), read(125));

        for (a, b) in text_a.iter().zip(&text_b) {
            assert_eq!(a * 1.25, *b);
        }
        assert_eq!(other_a, other_b);

        // the resolved values themselves don't change; only font-size reads do
        let (one, big) = (dark(None, 100), dark(None, 125));
        for (name, token) in &one.tokens {
            assert_eq!(token.value, big.tokens[name].value, "{name}");
        }
    }
}
