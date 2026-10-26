//! niri layer rules and materials from tokens: the compositor's blur
//! corners come from the same radius role the shell draws its cards with,
//! and its materials from the `material.*` tokens, so neither can drift.
//! `amane-shell layer-rules` prints the file; `niri-layer-rule.kdl` is its
//! committed output, checked by a golden test.

use std::fmt::Write;

use crate::components::context_menu::{ANCHOR_NS, MAX_DEPTH, level_ns};
use crate::components::dock::DOCK_NS;
use crate::components::launcher::LAUNCHER_NS;
use crate::components::notifications::{CENTER_NS, TOAST_NS};
use crate::components::quick_settings::QS_NS;
use crate::components::status_bar::BAR_NS;
use crate::components::widgets::WIDGETS_NS;
use crate::theme::{Density, Mode};
use amane_kit::material::Material;

use crate::tokens::{Key, Value, bar, library, notification, resolved, sys, with_key};

pub struct LayerRule {
    pub namespace: &'static str,
    pub comment: String,
    /// The surface is a card with these corners, casting niri's shadow: the
    /// shell can't draw it, as the surface is only as big as its card.
    /// `None` leaves it alone.
    pub radius: Option<f32>,
    /// The surface is frosted whole with these corners, in the bar's
    /// material, with no shadow, blurring the wallpaper (niri's xray): for a
    /// surface that reserves its zone, so nothing but the wallpaper is ever
    /// behind it.
    pub frost: Option<f32>,
    /// Its xdg-popups are frosted cards with these corners, drawn in the
    /// menu's material (they can't send blur shapes of their own). niri
    /// gives popups no shadow.
    pub popups: Option<f32>,
    /// A clear surface that asks for frost itself, where its modules are
    /// (ext-background-effect): no corners, no shadow, only `xray false`,
    /// since niri's blur would otherwise show the wallpaper alone.
    pub clear: bool,
}

// TODO(token): `sys.shadow.popover` once Penpot has it (docs/penpot-todo.md).
// The rules are static, so the shadow can't follow the mode: this is the
// dark one, which also reads in light.
/// The shadow under every frosted surface: the shell can't draw it, as each
/// surface is only as big as its card.
const SHADOW_SOFTNESS: f32 = 16.0;
const SHADOW_Y: f32 = 8.0;
const SHADOW_COLOR: &str = "#00000073";

/// One rule per frosted surface.
pub fn rules() -> Vec<LayerRule> {
    // radii don't vary with mode or density (a contract test checks it)
    let key = Key::new(Mode::Dark, Density::Comfortable);
    let popover = with_key(key, sys::radius::popover);
    let toast = with_key(key, notification::card::radius);
    let center = with_key(key, notification::center::radius);
    let strip = with_key(key, bar::radius);

    vec![
        LayerRule {
            namespace: BAR_NS,
            comment: String::from(
                "Status bar: one uniform frosted strip along the bottom, macOS 27's toolbar:\n\
                 frost, the standard material without a lens. It reserves its zone, so only\n\
                 the wallpaper is behind it and niri's xray blur is enough. Its xdg-popups\n\
                 (tooltips, Do Not Disturb's menu) are frosted cards.",
            ),
            radius: None,
            frost: Some(strip),
            popups: Some(popover),
            clear: false,
        },
        LayerRule {
            namespace: ANCHOR_NS,
            comment: format!(
                "Context menu. The anchor is an invisible 1×1 layer-surface; the visible\n\
                 cards (menu + nested submenus) are xdg-popups of it, so the `popups {{}}`\n\
                 block styles every level uniformly. Namespaces: the anchor is\n\
                 `{ANCHOR_NS}`; each depth level is `{}`..`{}`.",
                level_ns(0),
                MAX_DEPTH - 1
            ),
            radius: None,
            frost: None,
            popups: Some(popover),
            clear: false,
        },
        LayerRule {
            namespace: QS_NS,
            comment: String::from(
                "Quick Settings: a clear surface whose modules are frosted one by one, the\n\
                      surface saying where (ext-background-effect). Toggled by the status bar's\n\
                      Quick Settings button. A Bluetooth device's menu is an xdg-popup of it.",
            ),
            radius: None,
            frost: None,
            popups: Some(popover),
            clear: true,
        },
        LayerRule {
            namespace: DOCK_NS,
            comment: String::from(
                "Dock: a clear surface over the bar, one a monitor, whose card is frosted\n\
                      where the surface says (ext-background-effect). It comes up from the\n\
                      bottom edge; a window's menu is an xdg-popup of it.",
            ),
            radius: None,
            frost: None,
            popups: Some(popover),
            clear: true,
        },
        LayerRule {
            namespace: WIDGETS_NS,
            comment: String::from(
                "Desktop widgets: a clear surface on the wallpaper, one a monitor\n\
                      (`amane.widgets.<output>`, matched by the prefix), whose cards are frosted\n\
                      where the surface says (ext-background-effect). A card's menu is an\n\
                      xdg-popup of it.",
            ),
            radius: None,
            frost: None,
            popups: Some(popover),
            clear: true,
        },
        LayerRule {
            namespace: LAUNCHER_NS,
            comment: String::from(
                "Launcher: a clear surface over the output whose glass panel, in the middle,\n\
                      is frosted where the surface says (ext-background-effect). Toggled over IPC\n\
                      from a keybind. An app's menu is an xdg-popup of it.",
            ),
            radius: None,
            frost: None,
            popups: Some(popover),
            clear: true,
        },
        LayerRule {
            namespace: TOAST_NS,
            comment: String::from(
                "Notification toasts: one surface per banner, sized to its card, sliding in\n\
                      at the top-right. Each frosts its card itself (blur shapes, in the glass\n\
                      material); the rule rounds it and gives it niri's shadow.",
            ),
            radius: Some(toast),
            frost: None,
            popups: None,
            clear: true,
        },
        LayerRule {
            namespace: CENTER_NS,
            comment: String::from(
                "Notification center: a full-height glass sidebar at the right edge, opened\n\
                      from the clock, frosting itself (blur shapes, in the glass material); the\n\
                      rule rounds it and gives it niri's shadow. Its Do Not Disturb menu is an\n\
                      xdg-popup of it.",
            ),
            radius: Some(center),
            frost: None,
            popups: Some(popover),
            clear: true,
        },
    ]
}

/// The amane apps whose windows are frosted behind, in the window
/// material, their popups in the menu's (`kit::app::app` names them).
pub const APP_IDS: &[&str] = &["amane-settings"];

/// The line `gen-layer-rules.sh` looks for; everything after it goes into
/// the niri config.
const SENTINEL: &str = "//<<<GENERATED-RULES-BELOW>>>";

const HEADER: &str = "\
// GENERATED by `cargo run -q -- layer-rules > niri-layer-rule.kdl` from the
// design tokens (radii: sys.radius.popover, notification.card.radius,
// notification.center.radius) — do not edit by hand.
//
// The amane shell's niri layer-rules (compositor-side blur, rounding and
// shadow on its surfaces). Each rule matches an anchored regex on a
// layer-surface namespace that a component sets via LayerWindow::namespace(...).
//
// Don't hand-copy these into the niri config. gen-layer-rules.sh injects
// everything below the sentinel into the config between its
// `>>> amane layer-rules` / `<<<` markers (see docs/layer-rules.md); then:
//
//     niri validate -c ~/.config/niri/config.kdl
//     niri msg action load-config-file
//
// Verify the surfaces are up:  niri msg layers | grep amane

{SENTINEL}  (gen-layer-rules.sh injects everything past this line)
";

/// The whole `niri-layer-rule.kdl`.
pub fn render(rules: &[LayerRule]) -> String {
    let mut out = HEADER.replace("{SENTINEL}", SENTINEL);

    for rule in rules {
        out.push('\n');
        for line in rule.comment.lines() {
            let _ = writeln!(out, "// {}", line.trim_start());
        }
        out.push_str("layer-rule {\n");
        let _ = writeln!(
            out,
            "    match namespace=\"{}\"",
            kdl_string(&anchored(rule.namespace))
        );
        if let Some(radius) = rule.radius {
            let _ = writeln!(out, "    geometry-corner-radius {radius}");
            out.push_str("    shadow {\n        on\n");
            let _ = writeln!(out, "        softness {SHADOW_SOFTNESS}");
            let _ = writeln!(out, "        offset x=0 y={SHADOW_Y}");
            let _ = writeln!(out, "        color \"{SHADOW_COLOR}\"");
            out.push_str("    }\n");
        }
        if let Some(radius) = rule.frost {
            let _ = writeln!(out, "    geometry-corner-radius {radius}");
            out.push_str("    background-effect {\n        blur true\n");
            let _ = writeln!(out, "        material \"{}\"", Material::Bar.niri_name());
            out.push_str("    }\n");
        }
        if rule.clear {
            // the surface sets its own blur region; without `xray false`
            // niri would blur the wallpaper alone behind it
            out.push_str("    background-effect {\n        xray false\n    }\n");
        }
        if let Some(radius) = rule.popups {
            out.push_str("\n    popups {\n");
            let _ = writeln!(out, "        geometry-corner-radius {radius}");
            out.push_str("        background-effect {\n            blur true\n");
            let _ = writeln!(
                out,
                "            material \"{}\"",
                Material::Menu.niri_name()
            );
            out.push_str("        }\n    }\n");
        }
        out.push_str("}\n");
    }

    out.push_str(&render_windows(APP_IDS));
    out.push_str(&render_materials(&materials()));
    out
}

/// A rule for each amane app's windows: frost behind them in the window
/// material, which the window tints itself (`kit::app::window_bg`), and
/// its popups (dropdowns, menus) frosted like the shell's.
fn render_windows(app_ids: &[&str]) -> String {
    // radii don't vary with mode or density (a contract test checks it)
    let popover = with_key(
        Key::new(Mode::Dark, Density::Comfortable),
        sys::radius::popover,
    );

    let mut out = String::new();
    for app_id in app_ids {
        out.push_str(
            "\n// An amane app's windows: frosted behind, the window drawing its own tint\n\
             // over the frost (the window material), and its popups like the shell's.\n",
        );
        out.push_str("window-rule {\n");
        let _ = writeln!(
            out,
            "    match app-id=\"{}\"",
            kdl_string(&anchored(app_id))
        );
        out.push_str("    background-effect {\n        blur true\n");
        let _ = writeln!(out, "        material \"{}\"", Material::Window.niri_name());
        out.push_str("    }\n\n    popups {\n");
        let _ = writeln!(out, "        geometry-corner-radius {popover}");
        out.push_str("        background-effect {\n            blur true\n");
        let _ = writeln!(
            out,
            "            material \"{}\"",
            Material::Menu.niri_name()
        );
        out.push_str("        }\n    }\n}\n");
    }
    out
}

/// One of niri's materials: its name and its settings, in token order.
pub struct NiriMaterial {
    pub name: String,
    pub settings: Vec<(String, String)>,
}

/// The `material.<name>.<setting>` tokens, one material a name, each with
/// its twin for reduced motion: the same, without the lens building up.
pub fn materials() -> Vec<NiriMaterial> {
    // they don't vary with mode or density (a contract test checks it)
    let tokens = &resolved(Key::new(Mode::Dark, Density::Comfortable)).tokens;

    let mut out: Vec<NiriMaterial> = Vec::new();
    for token in library().sets.iter().flat_map(|s| &s.tokens) {
        let ["material", name, setting] = token.name.split('.').collect::<Vec<_>>()[..] else {
            continue;
        };
        let value = match &tokens[&token.name].value {
            Value::Number(n) => n.value.to_string(),
            Value::Bool(b) => b.to_string(),
            other => panic!(
                "{}: niri takes numbers and booleans, not {other:?}",
                token.name
            ),
        };
        let at = match out.iter().position(|m| m.name == name) {
            Some(at) => at,
            None => {
                out.push(NiriMaterial {
                    name: name.to_string(),
                    settings: Vec::new(),
                });
                out.len() - 1
            }
        };
        out[at].settings.push((setting.to_string(), value));
    }

    let still: Vec<NiriMaterial> = out
        .iter()
        .map(|m| NiriMaterial {
            name: format!("{}-still", m.name),
            settings: m
                .settings
                .iter()
                .cloned()
                .chain([(String::from("materialize"), String::from("false"))])
                .collect(),
        })
        .collect();
    out.extend(still);
    out
}

fn render_materials(materials: &[NiriMaterial]) -> String {
    let mut out = String::from(
        "\n// The shell's materials (material.* tokens), by the names it asks for: liquid\n\
         // glass, its clear variant for media, and frost without a lens; each `-still`\n\
         // twin is for reduced motion. They replace a hand-written `materials {}` of\n\
         // the same names. See the niri fork's docs/wiki/Window-Effects.md.\n\
         materials {\n",
    );
    for material in materials {
        let _ = writeln!(out, "    material \"{}\" {{", kdl_string(&material.name));
        for (setting, value) in &material.settings {
            let _ = writeln!(out, "        {setting} {value}");
        }
        out.push_str("    }\n");
    }
    out.push_str("}\n");
    out
}

/// `^…$` with regex metacharacters escaped.
/// The regex matching `namespace` whole, or every namespace it begins
/// when it ends in a dot (one surface a monitor).
fn anchored(namespace: &str) -> String {
    let mut out = String::from("^");
    for c in namespace.chars() {
        if r"\.^$|?*+()[]{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    if !namespace.ends_with('.') {
        out.push('$');
    }
    out
}

fn kdl_string(s: &str) -> String {
    s.replace('\\', r"\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_kdl_matches_the_tokens() {
        let committed = include_str!("../../niri-layer-rule.kdl");
        assert!(
            committed == render(&rules()),
            "niri-layer-rule.kdl is out of date — run: \
             cargo run -q -- layer-rules > niri-layer-rule.kdl"
        );
    }

    #[test]
    fn the_script_splices_after_the_same_sentinel() {
        let script = include_str!("../../gen-layer-rules.sh");
        assert!(script.contains(&format!("SENTINEL=\"{SENTINEL}\"")));
    }

    #[test]
    fn every_material_the_shell_asks_for_is_generated() {
        let names: Vec<String> = materials().into_iter().map(|m| m.name).collect();
        for m in Material::ALL {
            let name = m.niri_name();
            assert!(names.iter().any(|n| n == name), "{name} isn't generated");
            assert!(
                names.iter().any(|n| *n == format!("{name}-still")),
                "{name}-still isn't generated"
            );
        }
    }

    #[test]
    fn large_glass_is_glass_at_a_heavier_blur_and_lift() {
        let all = materials();
        let find = |name: &str| all.iter().find(|m| m.name == name).expect(name);
        let number = |m: &NiriMaterial, name: &str| -> f64 {
            m.settings
                .iter()
                .find(|(s, _)| s == name)
                .map(|(_, v)| v.parse().expect("a number"))
                .unwrap_or_else(|| panic!("{} sets {name}", m.name))
        };
        let (glass, large) = (find("glass"), find("glass-large"));
        assert!(number(large, "blur-passes") > number(glass, "blur-passes"));
        assert!(number(large, "tone") < number(glass, "tone"));
        assert_eq!(number(large, "bevel"), number(glass, "bevel"));
    }

    #[test]
    fn small_glass_is_glass_at_a_lighter_blur_and_lens() {
        let all = materials();
        let find = |name: &str| {
            all.iter()
                .find(|m| m.name == name)
                .unwrap_or_else(|| panic!("{name} isn't generated"))
        };
        let number = |m: &NiriMaterial, name: &str| -> f64 {
            let (_, value) = m.settings.iter().find(|(s, _)| s == name).expect(name);
            value.parse().expect("a number")
        };
        for (big, small) in [("glass", "glass-small"), ("glass-still", "glass-small-still")] {
            let (big, small) = (find(big), find(small));
            for setting in ["blur-passes", "bevel", "refraction"] {
                assert!(
                    number(small, setting) < number(big, setting),
                    "{} has less {setting} than {}",
                    small.name,
                    big.name
                );
            }
            let sized = |(setting, _): &&(String, String)| {
                setting.starts_with("blur-") || setting == "bevel" || setting == "refraction"
            };
            let rest: Vec<_> = big.settings.iter().filter(|s| !sized(s)).collect();
            assert_eq!(rest, small.settings.iter().filter(|s| !sized(s)).collect::<Vec<_>>());
        }
    }

    #[test]
    fn still_twins_only_add_materialize_false() {
        let all = materials();
        for twin in all.iter().filter(|m| m.name.ends_with("-still")) {
            let base = all
                .iter()
                .find(|m| format!("{}-still", m.name) == twin.name)
                .expect("each twin has its material");
            assert_eq!(twin.settings[..base.settings.len()], base.settings[..]);
            assert_eq!(
                twin.settings[base.settings.len()..],
                [(String::from("materialize"), String::from("false"))]
            );
        }
    }

    #[test]
    fn every_amane_app_has_a_window_rule() {
        let settings = include_str!("../../apps/settings/src/main.rs");
        for app_id in APP_IDS {
            assert!(
                settings.contains(&format!("app(\"{app_id}\")")),
                "{app_id} isn't an app id any app uses"
            );
        }
    }

    #[test]
    fn namespaces_are_escaped() {
        assert_eq!(
            kdl_string(&anchored("amane.status-bar.wifi")),
            r"^amane\\.status-bar\\.wifi$"
        );
        assert_eq!(anchored("a+b(c)"), r"^a\+b\(c\)$");
    }
}
