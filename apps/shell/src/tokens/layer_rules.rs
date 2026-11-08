//! niri layer rules and materials from tokens: the compositor's blur
//! corners come from the same radius role the shell draws its cards with,
//! and its materials from the `material.*` tokens, so neither can drift.
//! `cargo xtask layer-rules` prints the file; `niri-layer-rule.kdl` is its
//! committed output, checked by a golden test.

use std::fmt::Write;

use crate::surfaces::alert::ALERT_NS;
use crate::surfaces::dock::DOCK_NS;
use crate::surfaces::launcher::LAUNCHER_NS;
use crate::surfaces::notifications::{CENTER_NS, TOAST_NS};
use crate::surfaces::osd::OSD_NS;
use crate::surfaces::quick_settings::QS_NS;
use crate::surfaces::status_bar::BAR_NS;
use crate::surfaces::widgets::WIDGETS_NS;
use crate::theme::{Density, Mode};
use emaki_kit::material::Material;
use emaki_kit::menu::layer::{ANCHOR_NS, MAX_DEPTH, level_ns};

use crate::tokens::{Key, Value, bar, dialog, library, notification, osd, resolved, sys, with_key};

pub struct LayerRule {
    pub namespace: &'static str,
    pub comment: String,
    /// The surface is a card, rounded and casting niri's shadow: the shell
    /// can't draw the shadow, as the surface is only as big as its card.
    /// `None` leaves it alone.
    pub card: Option<Card>,
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
    /// The surface is the desktop (niri fork's `desktop`): on the wallpaper,
    /// still while workspaces switch, faded out with the overview, taking
    /// input where no window is.
    pub desktop: bool,
}

/// A card surface's corners, and how soft its shadow is and how far down
/// it falls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Card {
    pub radius: f32,
    pub softness: f32,
    pub y: f32,
}

// TODO(token): `sys.shadow.popover` once Penpot has it (docs/developer/design-system/penpot-backlog.md).
// The rules are static, so the shadow can't follow the mode: this is the
// dark one, which also reads in light (the banner design's light shadow is
// lighter, `#00000040`).
/// The shadow under every frosted surface: the shell can't draw it, as each
/// surface is only as big as its card.
const SHADOW_SOFTNESS: f32 = 16.0;
const SHADOW_Y: f32 = 8.0;
const SHADOW_COLOR: &str = "#00000073";

/// A notification banner's shadow spreads wider than a panel's, as in the
/// banner design (`0 8 48`).
const BANNER_SHADOW_SOFTNESS: f32 = 48.0;

/// An alert floats highest of all, its shadow spread wide and falling far,
/// as in the dialog design (`0 18 48`).
const ALERT_SHADOW_SOFTNESS: f32 = 48.0;
const ALERT_SHADOW_Y: f32 = 18.0;

/// emaki-drop's grid; its own app, whose namespace is in the kit.
const DROP_NS: &str = emaki_kit::app_ids::DROP_NS;

/// One rule per frosted surface.
pub fn rules() -> Vec<LayerRule> {
    // radii don't vary with mode or density (a contract test checks it)
    let key = Key::new(Mode::Dark, Density::Comfortable);
    let popover = with_key(key, sys::radius::popover);
    let toast = with_key(key, notification::card::radius);
    let center = with_key(key, notification::center::radius);
    let strip = with_key(key, bar::radius);
    let alert = with_key(key, dialog::radius);
    let capsule = with_key(key, osd::radius);

    vec![
        LayerRule {
            namespace: BAR_NS,
            comment: String::from(
                "Status bar: one uniform frosted strip along its edge, in the standard\n\
                 material without a lens (frost). It reserves its zone, so only\n\
                 the wallpaper is behind it and niri's xray blur is enough. Its xdg-popups\n\
                 (tooltips, Do Not Disturb's menu) are frosted cards.",
            ),
            card: None,
            frost: Some(strip),
            popups: Some(popover),
            clear: false,
            desktop: false,
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
            card: None,
            frost: None,
            popups: Some(popover),
            clear: false,
            desktop: false,
        },
        LayerRule {
            namespace: QS_NS,
            comment: String::from(
                "Quick Settings: a clear surface whose modules are frosted one by one, the\n\
                      surface saying where (ext-background-effect). Toggled by the status bar's\n\
                      Quick Settings button. A Bluetooth device's menu is an xdg-popup of it.",
            ),
            card: None,
            frost: None,
            popups: Some(popover),
            clear: true,
            desktop: false,
        },
        LayerRule {
            namespace: DOCK_NS,
            comment: String::from(
                "Dock: a clear surface over the bar, one a monitor, whose card is frosted\n\
                      where the surface says (ext-background-effect). It comes up from the\n\
                      bottom edge; a window's menu is an xdg-popup of it.",
            ),
            card: None,
            frost: None,
            popups: Some(popover),
            clear: true,
            desktop: false,
        },
        LayerRule {
            namespace: WIDGETS_NS,
            comment: String::from(
                "Desktop widgets: a clear surface on the wallpaper, one a monitor\n\
                      (`emaki.widgets.<output>`, matched by the prefix), whose cards are frosted\n\
                      where the surface says (ext-background-effect). The desktop: still while\n\
                      workspaces switch, faded out with the overview. A card's menu is an\n\
                      xdg-popup of it.",
            ),
            card: None,
            frost: None,
            popups: Some(popover),
            clear: true,
            desktop: true,
        },
        LayerRule {
            namespace: LAUNCHER_NS,
            comment: String::from(
                "Launcher: a clear surface over the output whose glass panel, in the middle,\n\
                      is frosted where the surface says (ext-background-effect). Toggled over IPC\n\
                      from a keybind. An app's menu is an xdg-popup of it.",
            ),
            card: None,
            frost: None,
            popups: Some(popover),
            clear: true,
            desktop: false,
        },
        LayerRule {
            namespace: TOAST_NS,
            comment: String::from(
                "Notification toasts: one surface per banner, sized to its card, sliding in\n\
                      at the top-right. Each frosts its card itself (blur shapes, in the glass\n\
                      material); the rule rounds it and gives it niri's shadow.",
            ),
            card: Some(Card {
                radius: toast,
                softness: BANNER_SHADOW_SOFTNESS,
                y: SHADOW_Y,
            }),
            frost: None,
            popups: None,
            clear: true,
            desktop: false,
        },
        LayerRule {
            namespace: CENTER_NS,
            comment: String::from(
                "Notification center: a full-height glass sidebar at the right edge, opened\n\
                      from the clock, frosting itself (blur shapes, in the glass material); the\n\
                      rule rounds it and gives it niri's shadow. Its Do Not Disturb menu is an\n\
                      xdg-popup of it.",
            ),
            card: Some(Card {
                radius: center,
                softness: SHADOW_SOFTNESS,
                y: SHADOW_Y,
            }),
            frost: None,
            popups: Some(popover),
            clear: true,
            desktop: false,
        },
        LayerRule {
            namespace: ALERT_NS,
            comment: String::from(
                "Alert: a question in the middle of the screen (\"Restart now?\"), on a surface\n\
                      as big as its card, which frosts itself (a blur shape, in thick glass); the\n\
                      rule rounds it and gives it niri's shadow, spread wide and falling far.",
            ),
            card: Some(Card {
                radius: alert,
                softness: ALERT_SHADOW_SOFTNESS,
                y: ALERT_SHADOW_Y,
            }),
            frost: None,
            popups: None,
            clear: true,
            desktop: false,
        },
        LayerRule {
            namespace: OSD_NS,
            comment: String::from(
                "On-screen display: the capsule that shows a level or a switch as a key\n\
                      changes it, above the bottom edge, on a surface as big as the capsule. It\n\
                      frosts itself (a blur shape, in small glass); the rule rounds it and gives\n\
                      it niri's shadow.",
            ),
            card: Some(Card {
                radius: capsule,
                softness: BANNER_SHADOW_SOFTNESS,
                y: SHADOW_Y,
            }),
            frost: None,
            popups: None,
            clear: true,
            desktop: false,
        },
        LayerRule {
            namespace: DROP_NS,
            comment: String::from(
                "emaki-drop's grid (apps/drop): a clear full-screen surface that frosts only\n\
                      its card, with a blur shape of its own, as Quick Settings does.",
            ),
            card: None,
            frost: None,
            popups: Some(popover),
            clear: true,
            desktop: false,
        },
    ]
}

/// The amane apps whose windows are frosted behind, in the window
/// material, their popups in the menu's (`kit::app::app` names them).
pub const APP_IDS: &[&str] = &[emaki_kit::app_ids::SETTINGS, emaki_kit::app_ids::MESSAGES];

/// The apps with no window rule, and why; the test holds every other app to one.
#[cfg(test)]
pub const NO_RULE: &[(&str, &str)] = &[
    (
        emaki_kit::app_ids::SHADER_LAB,
        "a developer tool, plain on purpose",
    ),
    (
        emaki_kit::app_ids::DROP,
        "its grid is a layer surface, with a layer rule of its own",
    ),
];

/// The phone's screen (`apps/mirror`): its own floating window, shaped
/// like a phone, opaque so nothing frosts behind it; each of the phone's
/// apps opened alone is `emaki-mirror.<package>`.
pub const PHONE_SCREEN_APP_ID: &str = emaki_kit::app_ids::MIRROR;

/// The line `gen-layer-rules.sh` looks for; everything after it goes into
/// the niri config.
const SENTINEL: &str = "//<<<GENERATED-RULES-BELOW>>>";

const HEADER: &str = "\
// GENERATED by `cargo xtask layer-rules > niri-layer-rule.kdl` from the
// design tokens (radii: sys.radius.popover, notification.card.radius,
// notification.center.radius) — do not edit by hand.
//
// The amane shell's niri layer-rules (compositor-side blur, rounding and
// shadow on its surfaces). Each rule matches an anchored regex on a
// layer-surface namespace that a component sets via LayerWindow::namespace(...).
//
// Don't hand-copy these into the niri config. gen-layer-rules.sh injects
// everything below the sentinel into the config between its
// `>>> emaki layer-rules` / `<<<` markers (see docs/developer/integration/layer-rules.md); then:
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
        if let Some(card) = rule.card {
            let _ = writeln!(out, "    geometry-corner-radius {}", card.radius);
            out.push_str("    shadow {\n        on\n");
            let _ = writeln!(out, "        softness {}", card.softness);
            let _ = writeln!(out, "        offset x=0 y={}", card.y);
            let _ = writeln!(out, "        color \"{SHADOW_COLOR}\"");
            out.push_str("    }\n");
        }
        if let Some(radius) = rule.frost {
            let _ = writeln!(out, "    geometry-corner-radius {radius}");
            out.push_str("    background-effect {\n        blur true\n");
            let _ = writeln!(out, "        material \"{}\"", Material::Bar.niri_name());
            out.push_str("    }\n");
        }
        if rule.desktop {
            out.push_str("    desktop true\n");
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
    out.push_str(&render_phone_screen());
    out.push_str(&render_materials(&materials()));
    out
}

/// A rule for each amane app's windows: frost behind them in the window
/// material, which the window tints itself (`kit::app::window_bg`), and
/// its popups (dropdowns, menus) frosted like the shell's. A second rule
/// makes its floating windows, its dialogs (a window with a parent floats),
/// cards of thick glass with the alert's corners and shadow
/// (`kit::dialog`).
fn render_windows(app_ids: &[&str]) -> String {
    // radii don't vary with mode or density (a contract test checks it)
    let key = Key::new(Mode::Dark, Density::Comfortable);
    let popover = with_key(key, sys::radius::popover);
    let dialog_radius = with_key(key, dialog::radius);

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

        out.push_str(
            "\n// Its dialogs, which float over their window: thick glass, the alert's corners\n\
             // and shadow. (A window floated by hand takes this look too.)\n",
        );
        out.push_str("window-rule {\n");
        let _ = writeln!(
            out,
            "    match app-id=\"{}\" is-floating=true",
            kdl_string(&anchored(app_id))
        );
        let _ = writeln!(out, "    geometry-corner-radius {dialog_radius}");
        out.push_str("    clip-to-geometry true\n");
        out.push_str("    shadow {\n        on\n");
        let _ = writeln!(out, "        softness {ALERT_SHADOW_SOFTNESS}");
        let _ = writeln!(out, "        offset x=0 y={ALERT_SHADOW_Y}");
        let _ = writeln!(out, "        color \"{SHADOW_COLOR}\"");
        out.push_str("    }\n");
        out.push_str("    background-effect {\n        blur true\n");
        let _ = writeln!(out, "        material \"{}\"", Material::Sheet.niri_name());
        out.push_str("    }\n}\n");
    }
    out
}

/// The phone's screen opens floating, at the size it asks for (a phone's
/// shape), with the alert's corners and shadow; it draws every pixel
/// itself, so there's no blur behind it.
fn render_phone_screen() -> String {
    let key = Key::new(Mode::Dark, Density::Comfortable);
    let radius = with_key(key, dialog::radius);

    let mut out = String::from(
        "\n// The phone's screen, and its apps opened alone: floating windows shaped like\n\
         // the phone, with the alert's corners and shadow.\n",
    );
    out.push_str("window-rule {\n");
    // the screen's id alone, or with an app's package after a dot
    let pattern = format!(
        "{}(\\..+)?$",
        anchored(PHONE_SCREEN_APP_ID).trim_end_matches('$')
    );
    let _ = writeln!(out, "    match app-id=\"{}\"", kdl_string(&pattern));
    out.push_str("    open-floating true\n");
    let _ = writeln!(out, "    geometry-corner-radius {radius}");
    out.push_str("    clip-to-geometry true\n");
    out.push_str("    shadow {\n        on\n");
    let _ = writeln!(out, "        softness {ALERT_SHADOW_SOFTNESS}");
    let _ = writeln!(out, "        offset x=0 y={ALERT_SHADOW_Y}");
    let _ = writeln!(out, "        color \"{SHADOW_COLOR}\"");
    out.push_str("    }\n}\n");
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
         // the same names. See the niri fork's docs/fork/effects/materials.md.\n\
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
        let committed = include_str!("../../../../niri-layer-rule.kdl");
        assert!(
            committed == render(&rules()),
            "niri-layer-rule.kdl is out of date — run: \
             cargo xtask layer-rules > niri-layer-rule.kdl"
        );
    }

    #[test]
    fn the_script_splices_after_the_same_sentinel() {
        let script = include_str!("../../../../gen-layer-rules.sh");
        assert!(script.contains(&format!("SENTINEL=\"{SENTINEL}\"")));
    }

    /// niri's ranges for each material setting, copied from niri-config.
    const MATERIAL_RANGES: &str = include_str!("../../tests/fixtures/niri/material-ranges.json");

    /// niri refuses a config with an unknown material setting or a value
    /// out of its range, so every generated one must be known and in range.
    #[test]
    fn every_material_setting_is_one_niri_accepts() {
        let ranges: serde_json::Value =
            serde_json::from_str(MATERIAL_RANGES).expect("the ranges fixture is JSON");
        let settings = &ranges["settings"];
        let mut wrong = Vec::new();
        for material in materials() {
            for (setting, value) in &material.settings {
                let at = format!("{}.{setting} = {value}", material.name);
                let Some(range) = settings.get(setting) else {
                    wrong.push(format!("{at}: niri has no such setting"));
                    continue;
                };
                let fits = match range["kind"].as_str() {
                    Some("bool") => value == "true" || value == "false",
                    Some(kind) => {
                        let (min, max) = (range["min"].as_f64(), range["max"].as_f64());
                        match (value.parse::<f64>(), min, max) {
                            (Ok(v), Some(min), Some(max)) => {
                                (min..=max).contains(&v) && (kind != "integer" || v.fract() == 0.0)
                            }
                            _ => false,
                        }
                    }
                    None => false,
                };
                if !fits {
                    wrong.push(format!("{at}: outside {range}"));
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "niri would refuse:\n  {}",
            wrong.join("\n  ")
        );
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
        for (big, small) in [
            ("glass", "glass-small"),
            ("glass-still", "glass-small-still"),
        ] {
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
            assert_eq!(
                rest,
                small
                    .settings
                    .iter()
                    .filter(|s| !sized(s))
                    .collect::<Vec<_>>()
            );
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
    fn every_emaki_app_has_a_window_rule() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        // every app under apps/ that opens a window names itself from the kit
        for entry in std::fs::read_dir(&root).unwrap().flatten() {
            let folder = entry.file_name().to_string_lossy().into_owned();
            let Ok(main) = std::fs::read_to_string(entry.path().join("src/main.rs")) else {
                continue;
            };
            let windowed = main.contains("app(") || main.contains(".app_id(");
            if folder == "shell" || !windowed {
                continue;
            }
            let Some((_, id)) = emaki_kit::app_ids::APPS.iter().find(|(f, _)| *f == folder) else {
                panic!("apps/{folder} opens a window but has no id in emaki_kit::app_ids");
            };
            assert!(
                !main.contains(&format!("\"{id}\"")),
                "apps/{folder} types its id by hand; use emaki_kit::app_ids"
            );
        }
        // and each id has a rule, or a reason it has none
        for (folder, id) in emaki_kit::app_ids::APPS {
            let ruled = APP_IDS.contains(&id) || id == PHONE_SCREEN_APP_ID;
            let excused = NO_RULE.iter().any(|(none, _)| *none == id);
            assert!(
                ruled != excused,
                "apps/{folder} ({id}) needs a window rule or a reason in NO_RULE, not both or neither"
            );
            let kdl = render(&rules());
            if ruled {
                assert!(kdl.contains(id), "{id} has no rule in the output");
            }
        }
    }

    #[test]
    fn namespaces_are_escaped() {
        assert_eq!(
            kdl_string(&anchored("emaki.status-bar.wifi")),
            r"^emaki\\.status-bar\\.wifi$"
        );
        assert_eq!(anchored("a+b(c)"), r"^a\+b\(c\)$");
    }

    /// Namespaces with no rule of their own, and why they need none.
    const POPUPS: &[(&str, &str)] = &[
        (
            "emaki.context-menu.",
            "menu levels: popups under the anchor's rule (popups {})",
        ),
        (
            "emaki.status-bar.activity",
            "a popup of the status bar, under its rule",
        ),
        (
            "emaki.tooltip",
            "a popup of whichever surface shows it, under that one's rule",
        ),
        (
            "emaki.menu",
            "a popup of the window or surface that opens it",
        ),
        (
            "emaki.fill",
            "an input-method popup over another app's window, not a layer surface",
        ),
    ];

    /// Every `.rs` file under `dir`.
    fn sources(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).expect("a source folder").flatten() {
            let path = entry.path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).expect("a source file");
                out.push((path, text));
            }
        }
    }

    /// The `amane.…` namespaces the shell's surfaces and popups use: the
    /// `…NS` constants (one, or an array of them), and literal
    /// `Popup::new("…")` and `.namespace("…")` calls.
    fn namespaces_in_code() -> std::collections::BTreeMap<String, String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        for dir in [
            "src",
            "../../crates/kit/src",
            "../../crates/base/src",
            "../drop/src",
        ] {
            sources(&root.join(dir), &mut files);
        }
        let mut found = std::collections::BTreeMap::new();
        for (path, text) in &files {
            let mut in_array = false;
            for (n, line) in text.lines().enumerate() {
                let decl = line
                    .split_once("const ")
                    .and_then(|(_, rest)| rest.split_once(':'))
                    .filter(|(name, _)| name.trim().ends_with("NS"));
                let take = match decl {
                    Some((_, ty)) if ty.trim_start().starts_with('[') => {
                        in_array = !line.contains("];");
                        true
                    }
                    Some(_) => true,
                    None if in_array => {
                        in_array = !line.contains("];");
                        true
                    }
                    None => line.contains("Popup::new(\"") || line.contains(".namespace(\""),
                };
                if !take {
                    continue;
                }
                for literal in line.split('"').skip(1).step_by(2) {
                    if literal.starts_with("amane.") {
                        let at = format!("{}:{}", path.display(), n + 1);
                        found.entry(literal.to_string()).or_insert(at);
                    }
                }
            }
        }
        found
    }

    #[test]
    fn every_namespace_has_a_rule_or_rides_on_one() {
        let rules: Vec<&str> = rules().iter().map(|r| r.namespace).collect();
        let covers =
            |rule: &str, ns: &str| ns == rule || (rule.ends_with('.') && ns.starts_with(rule));
        let found = namespaces_in_code();
        assert!(found.len() > 10, "found too few namespaces: {found:?}");

        let mut lost = Vec::new();
        for (ns, at) in &found {
            let ruled = rules.iter().any(|rule| covers(rule, ns));
            let popup = POPUPS.iter().any(|(p, _)| covers(p, ns));
            if !ruled && !popup {
                lost.push(format!("{ns} ({at})"));
            }
        }
        assert!(
            lost.is_empty(),
            "namespaces with no layer rule; add one to rules(), or to POPUPS with why:\n  {}",
            lost.join("\n  ")
        );

        // and nothing listed that the code no longer uses
        for rule in rules.iter().chain(POPUPS.iter().map(|(p, _)| p)) {
            assert!(
                found.keys().any(|ns| covers(rule, ns)),
                "{rule} is in the rules but no surface uses it"
            );
        }
    }
}
