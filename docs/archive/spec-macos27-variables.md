# Spec: adopting the macOS 27 kit's variables

> **Archived: implemented.** Kept as history and not kept up to date. The current references are
> [design-system/materials.md](../design-system/materials.md),
> [design-system/tokens.md](../design-system/tokens.md) and [kit/controls.md](../kit/controls.md#control-sizes).

**Status (2026-10-26): implemented.** All seven parts are in, with these departures:
- Accents: only the system red (`status.critical`), the default seed (blue `#0091ff`) and the
  Settings picker took the new values. The accent is one mode-free seed, so the per-mode and
  vibrant forms and the five colours nothing reads (mint, teal, cyan, indigo, brown) were left
  out.
- `selection.*`, `list.alternate-row` and `scrim` weren't added: the manual has selection follow
  the accent, `state.selected` already is the inactive selection, and nothing draws stripes or a
  scrim yet.
- Control sizes: everything is Medium for now, the shell's panels too, and density scales the
  sizes through `sys.density.factor` (manual rule 5) instead of moving a step. `scope(Large, …)`
  makes a panel Large.
- Part 7 (size-dependent frost) is in, the other way round: niri's `blur {}` here is already
  light (1 pass, offset 1), so it stays the small glass's blur (`glass-small`), and `glass`
  sets a heavier one of its own (2 passes, offset 1.5). Small means no more than 44 across the
  short side, not under 60 tall: a wide capsule button is small glass too.

The source is `variables(1).json`, exported on 2026-10-26 from the Figma community file
"macOS 27 (Community)". The file isn't Apple's own, but its values match Apple's published
system colours and AppKit control sizes. This spec says which of its variables the shell takes
and where each one goes. It also lists two vibrancy bugs that the export exposes.

## What the export holds

| Collection | Modes | Variables | Taken |
|---|---|---|---|
| Colors | Light, Dark | 63 | labels, fills, the vibrant forms of both, accents, window, sidebar, tables and the remaining roles |
| Sizes | Mini, Small, Medium, Large, XL | 65 | the control-size scale |
| Kit | one | 18 | the 8 Liquid Glass values only. The rest (`Component Fill`, `Section Stroke`, `Link` and the like) is scaffolding for the file's own designers |
| Context | Content Area, Over-glass | 1 (a string) | not a value. It's the rule that the vibrant set is for use over glass, which `material::vibrant()` already applies |

The `Mode`, `String` and `Labels/White` variables, and `Grays/Black` and `Grays/White`, carry no
design information and are skipped.

## 1. What the export teaches about vibrancy

### 1.1 Apple's vibrant colours are opaque greys, one blend per appearance

Each vibrant colour in the export is an opaque grey, labelled "Use Plus Lighter | Darker". The
blend that applies depends on the appearance, not on how light the grey is:

- in **dark** mode the grey is drawn with **plus-lighter**: `D + g`. Primary `#f5f5f5` adds
  96%, and tertiary `#404040` adds 25%;
- in **light** mode the grey is drawn with **plus-darker**: `D + g − 1`, which darkens by
  `1 − g`. Primary `#1a1a1a` darkens by 90%, and tertiary `#bfbfbf` darkens by 25%.

Our tokens carry each of these as a translucent colour with the same effect: white at alpha `g`
in dark mode, black at alpha `1 − g` in light mode. Under plus-lighter, white at alpha `g` gives
exactly `g`. **Keep the translucent form.** With vibrancy off, it draws correctly as a normal
colour, and amane's split routes it to the right layer: a white label goes to the lighter
layer, a black one to the darker layer.

Do **not** put the opaque greys into the tokens as they are. `amane/src/graphics/vibrancy.rs`
`grey_layer` picks the layer by lightness (`lowest >= 128`). Dark mode's tertiary `#404040`
would land on the darker layer, and light mode's `#bfbfbf` on the lighter one, so both would
come out inverted.

### 1.2 Bug: niri's plus-darker is a multiply, not Apple's subtraction

`niri/src/render_helpers/vibrant.rs` draws plus-darker as `(DST_COLOR, ONE_MINUS_SRC_ALPHA)`,
a multiply. Apple's formula is `D − a·(1 − c)`, clamped at 0. On a mid-grey backdrop
(`D = 0.5`), the 50% secondary label gives:

- Apple: `0.5 − 0.5 = 0.0`;
- ours: `0.5 × 0.5 = 0.25`.

So light-mode vibrant labels come out too faint over darker parts of the backdrop.

**Fix.** Make plus-darker an exact subtraction, using only blend state, so it still works
under every texture program:

- **niri:** for `PlusDarker`, use `glBlendEquationSeparate(FUNC_REVERSE_SUBTRACT, FUNC_ADD)`
  with `BlendFuncSeparate(ONE, ONE, ZERO, ONE)`, then put `FUNC_ADD` back in `reset_blend`.
  The result is `D − S`.
- **amane:** the darker layer's buffer must hold the amount to darken by, `a·(1 − c)`. In
  `split()`, draw each darker command with its colour inverted (`c → 255 − c`) at the same
  alpha. A black label at alpha 0.5 then becomes white at alpha 0.5, which subtracts 0.5.
- **Protocol:** `resources/niri-vibrancy-v1.xml` and amane's copy change the `plus_darker`
  summary to: "the surface's content is subtracted from what is behind, clamped at 0: a client
  draws the amount each pixel darkens by". The protocol is only used by our own fork, so it
  stays at v1.
- **Tests:**
  - in amane, the split inverts the darker layer's colours and leaves its alpha alone;
  - in niri, a unit test of the blend constants per mode, like the existing
    `modes_from_the_wire`;
  - live: compare the light-mode Quick Settings with vibrancy off and on, using `grim`. A
    secondary label on a mid-grey backdrop should come out close to black, not grey.

## 2. Labels, fills and separator (content area)

These go into `sys.color.<mode>.label.*` and `sys.color.<mode>.fill.*`, two new groups for text
and fills over opaque content: the Settings app's pages, cards, and anything not on glass.
They replace `content.secondary`, a single ad hoc level (dark `#ffffff66`, light `#0000008c`).

| Token | Light | Dark |
|---|---|---|
| `label.primary` | `#000000d9` (85%) | `#ffffffd9` (85%) |
| `label.secondary` | `#00000080` (50%) | `#ffffff8c` (55%) |
| `label.tertiary` | `#00000040` (25%) | `#ffffff40` (25%) |
| `label.quaternary` | `#0000001a` (10%) | `#ffffff1a` (10%) |
| `fill.primary` | `#0000001a` (10%) | `#ffffff1a` |
| `fill.secondary` | `#00000014` (8%) | `#ffffff14` |
| `fill.tertiary` | `#0000000d` (5%) | `#ffffff0d` |
| `separator` | `#0000001a` (10%) | `#ffffff1a` (10%) |

The light separator moves from `#00000014` to `#0000001a`.

Not taken: the quinary and "seximal" labels (5% and 3%) and the quaternary and quinary fills
(3% and 2%). Apple's API has four label levels and three fill levels, and `material.rs` keeps
to them.

**Code:**
- In `kit/src/material.rs`, give `label_in` and `fill_in` a `content` path for when
  `!vibrant()` and the material is opaque (`Window` under Solid glass, `Content`). Both
  functions already take `high_contrast`.
- Replace the uses of `content.secondary` with `label(Level::Secondary)`. Find them with
  `grep -rn content::secondary`.
- Contrast floors (`kit/src/tokens/rules.rs`): quaternary at 10% can't pass a text floor. As
  in Apple's system, it's for decoration such as placeholders and disabled glyphs, so exempt it
  from the floors and assert in debug builds that it isn't used for body text. Primary,
  secondary and tertiary keep their floors.

## 3. Vibrant labels and fills (over glass)

These update `sys.color.<mode>.vibrant.*`. The values are the export's greys converted as
described in 1.1.

| Token | Light now → new | Dark now → new |
|---|---|---|
| `label.primary` | `#000000d9` → `#000000e5` | `#ffffff` → `#fffffff5` |
| `label.secondary` | `#00000080` → `#0000008d` | `#ffffff8c` → `#ffffff8a` |
| `label.tertiary` | `#00000042` → `#00000040` | `#ffffff40` (same) |
| `label.quaternary` | `#0000002e` → `#00000026` | `#ffffff2e` → `#ffffff26` |
| `fill.primary` | `#0000001f` → `#00000026` | `#ffffff24` (same) |
| `fill.secondary` | `#00000014` → `#00000019` | `#ffffff17` → `#ffffff14` |
| `fill.tertiary` | `#0000000d` (same) | `#ffffff0f` → `#ffffff0d` |

Vibrant labels are stronger than content labels (90% against 85% in light mode), because the
blend loses some contrast over a frosted backdrop. Run the contrast floors over every material
tint again after the change. Quaternary on thin and ultra-thin is already banned.

## 4. Accents

The export has twelve system colours. Each has a normal form for content and a vibrant form
for use over glass.

| Name | Light | Dark | Vibrant light | Vibrant dark |
|---|---|---|---|---|
| red | `#ff383c` | `#ff4245` | `#f52f32` | `#ff4747` |
| orange | `#ff8d28` | `#ff9230` | `#f58625` | `#ff9e33` |
| yellow | `#ffcc00` | `#ffd600` | `#f5c200` | `#ffe014` |
| green | `#34c759` | `#30d158` | `#26bf4d` | `#3bdb63` |
| mint | `#00c8b3` | `#00dac3` | `#00bda9` | `#2de0cd` |
| teal | `#00c3d0` | `#00d2e0` | `#00b3bf` | `#2dd7e0` |
| cyan | `#00c0e8` | `#3cd3fe` | `#00abcf` | `#47d8fc` |
| blue | `#0088ff` | `#0091ff` | `#0078f0` | `#0a99ff` |
| indigo | `#6155f5` | `#6d7cff` | `#5c50e6` | `#7163ff` |
| purple | `#cb30e0` | `#db34f2` | `#b72bc9` | `#e647fc` |
| pink | `#ff2d55` | `#ff375f` | `#f5234b` | `#ff4169` |
| brown | `#ac7f5e` | `#b78a66` | `#9e7354` | `#c29672` |

Also `gray`: light `#8e8e93`, dark `#98989f`.

**Tokens:**
- `ref.color.system.<name>.{light,dark,vibrant-light,vibrant-dark}` as primitives.
- `sys.color.<mode>.system.<name>` and `sys.color.<mode>.system-vibrant.<name>` as roles.

**What changes:**
- `ref.color.accent.seed` (`#0a84ff`, the older iOS blue) becomes an alias of
  `system.blue`, per mode.
- `ref.color.red.58` (`#ff453a`) becomes `system.red`. `status.critical.*` keeps its
  structure, and only its values change.
- The accent picker in Settings, if there is one, offers the twelve names.
- `Material::Prominent`'s tint uses the vibrant form, because it is drawn over glass.
- Coloured glyphs on glass, such as the QS tile's "on" icon, use `system-vibrant`. They stay on
  the window layer because the split only takes greys, so they aren't blended.

## 5. Window, sidebar and app-window roles

These go into `sys.color.<mode>.*` and are used by `kit/src/app.rs` (Phase 5 of the materials
plan). They're opaque, so they apply when glass is Solid or high contrast is on, and to an
inactive window.

| Token | Light | Dark | Replaces or is used by |
|---|---|---|---|
| `window.bg` | `#ffffff` | `#1e1e1e` | `window_bg()`, an app window's content |
| `sidebar.bg.active` | `#dae6f2` | `#141517` | `Material::Sidebar` solid and high contrast |
| `sidebar.bg.inactive` | `#edf4fa` | `#131414` | `Material::Sidebar` while unfocused, in place of the computed `INACTIVE` mix when opaque |
| `sidebar.label.inactive` | `#878787` | `#666666` | sidebar rows while unfocused |
| `selection.active` | `#0165e2` | `#0158d2` | the selected list row in a focused window |
| `selection.inactive` | `#00000024` (14%) | `#ffffff2e` (18%) | the selected row in an unfocused window |
| `list.alternate-row` | `#0000000d` | `#ffffff0d` | `kit/src/list.rs` striping, if used |
| `scrollbar` | `#00000080` | `#ffffff8c` | `kit/src/scroll_view.rs`. It's the secondary label, so make it an alias |
| `progress.track-stroke` | `#00000012` (7%) | `#ffffff0a` (4%) | `slider.rs` track outline |
| `tooltip.inner-stroke` | none | `#ffffff26` (15%) | `tooltip.rs` stroke in dark mode |
| `scrim` | `#ababab99` | `#1717179e` | the dimming behind a sheet or alert, if one is added |

The export files the sidebar backgrounds under `Miscellaneous/Tooltips/`, which is a mistake
in the source. They're sidebar colours.

## 6. Control sizes

There are five sizes: **Mini, Small, Medium, Large, XL**. The core values are:

| | Mini | Small | Medium | Large | XL |
|---|---|---|---|---|---|
| height | 16 | 20 | 24 | 28 | 36 |
| radius | 4 | 5 | 6 | 7 | 9 |
| font | 10 | 11 | 13 | 13 | 13 |
| button padding, horizontal | 7 | 10 | 16 | 16 | 16 |
| button radius | 4 | 5 | 6 | capsule | capsule |

Large and XL buttons and disclosures are capsules; the export stores their radius as 1000.

**Per-control values** go into `sys.size.control.<size>.<control>.*`.

- **Switch:**
  - track: 36×16, 44×20, 54×24, 64×28, 80×36;
  - knob: 21×13, 26×16, 32×20, 38×24, 47×30;
  - knob while pressed: 33×20, 40×26, 50×31, 59×37, 73×46. The pill grows past the track,
    which is the Liquid Glass press.
- **Slider, unticked knob:**
  - size: 16×12, 18×14, 20×16, 24×20, 24×20;
  - while pressed: 20×15, 23×18, 25×20, 30×25, 30×25;
  - inset: 2, 1, 0, −2, −2.
- **Slider, ticked knob:** 8×16, 10×18, 20×16, 24×20, 24×20; while pressed: 10×20, 13×23,
  25×20, 30×25, 30×25.
- **Slider track:**
  - inset: 1, 1, 1, 0, 0;
  - tick spacing: 2, 2, 2, 3, 3.
- **Checkbox and radio:**
  - box: 12, 14, 16, 18, 18;
  - checkbox radius: 3.5, 4.5, 5.5, 6.5, 6.5;
  - checkmark: 7.78×7.55 at Mini, 9.31×8.93 at Small and Medium, 11.72×11.3 at Large and XL;
  - radio dot: 4, 4.8, 4.8, 5, 5;
  - radio dash (mixed state): 5.5×1.7, then 6.5×2, 6.5×2, 8×2, 8×2;
  - label spacing: 3, 3, 3, 5, 7.
- **Segmented control:**
  - margins: 6, 8, 10, 12, 14;
  - separator height: 10, 12, 14, 18, 20.
- **Menu row:**
  - height: 19, 22, 24, 24, 24;
  - font: 10, 11, 13, 13, 13;
  - symbol font: 11, 11, 13, 13, 13;
  - symbol width: 6, 10, 12, 12, 12;
  - header insets, top/bottom/left: 3/2/14, 4/3/18, 5/4/20, 5/4/20, 5/4/20.
- **Popup (dropdown):**
  - chevron font: 10, 11, 11, 13, 13;
  - left inset: 7, 10, 12, 14, 18.
- **Search field:**
  - side inset: 6, 6, 8, 8, 10;
  - glyph font/leading: 11/13, then 13/15 at the four larger sizes.

**Not taken:** the combo button, stepper, colour well, dial and arrow button, and the text
cursor height. The kit has none of these controls. Add their values when one is built.

**How the sizes map to the shell.** The kit currently has one size, set by density:
`control.height` = row − 10, which gives 30; the switch is 44×22 and the slider track is 6.
The proposal:

- the Settings app's content uses **Medium**, as macOS apps do;
- Quick Settings, the launcher and notifications use **Large**, with capsule buttons, as
  macOS 26's Control Center does;
- density picks one step up or down from these: compact goes one step smaller.

`kit/src/buttons/{switch,checkbox,radio,segmented,push_button}.rs`, `slider.rs`,
`dropdown.rs` and `search_field.rs` read a `ControlSize` from a thread-local scope, in the
same style as `material::scope`, instead of fixed tokens.

The switch's pressed knob (wider than the track) is a separate visual change. It can follow
in its own commit.

This section changes how the shell looks more than any other. Do it last, and screenshot the
Quick Settings and Settings pages before and after.

## 7. Liquid Glass parameters

These are Figma's glass-effect values, in Figma's units:

| Figma | Value | Ours (`components.material.glass`) | Mapping |
|---|---|---|---|
| Depth (Regular, and Medium and Large) | 30 | `bevel` 30 | matches |
| Refraction | 70 (%) | `refraction` 24 (px) | different units, so no direct mapping. Keep ours: it's tuned by eye in niri |
| Dispersion | 20 (%) | `dispersion` 0.6 | different scale. Note it, and don't change it |
| Light angle | 0° | niri `light-angle`, default 225 (top left) | Figma's 0 is its own default. Keep the top-left light |
| Frost | Regular 6, Medium 16, Large 16 | one global `blur {}` radius | see below |
| Splay | 20 | none | skip |
| Opacity | 25 (%) | the thin tint (`#0000008c` dark) | not a direct match. The materials' tints already set opacity per thickness |

**The one new idea is size-dependent frost.** Small glass, like a button, blurs lightly (6).
Medium and large glass blurs heavily (16). This matches Apple's rule that larger glass is more
opaque. niri's `Material` has no blur-radius field (`niri-config/src/appearance.rs:1118`), so
this needs work in niri:
- add `blur-radius` to `Material` and `MaterialParams`;
- render each shape's backdrop at its material's radius. The blur passes are per-region
  already, so this costs a second blur radius on the frame, not more passes per shape;
- in the tokens, give `glass` the large value and add a `glass-small` material with radius 6,
  for `Control` shapes under about 60 px tall. `Material::niri_name()` picks between them by
  the shape's height.

This is optional and has the highest cost here. Measure frame time against the +2 ms budget
before keeping it.

## Order

Each step builds and passes `nix develop -c cargo test` on its own.

1. **Plus-darker fix** (1.2): niri and amane, plus the protocol text. It's a correctness fix,
   and it's independent of the tokens.
2. **Vibrant values** (3). Run the contrast floors after it.
3. **Content labels and fills** (2), replacing `content.secondary`.
4. **Accents** (4), including the accent seed and `status.critical`.
5. **Window and sidebar roles** (5), used by the app-window materials.
6. **Control sizes** (6), with before-and-after screenshots.
7. Optional: **size-dependent frost** (7), in niri.

Each new token gets a row in `docs/penpot-todo.md`, and `docs/materials.md` gets the
content/over-glass rule and the accent table.
