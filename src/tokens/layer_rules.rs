//! niri layer rules from tokens: the compositor's blur corners come from the
//! same radius role the shell draws its cards with, so they can't drift.
//! `amane-shell layer-rules` prints the file; `niri-layer-rule.kdl` is its
//! committed output, checked by a golden test.

use std::fmt::Write;

use crate::components::context_menu::{ANCHOR_NS, MAX_DEPTH, level_ns};
use crate::components::dock::DOCK_NS;
use crate::components::launcher::LAUNCHER_NS;
use crate::components::notifications::{CENTER_NS, TOAST_NS};
use crate::components::quick_settings::QS_NS;
use crate::components::taskbar::BAR_NS;
use crate::theme::{Density, Mode};
use crate::tokens::{Key, notification, sys, with_key};

pub struct LayerRule {
    pub namespace: &'static str,
    pub comment: String,
    /// The surface is a frosted card with these corners, casting a shadow;
    /// `None` leaves it alone.
    pub radius: Option<f32>,
    /// Its xdg-popups are frosted cards with these corners. niri gives
    /// popups no shadow.
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

/// One rule per frosted surface. The bar's own is for its popups only: the
/// bar itself is deliberately not blurred.
pub fn rules() -> Vec<LayerRule> {
    // radii don't vary with mode or density (a contract test checks it)
    let key = Key::new(Mode::Dark, Density::Comfortable);
    let popover = with_key(key, sys::radius::popover);
    let toast = with_key(key, notification::card::radius);

    vec![
        LayerRule {
            namespace: BAR_NS,
            comment: String::from(
                "Taskbar: only its xdg-popups (tooltips, a window's menu) are frosted; the\n\
                 bar itself is deliberately NOT blurred.",
            ),
            radius: None,
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
            popups: Some(popover),
            clear: false,
        },
        LayerRule {
            namespace: QS_NS,
            comment: String::from(
                "Quick Settings: a clear surface whose modules are frosted one by one, the\n\
                      surface saying where (ext-background-effect). Toggled by the taskbar's\n\
                      Control Center button. A Bluetooth device's menu is an xdg-popup of it.",
            ),
            radius: None,
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
            popups: Some(popover),
            clear: true,
        },
        LayerRule {
            namespace: LAUNCHER_NS,
            comment: String::from(
                "Launcher: a clear surface in the middle of the screen whose glass panel is\n\
                      frosted where the surface says (ext-background-effect). Toggled over IPC\n\
                      from a keybind.",
            ),
            radius: None,
            popups: None,
            clear: true,
        },
        LayerRule {
            namespace: TOAST_NS,
            comment: String::from(
                "Notification toasts: one surface per banner, sized to its card, sliding in\n\
                      at the top-right.",
            ),
            radius: Some(toast),
            popups: None,
            clear: false,
        },
        LayerRule {
            namespace: CENTER_NS,
            comment: String::from(
                "Notification center: a full-height frosted sidebar at the right edge,\n\
                      opened from the clock. Its Do Not Disturb menu is an xdg-popup of it.",
            ),
            radius: Some(popover),
            popups: Some(popover),
            clear: false,
        },
    ]
}

/// The line `gen-layer-rules.sh` looks for; everything after it goes into
/// the niri config.
const SENTINEL: &str = "//<<<GENERATED-RULES-BELOW>>>";

const HEADER: &str = "\
// GENERATED by `cargo run -q -- layer-rules > niri-layer-rule.kdl` from the
// design tokens (radii: sys.radius.popover, notification.card.radius) — do not
// edit by hand.
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
            // niri gives a layer-surface's blur xray by default, which shows
            // only the wallpaper; these cards float over windows, so they blur
            // what's below (popups already default to that)
            out.push_str("    background-effect {\n        blur true\n        xray false\n    }\n");
            out.push_str("    shadow {\n        on\n");
            let _ = writeln!(out, "        softness {SHADOW_SOFTNESS}");
            let _ = writeln!(out, "        offset x=0 y={SHADOW_Y}");
            let _ = writeln!(out, "        color \"{SHADOW_COLOR}\"");
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
            out.push_str("        background-effect {\n            blur true\n        }\n");
            out.push_str("    }\n");
        }
        out.push_str("}\n");
    }

    out
}

/// `^…$` with regex metacharacters escaped.
fn anchored(namespace: &str) -> String {
    let mut out = String::from("^");
    for c in namespace.chars() {
        if r"\.^$|?*+()[]{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('$');
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
    fn namespaces_are_escaped() {
        assert_eq!(
            kdl_string(&anchored("amane.taskbar.wifi")),
            r"^amane\\.taskbar\\.wifi$"
        );
        assert_eq!(anchored("a+b(c)"), r"^a\+b\(c\)$");
    }
}
