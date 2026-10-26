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
- **Small glass frosts less.** A piece of Regular glass no more than `SMALL_GLASS` (44) across its
  short side, a button or a chip, asks for niri's `glass-small`. It frosts at about 6 (2 passes,
  offset 1.5) with a narrower lens (bevel 12). Larger glass, the modules, menus and sheets, takes
  `glass`: about 16 (3 passes, offset 2) with bevel 16, so a module isn't all lens edge. This is
  the macOS 27 kit's frost: 6 for small glass, 16 for medium and large. Without `glass-small` in
  the config, small glass takes `glass`.
- **Standard** materials frost without a lens (`frost`). They are used inside the content layer:
  the bar, tooltips and app windows. `frost` blurs at about 32 (4 passes, offset 2) and saturates
  1.8, as macOS's materials do (Gaussian 30 pt, saturation 1.6 to 2.4).
- **Tone, not opacity.** The tints are thin (the glass's around 36% to 55%). They stay readable
  because niri's `tone` (0.6) squeezes the backdrop's brightness toward the tint's side first:
  under dark glass a white wallpaper reads as 60% grey, and under light glass a black one reads
  as 40%. Apple's materials do the same with a darken or lighten layer. Where niri doesn't tone the
  backdrop (a tint the surface fills itself, such as the bar's, or no niri material), the tint goes
  `UNTONED` (0.4) of the way to opaque instead. `adaptive-tint` is 0.25, so glass over a backdrop
  near its own brightness thickens only a little, not to milky white.

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
| `Window` | windowBackground | standard | opaque | no | an app window's content |
| `Sidebar` | sidebar | standard | `material.sidebar` | no | an app's sidebar |
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
  `layer_rules::APP_IDS`. As in macOS's System Settings, only the sidebar shows it: the window
  draws its content on the opaque `sys.color.window.bg`, and the sidebar cuts itself out of that
  (a `Mask` fill) and lays `sys.color.material.sidebar` over the frost. That is Apple's sidebar
  recipe: grey 0.965 at 84% in light. Without the generated config there is no rule to frost
  the window, and the sidebar is the opaque `sys.color.sidebar.bg.*`.
- **Inactive windows.** While another window has the focus, `Sidebar` and `Header` go halfway to
  opaque, calmer, as AppKit's inactive windows are. The window itself doesn't fade.
- **Header.** It is defined, but no page scrolls under a header yet.

## Rules

- **Pick by role.** Choose the material for what the piece is for, not for how it looks: the
  look changes with the settings.
- **No glass in content.** Rows, cards and controls on a material are fills
  (`fill_role(..)`), not glass of their own.
- **One stained control a view.** `Prominent` is for the one primary action. Don't colour
  several controls.
- **Small glass adapts, large glass is thick.** Only `Control` and `Banner` flip light or dark
  with their backdrop. Large glass that holds text is thicker, so it stays legible over anything.
- **A panel flips together, and seldom.** Apple: "small elements like navbars and tabbars …
  flip from light to dark based on the background. Bigger elements, like menus or sidebars,
  also adapt based on context, but they don't flip." Quick Settings' modules are one panel:
  `Material::adapted_group` keeps the theme's tint on all of them, thinned together from the
  mean of the readings behind them, while their labels, glyphs and fills flip with how bright
  the glass itself comes out, the tint laid over that mean ("symbols and glyphs … flip from
  light to dark, mirroring the glass"): dark labels above 0.6, light below 0.45
  (`glass_region::PANEL_LABELS`). So thin light glass over a blue wallpaper is blue glass
  with white labels, as macOS's Control Center is, while dark glass keeps white labels until
  the backdrop is near white (`glass_region::panel_adaptation`, `material::under_labels`).
  The readings arrive a frame after the first draw, so the decision is on the glass, not the
  backdrop, or a dark panel would flip to black labels and fills over a bright sky. Dock items
  and toasts stay small glass of their own, flipping whole at 0.35 / 0.65.
- **Thin is thin, thick is thick.** Regular glass's thin tint is 12% (light) / 30% (dark)
  with no lift (`glass` tone 1), so a module is the wallpaper with a touch of white or black,
  saturated 1.5. Thick glass, the sheets and the launcher, is 55% (light) / 60% (dark) on its
  own `glass-large` niri material: the 30 pt blur (4 passes), saturation 1.8 and a little
  lift (tone 0.9), so the sheet shows its backdrop's colour the way Apple's does. A plain
  module in the Customize catalog is a translucent fill on the sheet (`fill_role(Secondary)`)
  with no shadow, not a tint over it. The contrast floors let a flipping surface pass with
  either mode's labels over each wallpaper.
- **The edge.** niri's lens bends the backdrop in from past the shape's edge, as the recovered
  macOS 26 glass shaders do, so a rim continues what lies beside it (no stretched ring). The
  light comes from the top (`light-angle 270`) with an opposite fill (`far-rim 0.5`), Apple's
  preset, kept faint (`rim 0.12`, `far-rim 0.25`, `specular 0.1`: a hint of an edge, not a
  lit one) with next to no `glow` over the bevel. Glass is
  saturated 2.0, at the top of Apple's 1.6–2.4, so the backdrop comes through vivid.
- **Labels are vibrant and monochrome.** Text and glyphs on a material take `label(Level::..)`.
  The levels are primary, secondary, tertiary and quaternary, plus `fill_role(..)` and
  `separator()`. These are translucent, so the backdrop tints them, as Apple's label colours
  are. Quaternary isn't for thin or ultra-thin materials (a debug assertion checks it).
  A custom colour opts out of vibrancy. Keep colour for status and the primary action.

## Tokens

- `sys.color.material.ultra-thin` … `.ultra-thick` and `.bar` are the tints, thinnest first.
  `.sidebar` is an app sidebar's, over the window's frost.
- `sys.color.vibrant.label.*`, `.fill.*` and `.separator` are the labels, fills and separator.
  The values are the macOS 27 kit's vibrant greys as translucent white (dark) and black (light),
  which blend the same way and still sort onto the right vibrancy layer. On an opaque surface
  (solid glass, `Material::Content`) labels and fills come from `sys.color.label.*` and
  `sys.color.fill.*` instead, a little weaker, as the kit's content area is. The vibrant set is
  one set for every material, and the contrast floors check primary and secondary
  labels on every tint over black and white wallpaper (`tokens::rules`): toned by the niri
  material that draws the tint, and again untoned and thickened by `UNTONED`, the tint the kit
  uses without niri's material.
- `material.<name>.*` (component tokens) are niri's material settings. `amane-shell layer-rules`
  writes one niri material for each of them into `niri-layer-rule.kdl`, with a `-still` twin
  (`materialize false`) for reduced motion; see [layer-rules.md](../integration/layer-rules.md).

## Settings

- **Glass level.** Clear keeps 0.8 of every tint. Solid makes the tints opaque and turns niri's
  materials off, as Apple's Reduce Transparency does.
- **Higher contrast.** Like Solid, it turns the materials off and brings the hairlines back. It
  also raises every label level (secondary to at least 85%, tertiary 70%, quaternary 60%),
  doubles the fills, and strengthens the separator.
- **Reduced motion.** Glass asks for its material's `-still` twin, so the lens appears at once
  instead of building up (Apple: avoid animating into and out of blurs).
- **Vibrant labels** (an experiment, off by default: `vibrancy` in `appearance.json`, the
  Settings switch, or `AMANE_VIBRANCY=1`) makes every shell surface, popup and app window
  vibrant (`material::vibrant`, amane's `.vibrant(true)`). On niri with `niri-vibrancy-v1`,
  light grey labels, glyphs and fills are then added to the frosted backdrop and dark grey ones
  subtracted from it (Apple's plus-lighter and plus-darker), so they take on its colour, as
  Apple's vibrancy does. Solid glass and higher
  contrast turn it off. Elsewhere the translucent label colours stand in.
- **Adaptive glass** (an experiment, off by default) lets `Control` and `Banner` follow niri's
  readings of their backdrop; see `glass_region` and [components.md](../kit/components.md).

## Look test

`examples/liquid_glass.rs` is a look test, not shell code: glass cards over a wallpaper it draws
itself, lit by the kit's edge shader (`kit/src/glass.wgsl`) with stronger values, for judging the
glass edge before it goes into the shell. Its `//!` header lists the `GLASS_*` variables it
takes.
