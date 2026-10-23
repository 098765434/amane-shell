# amane-shell Design Token Manual
 
For designers working in Penpot and developers working in the amane-shell code. Version 1.1,
23 Oct 2026. The implementation is specified in `spec-design-tokens-v2.md` (revision 5) and
lives in `src/tokens/`. Where this manual says "must", a check in CI enforces it (§13).
 
---
 
## 1. The rules on one page
 
1. **Penpot is the source of truth.** Every colour, size, gap, radius, font and timing the
   shell draws is a token in the Penpot file *amane-shell design system*. Nobody edits
   `design-tokens/tokens.json` by hand.
2. **Three tiers, one direction.** `ref.*` (raw values) ← `sys.*` (decisions) ← component
   tokens (`bar.*`, `menu.*`, `wifi.*`, `power.*`, `osd.*`, `notification.*`). References only
   point leftwards.
3. **Code reads `sys.*` and its own component's tokens. Never `ref.*`.**
4. **Light/dark changes only `sys.color.*`.** Primitives never change with mode; component
   tokens never contain mode values.
5. **Density changes only through `sys.density.factor`.** Spacing and control sizes multiply
   by it; radii, strokes and font sizes don't.
6. **The accent is four primitives** (`ref.color.accent.*`). The runtime replaces them with
   the user's accent; everything that references them follows. Never name an accent after a
   hue.
7. **Surfaces come in pairs.** Every surface has `bg`, `fg` and `stroke`. Use the `fg` of the
   surface you're drawing on.
8. **Component tokens are the exception.** Add one only for a value unique to that component
   or a deliberate departure from its surface role — never to rename a `sys` token.
9. **Use Penpot's specific types** (`spacing`, `sizing`, `borderRadius`, …) so tokens show up
   in the right pickers.
10. **Logical pixels only.** HiDPI is the compositor's job; text scale is the runtime's.
---
 
## 2. How the pieces fit
 
```
Penpot file "amane-shell design system"            ← designers edit here
   │  Tokens panel → Export → Single file
   ▼
design-tokens/tokens.json  (normalized, committed)  ← developers commit it
   │  include_str! + resolver that behaves like Penpot
   ▼
resolved values per (mode, density, accent)         ← cached at runtime
   │  build.rs: one Rust function per sys.* and component token
   ▼
bar · menus · Wi-Fi panel · OSD · notifications · niri layer rules
```
 
What the running shell may change on top of Penpot, and nothing else:
 
| Runtime input | Source | Acts on |
|---------------|--------|---------|
| Mode (light/dark) | `AMANE_THEME`, else system preference, else dark | selects theme `mode/light` or `mode/dark` |
| Density | `AMANE_DENSITY`, else `comfortable` | selects theme `density/comfortable` or `density/compact` |
| Accent | system accent, if published | replaces `ref.color.accent.*` before resolving |
| Text scale | `AMANE_TEXT_SCALE`, later the system setting | multiplies font sizes in `sys.type.*` after resolving |
| Output scale (HiDPI) | niri | below the token system; tokens stay logical px |
 
Everything else is exactly what Penpot shows.
 
---
 
## 3. The three tiers
 
| Tier | Prefix | Holds | May reference | Read by code? | Changes with mode? |
|------|--------|-------|---------------|---------------|--------------------|
| 1 · Primitives | `ref.` | raw values and scales: palette, spacing steps, font families, sizes, radii, opacities | nothing (literals only) | **no** | no |
| 2 · Semantic | `sys.` | decisions by role: surfaces, content, accent, status, states, spacing roles, control sizes, typography roles, shadows | `ref.*`, other `sys.*` | yes | colours yes; dimensions via density |
| 3 · Component | `bar.`, `menu.`, `wifi.`, `power.`, `osd.`, `notification.` | values unique to one component, or its deliberate departures | `sys.*`; `ref.*` only for non-colour values; `sys.density.factor` | only by that component | no |
 
```
ref.color.neutral.11 ──► sys.color.surface.popover.bg (in sys/color/dark) ──► menu code
ref.space.10 ──────────► sys.space.inset.l ─────────────────────────────────► wifi and menu code
sys.density.factor ────► menu.row.height = round(24 * {sys.density.factor}) ► menu code
```
 
Why three tiers: it's the shape every mature system converged on — Material 3
(reference → system → component), Fluent 2 (global → alias → control), GitHub Primer
(base → functional → component), Adobe Spectrum, Brad Frost's tiers 1–3. The middle tier
does the work: it's where theming happens ("the alias layer is where theming happens" —
Fluent). Primer and Spectrum both keep component tokens rare. OS toolkits (libadwaita, KDE,
Apple) theme their chrome by *surface roles*, not by widget, which is exactly the semantic
tier.
 
Why tier prefixes in the name: Penpot token names are global across all active sets and
designers pick tokens from one list. `ref.` and `sys.` tell everyone at a glance what's
allowed where, and let CI check it with a string prefix.
 
**Budget.** For a shell of five components, roughly 60 primitives, 70 semantic tokens and
15 component tokens is healthy. If the component tier grows bigger than the semantic tier,
semantic roles are missing.
 
---
 
## 4. Naming
 
### 4.1 Grammar
 
```
ref.<category>.<scale-or-family>[.<step>]          ref.color.neutral.11   ref.space.8
sys.<category>.<role>[.<variant>][.<part>|.<state>] sys.color.surface.popover.bg   sys.space.inset.m
<component>.<part>.<property>[.<state>]            menu.row.height   osd.progress.height
```
 
- Lowercase. Dots separate levels; hyphens join words inside a level (`padding-x`,
  `on-seed`). No spaces, no camelCase, no `$`.
- **Most general first, most specific last.** `sys.color.surface.popover.bg`, not
  `sys.color.bg.popover`.
- **Name the role, not the value**, in `sys.*` and components: `sys.color.accent.bg`, not
  `sys.color.blue`; `sys.space.inset.m`, not `sys.space.8`.
- **Name the value** in `ref.*` scales: `ref.space.8` is 8 px, `ref.color.neutral.11` has a
  lightness of about 11. A primitive never changes meaning, so its name can say what it is.
- Size variants use `xs, s, m, l, xl` (add `2xs`/`2xl` at the ends). Ordinals (`1, 2, 3`)
  only for elevation steps.
### 4.2 Vocabulary
 
Use these words; add new ones only by updating this manual.
 
| Level | Words |
|-------|-------|
| `ref` categories | `color`, `space`, `size`, `radius`, `border`, `font`, `opacity`, `duration`, `shadow` |
| `sys` categories | `color`, `space`, `size`, `radius`, `border`, `type`, `opacity`, `shadow`, `motion`, `density`, `effect` |
| Surfaces | `panel` (the bar), `popover` (menus, Wi-Fi and power panels, calendar), `osd` (volume/brightness), `notification`, `raised` (rows, chips and cards inside a surface) |
| Surface parts | `bg`, `fg`, `stroke` |
| Content levels | `primary`, `secondary`, `tertiary`, `disabled` |
| Accent | `bg` (fill), `fg` (text/icons on the fill), `text` (accent-coloured text on a normal surface) |
| Status | `critical`, `warning`, `success` — each with `bg`, `fg`, `text` |
| States | `hover`, `pressed`, `selected`, `focus`, `disabled` |
| Spacing roles | `inset` (padding inside a surface or control), `gap` (between siblings) |
| Size roles | `control` (button and field heights), `icon`, `row` (list rows), `list`; `panel` and `popover` (the size of those surfaces) |
| Type roles | `caption`, `label`, `body`, `title`, `numeric` |
| Component parts | `row`, `item`, `icon`, `label`, `field`, `button`, `switch`, `list`, `progress`, `card`; properties `height`, `width`, `min-width`, `max-width`, `padding-x`, `padding-y`, `gap`, `radius`, `size` |
 
### 4.3 Do and don't
 
| Don't | Do | Why |
|-------|----|-----|
| `color.accent.blue` | `ref.color.accent.seed` | the user may pick green |
| `sys.color.dark.surface` | `sys.color.surface.panel.bg` in set `sys/color/dark` | mode is a set, not a name |
| `menu.bg` = `{sys.color.surface.popover.bg}` | read `sys.color.surface.popover.bg` in menu code | pure renames bloat the system |
| `sys.space.8` | `sys.space.gap.m` | roles survive value changes |
| `ref.space.small` | `ref.space.6` | primitives name their value |
| `bar.size.wifi-panel-w` | `wifi.panel.width` | the Wi-Fi panel is its own component |
| `space.row-h` holding a height | `menu.row.height` | category must match meaning |
| `menu.hoverAlpha` | `menu.shortcut.hover-opacity` | lowercase, hyphens |
| `sys.color.text-grey` | `sys.color.content.secondary` | role, not appearance |
 
---
 
## 5. Tier 1 — primitives (`ref.*`)
 
Raw, context-free, mode-free values. Designers build scales here; nothing outside `sys.*`
and component non-colour values may use them.
 
| Group | Penpot type | Rule | Seed values |
|-------|-------------|------|-------------|
| `ref.color.neutral.<L>` | `color` | `<L>` = CIE L\* lightness rounded (0 black … 100 white); CI allows ±2 | `.0` #000000, `.11` #1d1d1f, `.97` #f5f5f7, `.100` #ffffff |
| `ref.color.<hue>.<L>` | `color` | same rule for status hues | `ref.color.red.58` #ff453a |
| `ref.color.accent.seed` | `color` | the default accent | #0a84ff |
| `ref.color.accent.on-seed` | `color` | text/icons on the seed (§8.3) | #ffffff |
| `ref.color.accent.dim` | `color` | seed with OKLab L ≤ 0.50 — accent text on light surfaces | #0060bf |
| `ref.color.accent.bright` | `color` | seed with OKLab L ≥ 0.85 — accent text on dark surfaces | #add1ff |
| `ref.space.<px>` | `spacing` | 2 px steps up to 12, then 4 px | `.2 .4 .6 .8 .10 .12` |
| `ref.size.<px>` | `sizing` | shared sizes only (icons, controls) | `.12 .16` |
| `ref.radius.<px>` | `borderRadius` | | `.0 .5 .6 .10` |
| `ref.border.<px>` | `borderWidth` | | `.1` |
| `ref.font.family.<name>` | `fontFamilies` | full fallback stack, first entry is the design font | `ui` [SF Pro Text, Inter, Atkinson Hyperlegible, sans-serif], `icon` [Material Symbols Rounded, Material Icons Round, sans-serif] |
| `ref.font.weight.<name>` | `fontWeights` | Penpot accepts 100–900 in hundreds and 950 | `regular` 400, `medium` 500 |
| `ref.font.size.<px>` | `fontSizes` | | as used |
| `ref.opacity.<percent>` | `opacity` | | `.38` … |
| `ref.duration.<ms>` | `number` | milliseconds | as used |
 
Don't:
- put a translucent colour in `ref` (`rgba(255,255,255,0.06)` is a decision — it goes in
  `sys/color/*`);
- name a primitive after where it's used (`ref.color.menu-grey`);
- reference another token from a primitive.
---
 
## 6. Tier 2 — semantic (`sys.*`)
 
The vocabulary the shell is built from. Every colour role has a light and a dark value; every
dimension role may follow density.
 
### 6.1 Colour roles
 
| Token | Meaning |
|-------|---------|
| `sys.color.surface.panel.bg / .fg / .stroke` | the bar |
| `sys.color.surface.popover.bg / .fg / .stroke` | context menus, Wi-Fi panel, other popovers |
| `sys.color.surface.osd.bg / .fg / .stroke` | volume/brightness OSD (reserved) |
| `sys.color.surface.notification.bg / .fg / .stroke` | notifications (reserved) |
| `sys.color.surface.raised.bg` | chips, rows and cards inside any surface (the bar's pill) |
| `sys.color.surface.popover.fg-secondary` | secondary text in menus (differs from `content.secondary` until converged) |
| `sys.color.content.secondary / .tertiary / .disabled` | less important text and icons on any surface |
| `sys.color.accent.bg / .fg / .text` | accent fill, content on it, accent-coloured text |
| `sys.color.status.critical.bg / .fg / .text` | urgent/destructive (also `warning`, `success` when needed) |
| `sys.color.status.critical.hover` | hover fill on a destructive row |
| `sys.color.state.hover / .pressed / .selected` | overlay fills painted on top of a surface |
| `sys.color.focus.ring` | keyboard focus outline |
| `sys.color.separator` | dividers inside a surface |
| `sys.color.scrim`, `sys.color.shadow` | dimming behind modal surfaces; shadow colour |
 
Primary content is the surface's own `fg`. Secondary, tertiary and disabled content are
shared across surfaces; if a surface needs its own, add `sys.color.surface.<s>.fg-secondary`
rather than a component token.
 
How to set them:
- **Accent**: `sys.color.accent.bg = {ref.color.accent.seed}`, `.fg = {ref.color.accent.on-seed}`;
  `.text = {ref.color.accent.dim}` in light, `{ref.color.accent.bright}` in dark.
- **States** follow Material's state-layer rule: the surface's `fg` at 8 % (hover),
  10 % (pressed), 12 % (selected, when selection isn't shown with the accent). Write them as
  `rgba({sys.color.surface.popover.fg}, 0.08)` once that's verified in Penpot (§11.5),
  literal `rgba()` until then. Disabled content is the content colour at 38 % opacity.
- **Elevation**: in dark mode a raised surface is *lighter* than what it sits on
  (popover lighter than panel, `raised` lighter still); in light mode use shadow plus stroke.
### 6.2 Dimension roles
 
| Token | Penpot type | Value |
|-------|-------------|-------|
| `sys.density.factor` | `number` | set by the density sets only: 1 comfortable, < 1 compact |
| `sys.space.gap.2xs … xl` | `spacing` | `round({ref.space.N} * {sys.density.factor})` |
| `sys.space.inset.xs … l` | `spacing` | same pattern |
| `sys.size.icon.s / .m / .l` | `sizing` | not density-scaled |
| `sys.size.panel.height` | `sizing` | the bar's height; popovers sit above it |
| `sys.size.popover.width / .height` | `sizing` | the Wi-Fi and power panels |
| `sys.size.list.max-height` | `sizing` | scrolling lists inside a popover |
| `sys.size.row.height` | `sizing` | `round(40 * {sys.density.factor})` — list rows in popovers |
| `sys.size.row.trailing-width` | `sizing` | room a row keeps for its trailing content |
| `sys.size.control.<variant>` | `sizing` | `round(<px> * {sys.density.factor})` — only for heights shared by two or more components |
| `sys.radius.popover / .control / .osd / .raised` | `borderRadius` | not density-scaled |
| `sys.border.hairline` | `borderWidth` | `{ref.border.1}` |
| `sys.opacity.disabled` | `opacity` | `{ref.opacity.38}` |
| `sys.shadow.popover / .osd` | `shadow` | |
| `sys.motion.duration.short / .medium` | `number` | ms, `{ref.duration.N}` |
| `sys.effect.backdrop-blur` | `number` | px of in-surface blur under translucent surfaces |
 
Always `round()` density-scaled values in Penpot, so Penpot and the shell land on the same
whole pixel.
 
### 6.3 Typography roles
 
`sys.type.caption`, `.label`, `.body`, `.title`, `.numeric` — Penpot `typography`
composites whose members reference `ref.font.*`. `numeric` is for clocks and percentages
(tabular figures). Five roles is enough for a shell; add one only when a size is used by two
components and matches no role. `sys.type.icon` (`fontFamilies`) is the icon font stack.

Today: `caption` 12/regular (secondary lines, small controls), `label` 13/regular, `title`
13/medium (headings, focused titles), `numeric` 13/medium (the clock); `body` is unused.
 
---
 
## 7. Tier 3 — component tokens
 
Components: `bar`, `menu`, `wifi`, `power`, `osd`, `notification`. Each component reads its
surface role from `sys` directly:
 
| Component | Surface role |
|-----------|--------------|
| `bar` | `sys.color.surface.panel.*` |
| `menu` | `sys.color.surface.popover.*` |
| `wifi`, `power` | `sys.color.surface.popover.*` (they still read `panel.*` until the seed's surface conflict is converged) |
| `osd` | `sys.color.surface.osd.*` |
| `notification` | `sys.color.surface.notification.*` |
 
**Add a component token only if:**
1. the value is unique to this component and not part of a shared scale
   (`osd.width = 240`, `menu.min-width`, `wifi.row.height`); or
2. the component deliberately departs from its role in a mode-free way (a fixed size, a
   different radius) — still aliasing `sys` or `ref` where possible; or
3. designers need one knob for one component that would otherwise mean editing a shared role.
**Never add one when:**
- it only renames a `sys` token (`menu.bg = {sys.color.surface.popover.bg}`);
- it would need a different value in light and dark — that's a `sys` colour role
  (add a surface variant like `sys.color.surface.panel.fg-secondary`);
- the role is missing — add the `sys` role instead if a second component could use it.
**Promote** a component token to `sys` as soon as a second component needs the same value.
 
Density: component dimensions that should follow density reference
`{sys.density.factor}` themselves: `menu.row.height = round(24 * {sys.density.factor})`.
 
---
 
## 8. Theme dimensions
 
### 8.1 Light and dark
 
- Sets `sys/color/light` and `sys/color/dark` define **the same names** with different
  values, and contain **only `sys.color.*`**.
- Theme group `mode`, themes `light` and `dark`. Required.
### 8.2 Density
 
- Sets `sys/density/comfortable` and `sys/density/compact` each define **only
  `sys.density.factor`** (comfortable = 1, today's look).
- Theme group `density`, themes `comfortable` and `compact`. Required.
- Scales: `sys.space.*`, `sys.size.control.*`, component heights/paddings that opt in.
  Doesn't scale: radii, strokes, icons, font sizes.
### 8.3 Accent
 
- Penpot holds the default accent in the four `ref.color.accent.*` primitives.
- At runtime, when the system publishes an accent, the shell replaces those four before
  resolving: `seed` = the system colour; `on-seed` = white if white has at least 3:1
  contrast on the seed, else black at 80 %; `dim` and `bright` = the seed with OKLab
  lightness clamped to ≤ 0.50 and ≥ 0.85 (libadwaita's rule), with chroma reduced where
  needed to stay inside sRGB at that lightness. Everything referencing them follows.
- So: **anything accent-coloured must reference `sys.color.accent.*`**, never repeat the
  hex value, or it won't follow the user's accent.
- Previewing other accents in Penpot (optional): theme group `accent` with sets
  `accent/blue`, `accent/green`, … each setting the four `ref.color.accent.*` values. The
  shell ignores this group.
### 8.4 Text scale and HiDPI
 
- Text scale is a runtime multiplier on `sys.type.*` font sizes; layouts must let rows grow.
  Nothing in Penpot.
- HiDPI: all values are logical px. Never make a "@2x" token.
---
 
## 9. The Penpot file
 
### 9.1 Sets, in this order
 
| Set | Contains only |
|-----|---------------|
| `ref` | `ref.*` |
| `sys/base` | `sys.*` except `sys.color.*` and `sys.density.*` |
| `sys/color/light` | `sys.color.*` |
| `sys/color/dark` | `sys.color.*` (same names) |
| `sys/density/comfortable` | `sys.density.factor` |
| `sys/density/compact` | `sys.density.factor` |
| `accent/<name>` (optional) | `ref.color.accent.*` |
| `components` | `bar.*`, `menu.*`, `wifi.*`, `power.*`, `osd.*`, `notification.*` |
 
Later sets win on name clashes; the only intended clashes are the accent preview sets
overriding `ref.color.accent.*`.
 
### 9.2 Themes
 
| Group | Theme | Enables |
|-------|-------|---------|
| `mode` | `light` | `ref`, `sys/base`, `sys/color/light`, `components` |
| `mode` | `dark` | `ref`, `sys/base`, `sys/color/dark`, `components` |
| `density` | `comfortable` | `sys/density/comfortable` |
| `density` | `compact` | `sys/density/compact` |
| `accent` (optional) | `blue`, `green`, … | `accent/<name>` |
 
Penpot ignores tokens in disabled sets when resolving, so **every `mode` theme must enable
`ref`, `sys/base` and `components`**, or references break. Every theme needs a description
(it can be empty) or Penpot's own import fails. A theme lists only the sets it enables: Penpot's
import enables every set a theme lists, even one marked `"disabled"`.

Every token carries its own `$type` (Penpot before 2.17 can't import a type set on a group).
Avoid the `boolean` and `string` types: Penpot imports them but has no panel to show them.
 
### 9.3 Applying tokens to layers in Penpot
 
- Use the token whose type matches the property: `spacing` tokens for gaps and padding,
  `sizing` for widths and heights, `borderRadius` for corners. That's why the types matter.
- Draw a hover row as a rectangle with fill `sys.color.state.hover` on top of the row, the
  same way the shell paints it.
- Build mockups in a separate file that uses *amane-shell design system* as a connected
  library, so everyone shares one token set.
- Check both `mode` themes and both `density` themes before exporting.
---
 
## 10. Workflows
 
### 10.1 Designer: change a value
 
1. Edit the token in Penpot. Check every `mode` × `density` combination: no red pills.
2. Tokens → Export → Single file.
3. Hand the file to a developer, or open a PR with it (§10.3).
### 10.2 Designer: add or rename a token
 
- **New colour need** → is there a `sys.color` role for it? Use it. If not, add the role in
  **both** `sys/color/light` and `sys/color/dark`. Never add colour tokens to `components`
  unless they alias a mode-free value.
- **New size or gap** → on a shared scale? Add/choose a `sys` role. Unique to one component?
  Add a component token (§7).
- **Rename** → use Penpot's rename, which offers to remap references. Tell developers: a
  token the code reads will fail CI by name until the code is updated in the same PR.
- **Delete** → if CI fails with "code reads X", the code still needs it.
### 10.3 Developer: bring an export in
 
```
amane-shell tokens normalize ~/Downloads/tokens.json > design-tokens/tokens.json
amane-shell tokens check design-tokens/tokens.json   # rules + diff against HEAD
cargo test
```
 
`check` prints changed, added and removed tokens. If `niri-layer-rule.kdl` is out of date
(a radius changed), regenerate it in the same PR:
`cargo run -q -- layer-rules > niri-layer-rule.kdl`.
 
### 10.4 Developer: read a token
 
Every `sys.*` and component token is a function, generated from `tokens.json` at build time
and named after the token. Call it where the value is used:

```rust
use crate::tokens::{Styled, menu, sys};

Rectangle::new()
    .fill(sys::color::surface::popover::bg())   // sys.color.surface.popover.bg
    .padding(sys::space::inset::l())             // sys.space.inset.l
    .height(menu::row::height());                // menu.row.height
Text::new("Open").style(sys::r#type::label());  // family, size and weight of a type role
```

- A new component writes no token code: import `sys` and its own component's module.
- Each call returns the value for the current mode, density, accent and text scale, with the
  Rust type of the token's Penpot type (`Color`, `f32`, `TextStyle`, …).
- Names map mechanically: hyphens become underscores (`fg-secondary` → `fg_secondary`), a
  leading digit gets an underscore (`2xs` → `_2xs`), and `type` is written `r#type`.
- A token renamed or removed in Penpot is a compile error at every line that reads it.
- There is no `ref` module, and a test fails if a component imports another component's
  tokens: read `sys.*` and your own prefix only.
- Missing a token? Ask design to add it in Penpot. Don't hard-code a value "for now".
- Layout arithmetic (`/ 2.0`, `* 2.0`) stays in code; style values don't.
### 10.5 PR checklist
 
- [ ] `tokens.json` came from Penpot and was normalized.
- [ ] `cargo test` passes (contract checks in §13).
- [ ] No new literal colours, sizes or gaps in component code.
- [ ] New component tokens meet a rule in §7.
- [ ] `niri-layer-rule.kdl` regenerated if a radius changed.
---
 
## 11. Shell-specific guidance
 
### 11.1 Surfaces over unknown wallpaper
 
- Translucent surfaces must stay readable over pure white **and** pure black wallpaper. CI
  composites each surface over both and checks content contrast (§13).
- Without compositor blur, keep surface alpha at roughly 0.75–0.85 or higher.
- Pair every translucent surface with a `stroke`, so edges read on wallpaper of the same
  tone (Windows does this for every acrylic flyout).
### 11.2 One accent, everywhere
 
Selection, focus, toggles and progress use `sys.color.accent.*`. Status colours never use
the accent, and the accent never means "blue".
 
### 11.3 States
 
Hover, pressed and selected are overlays (§6.1), so one rule covers bar items, menu rows and
Wi-Fi rows, and works on translucent surfaces. Selection in menus uses `accent.bg` +
`accent.fg`. Focus uses `sys.color.focus.ring`.
 
### 11.4 Typography
 
Five roles. Clocks and percentages use `numeric`. Don't add sizes per component; if text
must be smaller in one place, use `caption`.
 
### 11.5 Translucent colours that alias primitives
 
`rgba({ref.color.neutral.100}, 0.06)` keeps translucent neutrals tied to the palette. Penpot
doesn't document references inside `rgba()` (Tokens Studio does), so verify it in a scratch
Penpot file first. Until verified, write literal `rgba()` values in `sys/color/*`.
 
---
 
## 12. Anti-patterns
 
| Anti-pattern | What goes wrong | Instead |
|--------------|-----------------|---------|
| Code reads `ref.color.neutral.11` | ignores light/dark; breaks when the palette changes | read a `sys` role |
| `bar.theme.*` subtree per component | each component re-decides dark mode; they drift apart | `sys/color/*` only |
| Component token renaming a `sys` token | token bloat; two names for one decision | read the `sys` token |
| Hard-coded hex equal to the accent | doesn't follow the user's accent | reference `sys.color.accent.*` |
| Mixing pairs (`popover.fg` on `panel.bg`) | unreadable combinations ("colours from different sets should not be combined" — KDE) | use the pair of the surface you draw on |
| A mode theme without `ref` enabled | every reference in that mode breaks in Penpot | enable `ref`, `sys/base`, `components` in every mode theme |
| `$extensions` or hand edits in the JSON | Penpot drops them on the next import | everything in Penpot |
| `rem`, `em`, `%` sizes | rejected by Penpot's default resolver and by the shell | px or unitless |
| Font weights like 510 | Penpot rejects them | 100–900 in hundreds, 950 |
| Density-scaled values without `round()` | Penpot and shell disagree by sub-pixels | `round(… * {sys.density.factor})` |
| A "dark" token name | mode becomes part of the name | mode is a set |
| Token per hover per component | combinatorial growth | `sys.color.state.*` overlays |
 
---
 
## 13. What CI checks
 
From the spec's tests; failures name the token and the rule.
 
1. The export parses; groups `mode` (`light`, `dark`) and `density` (`comfortable`,
   `compact`) exist; every theme has a description.
2. Every token resolves without error in all four `mode` × `density` combinations.
3. Tier direction: `ref.*` holds literals; `sys.*` references only `ref`/`sys`; component
   tokens reference only `sys`, non-colour `ref`, and `sys.density.factor`.
4. Set contents match §9.1; light and dark define the same `sys.color.*` names.
5. Code reads only `sys.*` and its own prefix; every read token exists with an accepted type.
6. `ref.color.neutral.<L>` and `ref.color.<hue>.<L>` names match their lightness within ±2.
7. Geometry is mode-independent; density changes only density-scaled tokens.
8. Contrast: content on each surface over black and white backdrops meets the floors.
9. No `$extensions`, no unknown types, file normalized.
10. Report only (never fails): tokens no code reads; tier counts against the budget (§3).
---
 
## 14. Glossary
 
- **Token** — a named design value in Penpot.
- **Primitive (`ref`)** — a raw value; says what it is.
- **Semantic (`sys`)** — a decision; says what it's for.
- **Component token** — a value one component owns.
- **Set** — a group of tokens in Penpot; later sets override earlier ones.
- **Theme** — a choice of sets; one theme per group is active.
- **Surface** — an area the shell draws on (panel, popover, osd, notification, raised).
- **State layer** — a translucent overlay showing hover, pressed or selected.
- **Density factor** — the number spacing and control sizes are multiplied by.
- **Seed** — the accent colour everything accent-coloured derives from.
---
 
## Sources
 
- [Material Design 3: state layers](https://m3.material.io/foundations/interaction/states/state-layers); M3 colour roles and dynamic colour
- GitHub Primer design tokens (base → functional → component)
- [Adobe Spectrum: design tokens](https://spectrum.adobe.com/page/design-tokens)
- [Brad Frost: The many faces of themeable design systems](https://bradfrost.com/blog/post/the-many-faces-of-themeable-design-systems/)
- [IBM Carbon: themes](https://carbondesignsystem.com/elements/themes/overview/), [colour tokens](https://carbondesignsystem.com/elements/color/tokens/)
- [libadwaita: styles and appearance](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/styles-and-appearance.html) (surface pairs, accent derivation)
- [KDE KColorScheme](https://api.kde.org/kcolorscheme.html) (colour sets and roles)
- [Microsoft: acrylic material](https://learn.microsoft.com/windows/apps/design/style/acrylic) (surface + stroke, solid fallback)
- [Design Tokens Pills #28: Specific tokens at component level](https://designtokens.substack.com/p/specific-tokens-at-component-level)
- [Smart Interface Design Patterns: naming design tokens](https://smart-interface-design-patterns.com/articles/naming-design-tokens/); Nathan Curtis, *Naming Tokens in Design Systems* (EightShapes)
- [Penpot user guide: design tokens](https://help.penpot.app/user-guide/design-systems/design-tokens/) (sets, themes, disabled sets, math)
- [Tokens Studio: colour tokens](https://docs.tokens.studio/manage-tokens/token-types/color/) (`rgba()` with references)
- Penpot source (2.18.4: `common/src/app/common/types/token.cljc`, `tokens_lib.cljc`,
  `frontend/src/app/main/data/style_dictionary.cljs`)
 
