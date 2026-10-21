use std::sync::OnceLock;

use amane::Color;
use serde_json::Value;

const SOURCE: &str = include_str!("../../../design-tokens/menu.tokens.json");

#[derive(Clone, Copy)]
pub struct ThemeTokens {
    pub surface: Color,
    pub border: Color,
    pub fg: Color,
    pub muted: Color,
    pub hover_fill: Color,
    pub hover_destructive_fill: Color,
    pub selected_fill: Color,
    pub on_selected_fg: Color,
    pub accent: Color,
    pub separator: Color,
    pub destructive: Color,
}

pub struct Tokens {
    pub radius_card: f32,
    pub radius_row: f32,
    pub radius_submenu: f32,
    pub card_padding: f32,
    pub row_h: f32,
    pub gap: f32,
    pub row_height: f32,
    pub icon_slot: f32,
    pub chevron_slot: f32,
    pub separator: f32,
    pub separator_margin: f32,
    pub min_width: f32,
    pub max_width: f32,
    pub submenu_offset: f32,
    pub font_family: String,
    pub font_regular: u16,
    pub label_size: f32,
    pub shortcut_size: f32,
    pub shortcut_hover_alpha: f32,
    pub light: ThemeTokens,
    pub dark: ThemeTokens,
}

pub fn tokens() -> &'static Tokens {
    static CELL: OnceLock<Tokens> = OnceLock::new();
    // SOURCE is embedded at compile time, so a failure here is a token the
    // bundled JSON is missing or has in the wrong shape — a build-time bug.
    // Name the offending token so it is obvious which one to fix.
    CELL.get_or_init(|| parse(SOURCE).unwrap_or_else(|e| panic!("menu.tokens.json: {e}")))
}

fn parse(source: &str) -> Result<Tokens, String> {
    let root: Value = serde_json::from_str(source).map_err(|e| format!("invalid JSON: {e}"))?;

    let dim = |path: &str| dimension(&root, path).ok_or_else(|| format!("bad dimension {path}"));
    let num = |path: &str| number(&root, path).ok_or_else(|| format!("bad number {path}"));
    let weight = |path: &str| {
        number(&root, path)
            .map(|n| n as u16)
            .ok_or_else(|| format!("bad weight {path}"))
    };

    Ok(Tokens {
        radius_card: dim("radius.card")?,
        radius_row: dim("radius.row")?,
        radius_submenu: dim("radius.submenu")?,
        card_padding: dim("space.card-padding")?,
        row_h: dim("space.row-h")?,
        gap: dim("space.gap")?,
        row_height: dim("space.row-height")?,
        icon_slot: dim("space.icon-slot")?,
        chevron_slot: dim("space.chevron-slot")?,
        separator: dim("space.separator")?,
        separator_margin: dim("space.separator-margin")?,
        min_width: dim("space.min-width")?,
        max_width: dim("space.max-width")?,
        submenu_offset: dim("space.submenu-offset")?,
        font_family: font_family(&root, "font.ui.family").ok_or("bad token font.ui.family")?,
        font_regular: weight("font.ui.weight.regular")?,
        label_size: num("type.label.size")?,
        shortcut_size: num("type.shortcut.size")?,
        shortcut_hover_alpha: num("type.shortcut.hover-alpha")?,
        light: theme(&root, "theme.light")?,
        dark: theme(&root, "theme.dark")?,
    })
}

fn theme(root: &Value, prefix: &str) -> Result<ThemeTokens, String> {
    let c = |leaf: &str| {
        color(root, &format!("{prefix}.{leaf}")).ok_or_else(|| format!("bad color {prefix}.{leaf}"))
    };

    Ok(ThemeTokens {
        surface: c("surface")?,
        border: c("border")?,
        fg: c("fg")?,
        muted: c("muted")?,
        hover_fill: c("hover-fill")?,
        hover_destructive_fill: c("hover-destructive-fill")?,
        selected_fill: c("selected-fill")?,
        on_selected_fg: c("on-selected-fg")?,
        accent: c("accent")?,
        separator: c("separator")?,
        destructive: c("destructive")?,
    })
}

fn color(root: &Value, path: &str) -> Option<Color> {
    color_value(&resolve(root, &token_at(root, path)?)?)
}

fn dimension(root: &Value, path: &str) -> Option<f32> {
    dimension_value(&resolve(root, &token_at(root, path)?)?)
}

fn number(root: &Value, path: &str) -> Option<f32> {
    let v = resolve(root, &token_at(root, path)?)?;
    v.as_f64().map(|n| n as f32)
}

fn font_family(root: &Value, path: &str) -> Option<String> {
    let v = resolve(root, &token_at(root, path)?)?;

    if let Some(s) = v.as_str() {
        return Some(s.to_string());
    }

    v.as_array()?
        .iter()
        .find_map(|entry| entry.as_str().map(|s| s.to_string()))
}

fn token_at(root: &Value, path: &str) -> Option<Value> {
    let node = walk(root, path)?;
    node.get("$value").cloned()
}

fn walk<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;

    for part in path.split('.') {
        cur = cur.get(part)?;
    }

    Some(cur)
}

fn resolve(root: &Value, value: &Value) -> Option<Value> {
    let mut current = value.clone();

    for _ in 0..16 {
        let Some(s) = current.as_str() else {
            return Some(current);
        };

        if !(s.starts_with('{') && s.ends_with('}')) {
            return Some(current);
        }

        let inner = &s[1..s.len() - 1];
        current = token_at(root, inner)?;
    }

    None
}

fn color_value(v: &Value) -> Option<Color> {
    let comps = v.get("components")?.as_array()?;

    if comps.len() < 3 {
        return None;
    }

    let r = channel(&comps[0])?;
    let g = channel(&comps[1])?;
    let b = channel(&comps[2])?;

    let a = v
        .get("alpha")
        .and_then(|a| a.as_f64())
        .map(|a| (a.clamp(0.0, 1.0) * 255.0).round() as u8)
        .unwrap_or(255);

    Some(Color::rgba(r, g, b, a))
}

fn channel(v: &Value) -> Option<u8> {
    let n = v.as_f64()?.clamp(0.0, 1.0);
    Some((n * 255.0).round() as u8)
}

fn dimension_value(v: &Value) -> Option<f32> {
    v.get("value")?.as_f64().map(|n| n as f32)
}
