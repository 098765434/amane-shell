use std::sync::OnceLock;

use amane::Color;
use serde_json::Value;

const SOURCE: &str = include_str!("../../../design-tokens/bar.tokens.json");

#[derive(Clone, Copy)]
pub struct ThemeTokens {
    pub surface: Color,
    pub fg: Color,
    pub fg_strong: Color,
    pub pill: Color,
    pub pill_focused: Color,
    pub accent: Color,
    pub urgent: Color,
}

pub struct Tokens {
    pub height_compact: f32,
    pub height_overview: f32,
    pub radius_panel: f32,
    pub radius_pill: f32,
    pub radius_tile: f32,
    pub panel_pad_x: f32,
    pub panel_pad_y: f32,
    pub section_gap: f32,
    pub item_gap: f32,
    pub pill_pad_x: f32,
    pub pill_pad_y: f32,
    pub dot_size: f32,
    pub dot_active: f32,
    pub icon_size: f32,
    pub window_max: f32,
    pub tile_pad: f32,
    pub panel_w: f32,
    pub panel_h: f32,
    pub list_h: f32,
    pub row_h: f32,
    pub panel_gap: f32,
    pub blur: f32,
    pub font_family: String,
    pub font_icon: String,
    pub weight_medium: u16,
    pub label_size: f32,
    pub clock_size: f32,
    pub window_size: f32,
    pub light: ThemeTokens,
    pub dark: ThemeTokens,
}

pub fn tokens() -> &'static Tokens {
    static CELL: OnceLock<Tokens> = OnceLock::new();
    // SOURCE is embedded at compile time, so a failure here is a token the
    // bundled JSON is missing or has in the wrong shape — a build-time bug.
    CELL.get_or_init(|| parse(SOURCE).unwrap_or_else(|e| panic!("bar.tokens.json: {e}")))
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
        height_compact: dim("size.height-compact")?,
        height_overview: dim("size.height-overview")?,
        radius_panel: dim("radius.panel")?,
        radius_pill: dim("radius.pill")?,
        radius_tile: dim("radius.tile")?,
        panel_pad_x: dim("space.panel-pad-x")?,
        panel_pad_y: dim("space.panel-pad-y")?,
        section_gap: dim("space.section-gap")?,
        item_gap: dim("space.item-gap")?,
        pill_pad_x: dim("space.pill-pad-x")?,
        pill_pad_y: dim("space.pill-pad-y")?,
        dot_size: dim("space.dot-size")?,
        dot_active: dim("space.dot-active")?,
        icon_size: dim("space.icon-size")?,
        window_max: dim("space.window-max")?,
        tile_pad: dim("space.tile-pad")?,
        panel_w: dim("space.panel-w")?,
        panel_h: dim("space.panel-h")?,
        list_h: dim("space.list-h")?,
        row_h: dim("space.row-h")?,
        panel_gap: dim("space.panel-gap")?,
        blur: num("effect.blur")?,
        font_family: font_family(&root, "font.ui.family").ok_or("bad token font.ui.family")?,
        font_icon: font_family(&root, "font.icon.family").ok_or("bad token font.icon.family")?,
        weight_medium: weight("font.ui.weight.medium")?,
        label_size: num("type.label.size")?,
        clock_size: num("type.clock.size")?,
        window_size: num("type.window.size")?,
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
        fg: c("fg")?,
        fg_strong: c("fg-strong")?,
        pill: c("pill")?,
        pill_focused: c("pill-focused")?,
        accent: c("accent")?,
        urgent: c("urgent")?,
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
    walk(root, path)?.get("$value").cloned()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_tokens_parse() {
        // The embedded bar.tokens.json must be complete and well-shaped; this
        // catches a renamed/missing token or a wrong-typed value at test time
        // rather than as a runtime panic in tokens().
        let t = parse(SOURCE).expect("bundled bar.tokens.json must parse");
        assert!(t.height_overview >= t.height_compact);
        assert!(t.weight_medium > 0);
        assert!(!t.font_family.is_empty());
        assert!(!t.font_icon.is_empty());
        assert!(t.radius_pill >= 0.0);
    }

    #[test]
    fn invalid_json_is_err() {
        assert!(parse("definitely not json").is_err());
    }

    #[test]
    fn missing_tokens_are_err() {
        // Valid JSON but missing the required token tree.
        assert!(parse("{}").is_err());
    }

    #[test]
    fn alias_resolution_terminates() {
        // A self-referential alias must not loop forever (resolve() is capped).
        let root: Value = serde_json::from_str(r#"{"a":{"$value":"{a}"}}"#).unwrap();
        assert!(resolve(&root, &Value::String("{a}".to_string())).is_none());
    }
}
