#!/usr/bin/env python3
"""One-time seed: today's bar/menu token files -> a Penpot single-file export.

    python3 scripts/seed-penpot-tokens.py > seed.json
    cargo run -q -- tokens normalize seed.json > design-tokens/tokens.json

Lays the tokens out as the Design Token Manual §9 (sets ref, sys/base,
sys/color/{light,dark}, sys/density/{comfortable,compact}, components) with
the tier mapping of spec §5. Every value is read from the old files, so the
shell draws the same pixels. Places where two old values meet in one role
but differ are kept as separate roles and listed on stderr (the conflict
report) for a designer to converge in Penpot.

Written for Penpot 2.10-2.18: `$type` on every token (group-level types only
import from 2.17), themes list only the sets they enable (Penpot's import
enables every listed set), every theme has a description.

Deleted once the components read design-tokens/tokens.json.
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BAR = json.loads((ROOT / "design-tokens/bar.tokens.json").read_text())
MENU = json.loads((ROOT / "design-tokens/menu.tokens.json").read_text())


def node(tree, path):
    for part in path.split("."):
        tree = tree[part]
    return tree["$value"]


def resolved(tree, path):
    value = node(tree, path)
    while isinstance(value, str) and value.startswith("{"):
        value = node(tree, value[1:-1])
    return value


def num(tree, path):
    value = resolved(tree, path)
    n = value["value"] if isinstance(value, dict) else value
    return f"{n:g}"


def hex_color(tree, path):
    value = resolved(tree, path)
    byte = lambda c: round(max(0.0, min(1.0, c)) * 255)
    r, g, b = (byte(c) for c in value["components"])
    a = byte(value.get("alpha", 1.0))
    return f"#{r:02x}{g:02x}{b:02x}" + ("" if a == 255 else f"{a:02x}")


def bar(path):
    return num(BAR, path)


def menu(path):
    return num(MENU, path)


conflicts = []


def same(role, *sources):
    """One role fed by several old tokens: they must agree."""
    values = {f"{name} = {value}" for name, value in sources}
    distinct = {value for _, value in sources}
    if len(distinct) > 1:
        conflicts.append(f"{role}: " + ", ".join(sorted(values)))
    return sources[0][1]


def tok(ty, value):
    return {"$type": ty, "$value": value}


def density(px):
    return f"round({px} * {{sys.density.factor}})"


# ───── ref ─────────────────────────────────────────────────────────────────

ink = hex_color(MENU, "color.neutral.ink")
chalk = hex_color(MENU, "color.neutral.chalk")
red = same(
    "ref.color.red.58",
    ("bar color.urgent.red", hex_color(BAR, "color.urgent.red")),
    ("menu color.destructive.red", hex_color(MENU, "color.destructive.red")),
)
seed = same(
    "ref.color.accent.seed",
    ("bar color.accent.blue", hex_color(BAR, "color.accent.blue")),
    ("menu color.accent.blue", hex_color(MENU, "color.accent.blue")),
)

bar_ui = node(BAR, "font.ui.family")
menu_ui = node(MENU, "font.ui.family")
if bar_ui != menu_ui:
    conflicts.append(
        f"ref.font.family.ui: bar {bar_ui} vs menu {menu_ui} — kept the bar's "
        "(same first family, so no visible change while SF Pro Text is installed)"
    )

medium = node(BAR, "font.ui.weight.medium")
if medium not in (100, 200, 300, 400, 500, 600, 700, 800, 900, 950):
    conflicts.append(
        f"ref.font.weight.medium: {medium} is not accepted by Penpot — seeded as 500"
    )

ref = {
    "color": {
        "neutral": {
            "0": tok("color", "#000000"),
            "11": tok("color", ink),
            "97": tok("color", chalk),
            "100": tok("color", "#ffffff"),
        },
        "red": {"58": tok("color", red)},
        # derived from the seed by the runtime rule (manual §8.3), so the
        # default and a system accent of the same colour look identical
        "accent": {
            "seed": tok("color", seed),
            "on-seed": tok("color", "#ffffff"),
            "dim": tok("color", "#0060bf"),
            "bright": tok("color", "#add1ff"),
        },
    },
    "space": {str(n): tok("spacing", str(n)) for n in (2, 4, 6, 8, 10, 12)},
    "size": {str(n): tok("sizing", str(n)) for n in (12, 16)},
    "radius": {
        "0": tok("borderRadius", bar("radius.panel")),
        "5": tok("borderRadius", menu("radius.row")),
        "6": tok("borderRadius", bar("radius.pill")),
        "10": tok("borderRadius", "10"),
    },
    "border": {"1": tok("borderWidth", menu("space.separator"))},
    "font": {
        "family": {
            "ui": tok("fontFamilies", bar_ui),
            "icon": tok("fontFamilies", node(BAR, "font.icon.family")),
        },
        "weight": {
            "regular": tok("fontWeights", f"{node(BAR, 'font.ui.weight.regular')}"),
            "medium": tok("fontWeights", "500"),
        },
        "size": {
            "12": tok("fontSizes", bar("type.window.size")),
            "13": tok("fontSizes", "13"),
        },
    },
    "opacity": {"72": tok("opacity", menu("type.shortcut.hover-alpha"))},
}

# ───── sys/base ────────────────────────────────────────────────────────────


def gap(px):
    return tok("spacing", density(f"{{ref.space.{px}}}"))


def type_role(size, weight):
    return tok(
        "typography",
        {
            "fontFamilies": "{ref.font.family.ui}",
            "fontSizes": f"{{ref.font.size.{size}}}",
            "fontWeights": f"{{ref.font.weight.{weight}}}",
        },
    )


label = same(
    "sys.type.label size",
    ("bar type.label.size", bar("type.label.size")),
    ("menu type.label.size", menu("type.label.size")),
    ("menu type.shortcut.size", menu("type.shortcut.size")),
    ("bar type.clock.size", bar("type.clock.size")),
)
assert label == "13"

radius_popover = same(
    "sys.radius.popover",
    ("menu radius.card", menu("radius.card")),
    ("menu radius.submenu", menu("radius.submenu")),
    ("bar radius.tile (Wi-Fi/power card)", bar("radius.tile")),
)

sys_base = {
    "space": {
        "gap": {
            "2xs": gap(2),
            "xs": gap(4),
            "s": gap(6),
            "m": gap(8),
            "l": gap(10),
            "xl": gap(12),
        },
        "inset": {"s": gap(6), "m": gap(8), "l": gap(10)},
    },
    "size": {
        "icon": {"s": tok("sizing", "{ref.size.12}"), "m": tok("sizing", "{ref.size.16}")},
        "panel": {"height": tok("sizing", bar("size.height-compact"))},
        "popover": {
            "width": tok("sizing", bar("space.panel-w")),
            "height": tok("sizing", bar("space.panel-h")),
        },
        "list": {"max-height": tok("sizing", bar("space.list-h"))},
        "row": {
            "height": tok("sizing", density(bar("space.row-h"))),
            "trailing-width": tok("sizing", "120"),
        },
        "control": {"height": tok("sizing", "{sys.size.row.height} - 10")},
    },
    "radius": {
        "popover": tok("borderRadius", "{ref.radius.10}"),
        "raised": tok("borderRadius", "{ref.radius.6}"),
    },
    "border": {"hairline": tok("borderWidth", "{ref.border.1}")},
    "effect": {"backdrop-blur": tok("number", bar("effect.blur"))},
    "type": {
        "caption": type_role(12, "regular"),
        "label": type_role(13, "regular"),
        "title": type_role(13, "medium"),
        "numeric": type_role(13, "medium"),
        "icon": tok("fontFamilies", "{ref.font.family.icon}"),
    },
}
assert radius_popover == "10"

# ───── sys/color/{light,dark} ──────────────────────────────────────────────


def colors(mode):
    b = lambda leaf: hex_color(BAR, f"theme.{mode}.{leaf}")
    m = lambda leaf: hex_color(MENU, f"theme.{mode}.{leaf}")
    neutral = "{ref.color.neutral.11}" if mode == "light" else "{ref.color.neutral.97}"
    accent_text = "{ref.color.accent.dim}" if mode == "light" else "{ref.color.accent.bright}"

    if b("fg") != m("muted"):
        conflicts.append(
            f"[{mode}] secondary text: bar fg {b('fg')} vs menu muted {m('muted')} — "
            "kept as sys.color.content.secondary and sys.color.surface.popover.fg-secondary"
        )
    if b("surface") != m("surface"):
        conflicts.append(
            f"[{mode}] surface: bar {b('surface')} (also the Wi-Fi and power cards) vs "
            f"menu {m('surface')} — kept as surface.panel.bg and surface.popover.bg; "
            "Wi-Fi and power still read the panel pair"
        )
    c = lambda value: tok("color", value)
    return {
        "color": {
            "surface": {
                "panel": {"bg": c(b("surface")), "fg": c(b("fg-strong"))},
                "popover": {
                    "bg": c(m("surface")),
                    "fg": c(neutral),
                    "fg-secondary": c(m("muted")),
                    "stroke": c(m("border")),
                },
                "raised": {"bg": c(b("pill"))},
            },
            "content": {"secondary": c(b("fg"))},
            "accent": {
                "bg": c("{ref.color.accent.seed}"),
                "fg": c("{ref.color.accent.on-seed}"),
                "text": c(accent_text),
            },
            "status": {
                "critical": {
                    "bg": c("{ref.color.red.58}"),
                    "text": c("{ref.color.red.58}"),
                    "hover": c(m("hover-destructive-fill")),
                }
            },
            "state": {"hover": c(m("hover-fill")), "selected": c(b("pill-focused"))},
            "separator": c(m("separator")),
        }
    }


# ───── components ──────────────────────────────────────────────────────────

components = {
    "bar": {
        "overview": {"height": tok("sizing", bar("size.height-overview"))},
        "radius": tok("borderRadius", "{ref.radius.0}"),
        "panel": {"padding-y": tok("spacing", density(bar("space.panel-pad-y")))},
        "pill": {"padding-y": tok("spacing", density(bar("space.pill-pad-y")))},
        "workspace-dot": {
            "size": tok("sizing", bar("space.dot-size")),
            "active-size": tok("sizing", bar("space.dot-active")),
        },
        "indicator": {"width": tok("sizing", bar("space.dot-active"))},
        "window-item": {
            "max-width": tok("sizing", bar("space.window-max")),
            "radius": tok("borderRadius", "{ref.radius.10}"),
        },
        "volume": {"width": tok("sizing", "58")},
    },
    "menu": {
        "row": {
            "height": tok("sizing", density(menu("space.row-height"))),
            "radius": tok("borderRadius", "{ref.radius.5}"),
        },
        "card": {"padding": tok("spacing", density(menu("space.card-padding")))},
        "icon-slot": {"width": tok("sizing", "{ref.size.16}")},
        "chevron-slot": {"width": tok("sizing", menu("space.chevron-slot"))},
        "min-width": tok("sizing", menu("space.min-width")),
        "max-width": tok("sizing", menu("space.max-width")),
        "submenu": {"offset": tok("dimension", menu("space.submenu-offset"))},
        "shortcut": {"hover-opacity": tok("opacity", "{ref.opacity.72}")},
        "label": {"advance-ratio": tok("number", "0.56")},
    },
    "wifi": {
        "switch": {"width": tok("sizing", "44"), "height": tok("sizing", "22")},
        "icon-button": {"width": tok("sizing", "28")},
        "button": {"width": tok("sizing", "72")},
    },
}

# old values that map onto shared roles must equal them
for role, want, old, px in [
    ("sys.space.inset.m", "8", "bar space.panel-pad-x", bar("space.panel-pad-x")),
    ("sys.space.inset.m", "8", "bar space.pill-pad-x", bar("space.pill-pad-x")),
    ("sys.space.inset.m", "8", "bar space.tile-pad", bar("space.tile-pad")),
    ("sys.space.inset.l", "10", "menu space.row-h", menu("space.row-h")),
    ("sys.space.gap.xl", "12", "bar space.section-gap", bar("space.section-gap")),
    ("sys.space.gap.s", "6", "bar space.item-gap", bar("space.item-gap")),
    ("sys.space.gap.m", "8", "menu space.gap", menu("space.gap")),
    ("sys.space.gap.m", "8", "bar space.panel-gap", bar("space.panel-gap")),
    ("sys.space.gap.xs", "4", "menu space.separator-margin", menu("space.separator-margin")),
    ("sys.size.icon.m", "16", "bar space.icon-size", bar("space.icon-size")),
    ("sys.size.icon.s", "12", "bar type.window.size (lock icon)", bar("type.window.size")),
    ("menu.icon-slot.width", "16", "menu space.icon-slot", menu("space.icon-slot")),
]:
    if px != want:
        conflicts.append(f"{role} = {want}, but {old} = {px}")

# ───── envelope ────────────────────────────────────────────────────────────

ORDER = [
    "ref",
    "sys/base",
    "sys/color/light",
    "sys/color/dark",
    "sys/density/comfortable",
    "sys/density/compact",
    "components",
]


def theme(group, name, description, sets):
    return {
        "name": name,
        "group": group,
        "description": description,
        "isSource": False,
        "selectedTokenSets": {s: "enabled" for s in sets},
    }


export = {
    "ref": {"ref": ref},
    "sys/base": {"sys": sys_base},
    "sys/color/light": {"sys": colors("light")},
    "sys/color/dark": {"sys": colors("dark")},
    "sys/density/comfortable": {"sys": {"density": {"factor": tok("number", "1")}}},
    "sys/density/compact": {"sys": {"density": {"factor": tok("number", "0.875")}}},
    "components": components,
    "$themes": [
        theme("mode", "light", "Light surfaces", ["ref", "sys/base", "sys/color/light", "components"]),
        theme("mode", "dark", "Dark surfaces", ["ref", "sys/base", "sys/color/dark", "components"]),
        theme("density", "comfortable", "Default spacing", ["sys/density/comfortable"]),
        theme("density", "compact", "Tighter spacing and rows", ["sys/density/compact"]),
    ],
    "$metadata": {
        "tokenSetOrder": ORDER,
        "activeThemes": ["mode/dark", "density/comfortable"],
        "activeSets": ["ref", "sys/base", "sys/color/dark", "components", "sys/density/comfortable"],
    },
}

json.dump(export, sys.stdout, indent=2, ensure_ascii=False)
print()

print("Conflict report (kept as separate roles; converge in Penpot):", file=sys.stderr)
for line in conflicts or ["none"]:
    print(f"  - {line}", file=sys.stderr)
