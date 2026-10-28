//! The Design Token Manual's rules (manual §13), as checks over a Penpot
//! export. `tokens check` prints them; the tests below fail on them.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::Value as Json;

use super::color::Rgba;
use super::envelope::{Library, TokenType};
use super::resolve::references;
use super::values::Value;
use super::{COMPONENTS, Key, Resolved, resolve_key};
use crate::theme::{Density, Mode};

/// Contrast floors over black and white wallpaper (manual §11.1). Set to
/// what the seeded design meets (weakest: dark popover text 3.46:1, dark
/// panel secondary 2.06:1); raise them toward 4.5 and 3.0 as the design
/// converges in Penpot.
pub const FG_FLOOR: f64 = 3.4;
pub const SECONDARY_FLOOR: f64 = 2.0;

/// The surfaces checked for contrast: (background, primary, secondary, the
/// niri material whose `tone` the compositor lays under it, if any, and
/// whether the labels on it flip with the backdrop, so either mode's set
/// may carry the contrast: Apple's glyphs "flip from light to dark,
/// mirroring the glass").
const SURFACES: &[(&str, &str, &str, Option<&str>, bool)] = &[
    // every material's tint, under the vibrant labels the material hands out
    (
        // Clear glass is dark whatever the theme, with white labels
        "sys.color.material.ultra-thin",
        "sys.color.vibrant.label.primary",
        "sys.color.vibrant.label.secondary",
        Some("material.glass-clear.tone"),
        true,
    ),
    (
        "sys.color.material.thin",
        "sys.color.vibrant.label.primary",
        "sys.color.vibrant.label.secondary",
        Some("material.glass.tone"),
        true,
    ),
    (
        "sys.color.material.regular",
        "sys.color.vibrant.label.primary",
        "sys.color.vibrant.label.secondary",
        Some("material.glass.tone"),
        true,
    ),
    (
        "sys.color.material.thick",
        "sys.color.vibrant.label.primary",
        "sys.color.vibrant.label.secondary",
        Some("material.glass-large.tone"),
        false,
    ),
    (
        "sys.color.material.ultra-thick",
        "sys.color.vibrant.label.primary",
        "sys.color.vibrant.label.secondary",
        None,
        false,
    ),
    // drawn by the window over niri's frost, which has no tint to tone toward
    (
        "sys.color.material.sidebar",
        "sys.color.vibrant.label.primary",
        "sys.color.vibrant.label.secondary",
        None,
        false,
    ),
];

/// Every rule that can fail, in manual §13 order.
pub fn check(lib: &Library, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    out.extend(themes(lib));
    let resolved = match resolve_all(lib) {
        Ok(r) => r,
        Err(errors) => {
            out.extend(errors);
            return out;
        }
    };
    out.extend(tiers(lib));
    out.extend(set_contents(lib));
    out.extend(naming(lib));
    out.extend(invariants(lib, &resolved));
    out.extend(contrast(&resolved));
    out.extend(type_roles(&resolved));
    out.extend(normalized(source));
    out
}

// ───── 1. themes ─────────────────────────────────────────────────────────────

pub fn themes(lib: &Library) -> Vec<String> {
    let mut out = Vec::new();

    for (group, names) in [
        ("mode", Mode::ALL.map(Mode::theme)),
        ("density", Density::ALL.map(Density::theme)),
    ] {
        for name in names {
            if lib.theme(group, name).is_none() {
                out.push(format!("manual §9.2: theme {group}/{name} is missing"));
            }
        }
    }

    for theme in &lib.themes {
        let id = format!("{}/{}", theme.group, theme.name);
        if theme.description.is_none() {
            out.push(format!(
                "manual §9.2: theme {id} has no description (Penpot's import needs one)"
            ));
        }
        for set in &theme.not_enabled {
            out.push(format!(
                "manual §9.2: theme {id} lists set {set} as not enabled; Penpot's import \
                 enables every listed set, so remove it from the theme instead"
            ));
        }
    }

    out
}

// ───── 2. resolves in every combination ──────────────────────────────────────

pub fn resolve_all(lib: &Library) -> Result<Vec<(Key, Resolved)>, Vec<String>> {
    let mut resolved = Vec::new();
    let mut errors = Vec::new();

    for key in Key::all() {
        match resolve_key(lib, key) {
            Ok(r) => resolved.push((key, r)),
            Err(e) => errors.extend(e.iter().map(|e| format!("manual §13.2: {e}"))),
        }
    }

    if errors.is_empty() {
        Ok(resolved)
    } else {
        Err(errors)
    }
}

// ───── 3. tier direction ─────────────────────────────────────────────────────

fn tier(name: &str) -> Option<&'static str> {
    let first = name.split('.').next().unwrap_or("");
    match first {
        "ref" => Some("ref"),
        "sys" => Some("sys"),
        _ => COMPONENTS
            .iter()
            .find(|c| **c == first)
            .map(|_| "component"),
    }
}

fn string_refs(json: &Json, out: &mut Vec<String>) {
    match json {
        Json::String(s) => out.extend(references(s).into_iter().map(String::from)),
        Json::Array(items) => items.iter().for_each(|i| string_refs(i, out)),
        Json::Object(map) => map.values().for_each(|v| string_refs(v, out)),
        _ => {}
    }
}

fn types_by_name(lib: &Library) -> HashMap<&str, TokenType> {
    lib.sets
        .iter()
        .flat_map(|s| &s.tokens)
        .map(|t| (t.name.as_str(), t.ty))
        .collect()
}

pub fn tiers(lib: &Library) -> Vec<String> {
    let types = types_by_name(lib);
    let mut out = Vec::new();

    for set in &lib.sets {
        for token in &set.tokens {
            let name = &token.name;
            let Some(own) = tier(name) else {
                out.push(format!(
                    "manual §3: {name} (set {}) has no tier prefix (ref., sys. or one of {})",
                    set.name,
                    COMPONENTS.join(", ")
                ));
                continue;
            };

            let mut refs = Vec::new();
            string_refs(&token.value, &mut refs);

            for target in refs {
                let allowed = match (own, tier(&target)) {
                    ("ref", _) => false,
                    ("sys", Some("ref" | "sys")) => true,
                    ("component", Some("sys")) => true,
                    ("component", Some("ref")) => {
                        types.get(target.as_str()) != Some(&TokenType::Color)
                    }
                    _ => false,
                };
                if !allowed {
                    let rule = match own {
                        "ref" => "primitives hold literals only",
                        "sys" => "semantic tokens reference only ref.* and sys.*",
                        _ => "component tokens reference only sys.* and non-colour ref.*",
                    };
                    out.push(format!(
                        "manual §3: {name} (set {}) references {{{target}}}: {rule}",
                        set.name
                    ));
                }
            }
        }
    }

    out
}

// ───── 4. set contents ───────────────────────────────────────────────────────

pub fn set_contents(lib: &Library) -> Vec<String> {
    let mut out = Vec::new();

    let allowed = |set: &str, name: &str| -> Option<bool> {
        Some(match set {
            "ref" => name.starts_with("ref."),
            "sys/base" => {
                name.starts_with("sys.")
                    && !name.starts_with("sys.color.")
                    && !name.starts_with("sys.density.")
            }
            "sys/color/light" | "sys/color/dark" => name.starts_with("sys.color."),
            "sys/density/comfortable" | "sys/density/compact" => name == "sys.density.factor",
            "components" => tier(name) == Some("component"),
            s if s.starts_with("accent/") => name.starts_with("ref.color.accent."),
            _ => return None,
        })
    };

    for set in &lib.sets {
        for token in &set.tokens {
            match allowed(&set.name, &token.name) {
                None => {
                    out.push(format!(
                        "manual §9.1: set {} is not one of the manual's sets",
                        set.name
                    ));
                    break;
                }
                Some(false) => out.push(format!(
                    "manual §9.1: set {} may not contain {}",
                    set.name, token.name
                )),
                Some(true) => {}
            }
        }
    }

    let names = |set: &str| -> BTreeSet<&str> {
        lib.sets
            .iter()
            .filter(|s| s.name == set)
            .flat_map(|s| s.tokens.iter().map(|t| t.name.as_str()))
            .collect()
    };
    let (light, dark) = (names("sys/color/light"), names("sys/color/dark"));
    for only in light.symmetric_difference(&dark) {
        out.push(format!(
            "manual §8.1: {only} is in only one of sys/color/light and sys/color/dark"
        ));
    }

    // each mode needs the primitives, the base, its colours and the
    // components; each density its own set
    let mode_sets = Mode::ALL.map(|m| {
        let color = format!("sys/color/{}", m.theme());
        (
            "mode",
            m.theme(),
            vec!["ref".into(), "sys/base".into(), color, "components".into()],
        )
    });
    let density_sets = Density::ALL.map(|d| {
        (
            "density",
            d.theme(),
            vec![format!("sys/density/{}", d.theme())],
        )
    });
    for (group, theme, needs) in mode_sets.into_iter().chain(density_sets) {
        let Some(t) = lib.theme(group, theme) else {
            continue;
        };
        for set in &needs {
            if !t.sets.iter().any(|s| s == set) {
                out.push(format!(
                    "manual §9.2: theme {group}/{theme} must enable set {set}"
                ));
            }
        }
    }

    out
}

// ───── 6. naming ─────────────────────────────────────────────────────────────

fn segment_ok(s: &str) -> bool {
    !s.is_empty()
        && s.split('-').all(|w| {
            !w.is_empty()
                && w.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

pub fn naming(lib: &Library) -> Vec<String> {
    let mut out = Vec::new();

    for token in lib.sets.iter().flat_map(|s| &s.tokens) {
        let name = &token.name;
        if !name.split('.').all(segment_ok) {
            out.push(format!(
                "manual §4.1: {name} — lowercase words, dots between levels, hyphens inside"
            ));
        }

        let parts: Vec<&str> = name.split('.').collect();
        if let ["ref", "color", scale, step] = parts.as_slice()
            && *scale != "accent"
        {
            let Ok(want) = step.parse::<f64>() else {
                out.push(format!("manual §5: {name} — a palette step is its CIE L*"));
                continue;
            };
            let Some(lightness) = token
                .value
                .as_str()
                .and_then(|v| super::color::parse(v).ok())
                .map(Rgba::lightness)
            else {
                continue;
            };
            if (lightness - want).abs() > 2.0 {
                out.push(format!(
                    "manual §5: {name} has lightness {lightness:.0}, more than 2 from its name"
                ));
            }
        }
    }

    out
}

// ───── 7. invariants ─────────────────────────────────────────────────────────

/// Tokens whose value may depend on `sys.density.factor`.
fn density_scaled(lib: &Library) -> HashSet<String> {
    dependents(lib, &["sys.density.factor"])
}

/// `roots` and every token that references one of them, directly or not.
pub fn dependents(lib: &Library, roots: &[&str]) -> HashSet<String> {
    let mut refs: HashMap<&str, Vec<String>> = HashMap::new();
    for token in lib.sets.iter().flat_map(|s| &s.tokens) {
        string_refs(&token.value, refs.entry(&token.name).or_default());
    }

    let mut scaled: HashSet<String> = roots.iter().map(|r| r.to_string()).collect();
    loop {
        let before = scaled.len();
        for (name, targets) in &refs {
            if targets.iter().any(|t| scaled.contains(t)) {
                scaled.insert(name.to_string());
            }
        }
        if scaled.len() == before {
            return scaled;
        }
    }
}

fn geometry_fixed(name: &str, ty: TokenType) -> bool {
    matches!(
        ty,
        TokenType::BorderRadius
            | TokenType::BorderWidth
            | TokenType::FontSizes
            | TokenType::Typography
    ) || name.starts_with("sys.size.icon.")
}

pub fn invariants(lib: &Library, resolved: &[(Key, Resolved)]) -> Vec<String> {
    let scaled = density_scaled(lib);
    let mut out = BTreeSet::new();
    let get = |m: Mode, d: Density| {
        &resolved
            .iter()
            .find(|(k, _)| k.mode == m && k.density == d)
            .expect("every combination resolved")
            .1
    };

    for d in Density::ALL {
        let (light, dark) = (get(Mode::Light, d), get(Mode::Dark, d));
        for (name, token) in &light.tokens {
            if token.ty != TokenType::Color
                && dark.tokens.get(name).map(|t| &t.value) != Some(&token.value)
            {
                out.insert(format!(
                    "manual §1.4: {name} changes with mode, but only colours may"
                ));
            }
        }
    }

    for m in Mode::ALL {
        let (comfortable, compact) = (get(m, Density::Comfortable), get(m, Density::Compact));
        for (name, token) in &comfortable.tokens {
            if name == "sys.density.factor"
                || compact.tokens.get(name).map(|t| &t.value) == Some(&token.value)
            {
                continue;
            }
            if geometry_fixed(name, token.ty) || token.ty == TokenType::Color {
                out.insert(format!(
                    "manual §8.2: {name} changes with density, but radii, strokes, icon \
                     sizes, font sizes and colours don't"
                ));
            } else if !scaled.contains(name) {
                out.insert(format!(
                    "manual §8.2: {name} changes with density without referencing \
                     sys.density.factor"
                ));
            }
        }
    }

    out.into_iter().collect()
}

// ───── 8. contrast ───────────────────────────────────────────────────────────

/// The lowest contrast of `fg` on `bg` over black and over white wallpaper.
pub fn worst_contrast(bg: Rgba, fg: Rgba) -> f64 {
    toned_contrast(bg, &[fg], 1.0)
}

/// [`worst_contrast`] with the wallpaper toned as niri's material `tone`
/// does, squeezed toward the tint's side, and any of `fgs` to read with:
/// labels that flip with the backdrop take the better one over each
/// wallpaper.
pub fn toned_contrast(bg: Rgba, fgs: &[Rgba], tone: f64) -> f64 {
    let dark = bg.luminance() < Rgba::opaque(0.5, 0.5, 0.5).luminance();
    [Rgba::BLACK, Rgba::WHITE]
        .into_iter()
        .map(|wallpaper| {
            let c = |c: f64| {
                if dark {
                    c * tone
                } else {
                    1.0 - (1.0 - c) * tone
                }
            };
            let toned = Rgba::opaque(c(wallpaper.r), c(wallpaper.g), c(wallpaper.b));
            let surface = bg.over(toned);
            fgs.iter()
                .map(|fg| fg.over(surface).contrast(surface))
                .fold(0.0, f64::max)
        })
        .fold(f64::INFINITY, f64::min)
}

pub fn contrast(resolved: &[(Key, Resolved)]) -> Vec<String> {
    let mut out = Vec::new();

    for (key, r) in resolved
        .iter()
        .filter(|(k, _)| k.density == Density::Comfortable)
    {
        let color = |name: &str| match r.tokens.get(name).map(|t| &t.value) {
            Some(Value::Color(c)) => Some(*c),
            _ => None,
        };
        // the other mode's labels, for the surfaces whose labels flip
        let flipped = resolved
            .iter()
            .find(|(k, _)| k.density == key.density && k.mode != key.mode)
            .map(|(_, r)| r);
        let flipped_color = |name: &str| match flipped.and_then(|r| r.tokens.get(name)) {
            Some(token) => match &token.value {
                Value::Color(c) => Some(*c),
                _ => None,
            },
            None => None,
        };

        for (bg, fg, secondary, tone, adaptive) in SURFACES {
            let Some(bg_c) = color(bg) else {
                out.push(format!("manual §6.1: {bg} is missing or not a colour"));
                continue;
            };
            let tone = match tone.map(|name| r.tokens.get(name).map(|t| &t.value)) {
                None => 1.0,
                Some(Some(Value::Number(n))) => n.value,
                Some(_) => {
                    out.push(format!(
                        "manual §6.1: {} is missing or not a number",
                        tone.unwrap()
                    ));
                    continue;
                }
            };
            // with niri's material: toned; without it, the kit thickens the tint
            let untoned = Rgba {
                a: bg_c.a + (1.0 - bg_c.a) * f64::from(crate::material::UNTONED),
                ..bg_c
            };
            for (content, floor) in [(fg, FG_FLOOR), (secondary, SECONDARY_FLOOR)] {
                let Some(c) = color(content) else {
                    out.push(format!("manual §6.1: {content} is missing or not a colour"));
                    continue;
                };
                // flipping labels take whichever mode's set reads better
                let mut fgs = vec![c];
                if *adaptive {
                    fgs.extend(flipped_color(content));
                }
                for (how, worst) in [
                    ("toned", toned_contrast(bg_c, &fgs, tone)),
                    (
                        "without niri's material",
                        toned_contrast(untoned, &fgs, 1.0),
                    ),
                ] {
                    if worst < floor {
                        out.push(format!(
                            "manual §11.1: [{}] {content} on {bg} ({how}) reaches {worst:.2}:1 \
                             over black or white wallpaper; the floor is {floor}:1",
                            key.mode.theme()
                        ));
                    }
                }
            }
        }
    }

    out
}

// ───── type roles ─────────────────────────────────────────────────────────────

/// Code sets family, size and weight from a type role, so each role must
/// define all three (manual §6.3).
pub fn type_roles(resolved: &[(Key, Resolved)]) -> Vec<String> {
    let Some((_, r)) = resolved.first() else {
        return Vec::new();
    };
    let mut out: Vec<String> = r
        .tokens
        .iter()
        .filter_map(|(name, token)| match &token.value {
            Value::Typography(t)
                if t.families.is_empty() || t.size.is_none() || t.weight.is_none() =>
            {
                Some(format!(
                    "manual §6.3: {name} must set fontFamilies, fontSizes and fontWeights"
                ))
            }
            _ => None,
        })
        .collect();
    out.sort();
    out
}

// ───── 9. hygiene ────────────────────────────────────────────────────────────

pub fn normalized(source: &str) -> Vec<String> {
    match super::check::normalize(source) {
        Ok(n) if n == source => Vec::new(),
        Ok(_) => vec![String::from(
            "manual §10.3: the file isn't normalized — run `amane-shell tokens normalize`",
        )],
        Err(e) => vec![e],
    }
}

// ───── 11. report ────────────────────────────────────────────────────────────

/// Token counts per tier, against the manual's budget (§3).
pub fn tier_counts(lib: &Library) -> [(&'static str, usize, usize); 3] {
    let names: BTreeSet<&str> = lib
        .sets
        .iter()
        .flat_map(|s| &s.tokens)
        .map(|t| t.name.as_str())
        .collect();
    let count = |t: &str| names.iter().filter(|n| tier(n) == Some(t)).count();

    [
        ("ref", count("ref"), 60),
        ("sys", count("sys"), 70),
        ("component", count("component"), 15),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{GENERATED, SOURCE, library};

    fn assert_none(rule: &str, violations: Vec<String>) {
        assert!(
            violations.is_empty(),
            "{rule}:\n  {}",
            violations.join("\n  ")
        );
    }

    #[test]
    fn export_parses_with_required_themes() {
        assert_none("themes", themes(library()));
    }

    #[test]
    fn resolves_in_every_combination() {
        if let Err(errors) = resolve_all(library()) {
            assert_none("resolution", errors);
        }
    }

    #[test]
    fn tier_direction() {
        assert_none("tiers", tiers(library()));
    }

    #[test]
    fn set_contents_match_the_manual() {
        assert_none("set contents", set_contents(library()));
    }

    #[test]
    fn names_follow_the_grammar() {
        assert_none("naming", naming(library()));
    }

    #[test]
    fn geometry_and_density_invariants() {
        let resolved = resolve_all(library()).expect("resolves");
        assert_none("invariants", invariants(library(), &resolved));
    }

    #[test]
    fn contrast_floors() {
        let resolved = resolve_all(library()).expect("resolves");
        assert_none("contrast", contrast(&resolved));
    }

    #[test]
    fn type_roles_are_complete() {
        let resolved = resolve_all(library()).expect("resolves");
        assert_none("type roles", type_roles(&resolved));
    }

    #[test]
    fn hygiene() {
        assert_none("hygiene", normalized(SOURCE));
    }

    #[test]
    fn generated_tokens_resolve_with_their_type() {
        for key in Key::all() {
            let r = resolve_key(library(), key).expect("resolves");
            for (name, ty, _) in GENERATED {
                let want = TokenType::from_name(ty).expect("generated from a known type");
                match r.tokens.get(*name) {
                    Some(token) if token.ty == want => {}
                    Some(token) => panic!(
                        "manual §13.5: {name} is {} in {}, but the code was generated for {ty}",
                        token.ty.name(),
                        r.key
                    ),
                    None => panic!("manual §13.5: {name} is missing in {}", r.key),
                }
            }
        }
    }

    /// Source files and the component whose tokens they may read, besides `sys`.
    /// The shared components (`buttons/`, `card.rs`, `list.rs`, `layout.rs` …)
    /// are left out on purpose: any component can use them, so they read
    /// `sys` only (manual §10.4).
    const OWNERS: &[(&str, &str)] = &[
        ("status_bar/", "bar"),
        ("context_menu/", "menu"),
        ("notifications/", "notification"),
        ("quick_settings/", "quick"),
        // the kit's dialog, and the shell's alert that shows it
        ("containers/dialog.rs", "dialog"),
        ("alert/", "dialog"),
    ];

    fn component_sources() -> Vec<(String, String)> {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }

        // the shell's surfaces, and the shared pieces of the shell and the kit,
        // which belong to no component
        let kit = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let roots = [
            kit.join("../src/surfaces"),
            kit.join("../src/components"),
            kit.join("src"),
        ];

        let mut sources = Vec::new();
        for root in roots {
            let mut files = Vec::new();
            walk(&root, &mut files);
            for f in files {
                let rel = f
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                // the token system itself is no component
                if rel.starts_with("tokens/") {
                    continue;
                }
                sources.push((rel, std::fs::read_to_string(&f).unwrap()));
            }
        }
        sources
    }

    #[test]
    fn components_read_only_sys_and_their_own_tokens() {
        for (file, source) in component_sources() {
            let own = OWNERS
                .iter()
                .find(|(prefix, _)| file.starts_with(prefix))
                .map(|(_, c)| *c);

            // every `crate::tokens::…` path or import list, by its first segment
            let mut used = BTreeSet::new();
            for part in source.split("crate::tokens::").skip(1) {
                let names = match part.strip_prefix('{') {
                    Some(list) => list.split('}').next().unwrap_or(""),
                    // one path: its first segment alone, not the text after it
                    None => part
                        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .next()
                        .unwrap_or(""),
                };
                for name in names.split(',') {
                    let name: String = name
                        .trim()
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    used.insert(name);
                }
            }

            for name in used.iter().filter(|n| COMPONENTS.contains(&n.as_str())) {
                assert_eq!(
                    Some(name.as_str()),
                    own,
                    "manual §3: src/components/{file} reads {name}.* tokens; code reads only \
                     sys.* and its own component's tokens"
                );
            }
        }
    }

    /// A generated function's call as it appears in code: the last two
    /// segments of its path, e.g. `popover::bg(`.
    fn call_suffix(path: &str) -> String {
        let parts: Vec<&str> = path.rsplit("::").take(2).collect();
        format!("{}::{}(", parts[1], parts[0])
    }

    /// Report only: tokens no code reads, and tier sizes against the budget.
    #[test]
    fn report() {
        let code: String = component_sources().into_iter().map(|(_, s)| s).collect();
        let unread: Vec<&str> = GENERATED
            .iter()
            .filter(|(name, _, path)| {
                *name != "sys.density.factor" && !code.contains(&call_suffix(path))
            })
            .map(|(name, _, _)| *name)
            .collect();

        eprintln!("tokens no code reads ({}):", unread.len());
        for name in unread {
            eprintln!("  {name}");
        }
        for (tier, count, budget) in tier_counts(library()) {
            eprintln!("{tier}: {count} tokens (budget about {budget})");
        }
    }

    #[test]
    fn rules_catch_breakage() {
        // a sys token referencing a component, and a component token in a colour set
        let lib = Library::parse(
            r##"{
                "sys/color/light": { "sys": { "color": { "x": { "$type": "color", "$value": "#000" } } },
                                     "bar": { "theme": { "surface": { "$type": "color", "$value": "#fff" } } } },
                "sys/base": { "sys": { "a": { "$type": "number", "$value": "{bar.b}" } } },
                "ref": { "ref": { "color": { "neutral": { "50": { "$type": "color", "$value": "#000" } } } } }
            }"##,
        )
        .unwrap();
        assert!(!tiers(&lib).is_empty());
        assert!(!set_contents(&lib).is_empty());
        assert!(!naming(&lib).is_empty());
        assert!(!themes(&lib).is_empty());
    }
}
