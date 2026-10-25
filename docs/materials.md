# Materials

The shell's glass follows the materials in Apple's Human Interface Guidelines. A piece of glass
asks for the **material of its role**, never for a colour (`amane_kit::material::Material`). The
material then decides the rest:

- which of niri's materials frosts it;
- how thick its tint is;
- whether it turns light or dark with its backdrop;
- the labels, fills and separator drawn on it.

The glass setting, reduced motion and higher contrast change every material at once.

## Two families

- **Glass**, Apple's Liquid Glass, is the layer of controls and navigation that floats over
  content. niri's `glass` material lenses it. Its **Clear** variant (`glass-clear`) is for media
  only: it is dark whatever the theme, keeps 0.7 of its tint, and keeps all of it over a bright
  backdrop. That is Apple's 35% dimming layer.
- **Standard** materials frost without a lens (`frost`). They are used inside the content layer:
  the bar, tooltips and app windows.

## Roles

| `Material` | Apple's | Family | Thickness | Adapts | Used by |
|---|---|---|---|---|---|
| `Control` | Control Center module, tab bar | glass | thin | yes | Quick Settings modules, the dock |
| `Banner` | notification banner | glass | thin | yes | toasts |
| `Prominent` | prominent button | glass, stained with the accent | — | no | Quick Settings' Done |
| `Menu` | menu | glass | regular | no | context menus, every popup |
| `Popover` | popover | glass | regular | no | dropdowns, a Quick Settings page (it grows out of a `Control`) |
| `Sheet` | sheet, sidebar | glass | thick | no | the launcher, the notification center, Customize |
| `ToolTip` | toolTip | standard | thick | no | tooltips |
| `Bar` | bar | standard | bar | no | the status bar |
| `Media` | Clear glass | glass, clear | ultra-thin | dims | nothing yet. The media module's art is inside it, not behind it. |
| `Window` | windowBackground | standard | thick | no | an app window |
| `Sidebar` | sidebar | standard | regular | no | an app's sidebar |
| `Header` | headerView | standard, within the window | regular | no | an app's header over its scrolling page |
| `Content` | contentBackground | standard | ultra-thick | no | grouped content inside a material |

A piece of glass is built with its material's methods:

- `shape(..)` is the frost niri draws behind it, as a blur shape. It is `shape_tinted(..)` for
  glass on its way from one material to another.
- `fill()` is what the surface draws: clear where niri lays the tint on itself.
- `lit(..)` adds the shader's edge where niri doesn't draw one.
- `stroke(..)` is a hairline, there only where niri draws no edge.
- `adapted(id, ..)` builds the piece in the mode its backdrop asks for, if the material adapts.

## App windows

- **Window frost.** An amane app's window frosts what's behind it in niri's `frost` material.
  The rule comes from `layer-rules`, which writes a `window-rule` for each app in
  `layer_rules::APP_IDS`. The window draws `Material::Window`'s tint over it, and stays solid
  wherever `frost` isn't offered: without the generated config there is no rule to frost it.
- **Inactive windows.** While another window has the focus, `Window`, `Sidebar` and `Header` go
  halfway to opaque, calmer, as AppKit's inactive windows are.
- **Sidebar and Header.** Both are defined, but amane's windows can't send blur shapes yet, so the
  sidebar sits on the window's frost. No page scrolls under a header yet either.

## Rules

- **Pick by role.** Choose the material for what the piece is for, not for how it looks: the
  look changes with the settings.
- **No glass in content.** Rows, cards and controls on a material are fills
  (`fill_role(..)`), not glass of their own.
- **One stained control a view.** `Prominent` is for the one primary action. Don't colour
  several controls.
- **Small glass adapts, large glass is thick.** Only `Control` and `Banner` flip light or dark
  with their backdrop. Large glass that holds text is thicker, so it stays legible over anything.
- **Labels are vibrant and monochrome.** Text and glyphs on a material take `label(Level::..)`.
  The levels are primary, secondary, tertiary and quaternary, plus `fill_role(..)` and
  `separator()`. These are translucent, so the backdrop tints them, as Apple's label colours
  are. Quaternary isn't for thin or ultra-thin materials (a debug assertion checks it).
  A custom colour opts out of vibrancy. Keep colour for status and the primary action.

## Tokens

- `sys.color.material.ultra-thin` … `.ultra-thick` and `.bar` are the tints, thinnest first.
- `sys.color.vibrant.label.*`, `.fill.*` and `.separator` are the labels, fills and separator.
  They are one set for every material, and the contrast floors check primary and secondary
  labels on every tint over black and white wallpaper (`tokens::rules`).
- `material.<name>.*` (component tokens) are niri's material settings. `amane-shell layer-rules`
  writes one niri material for each of them into `niri-layer-rule.kdl`, with a `-still` twin
  (`materialize false`) for reduced motion; see [layer-rules.md](layer-rules.md).

## Settings

- **Glass level.** Clear keeps 0.8 of every tint. Solid makes the tints opaque and turns niri's
  materials off, as Apple's Reduce Transparency does.
- **Higher contrast.** Like Solid, it turns the materials off and brings the hairlines back. It
  also raises every label level (secondary to at least 85%, tertiary 70%, quaternary 60%),
  doubles the fills, and strengthens the separator.
- **Reduced motion.** Glass asks for its material's `-still` twin, so the lens appears at once
  instead of building up (Apple: avoid animating into and out of blurs).
- **Adaptive glass** (an experiment, off by default) lets `Control` and `Banner` follow niri's
  readings of their backdrop; see `glass_region` and [components.md](components.md).
