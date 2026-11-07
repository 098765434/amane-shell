# Lists: the Material 3 spec we build to

`amane_kit::lists` draws Material 3's list in three styles: `Shell` (the shell's look),
`M3` (baseline) and `M3Expressive`. This page records where each M3 number comes from.
Every value lives in `crates/kit/src/containers/lists/style.rs`, and that file's unit tests check
them. To compare them on screen, run the showcase:

```sh
nix develop -c cargo run -p amane-kit --example lists      # LISTS_PAGE=0..8 opens a page
```

## Sources

m3.material.io renders on the client, so we couldn't read it directly. The values come from:

- **Baseline:** Material Web tokens v0.192 (`tokens/versions/v0_192/_md-comp-list.scss`,
  `_md-sys-state.scss`), plus the paddings of the legacy (pre-expressive) Compose `ListItem`.
- **Expressive:** Jetpack Compose material3 on androidx-main: `tokens/ListTokens.kt`
  (version 29.0.0), `tokens/ReorderListTokens.kt`, `ListItem.kt` and `ListItemDefaults.kt`.
- **Corner scale:** the standard M3 shape scale (extra-small 4, small 8, medium 12,
  large 16). `ShapeKeyTokens.kt` holds only the token ids.

## Values

| | M3 baseline | M3 Expressive |
|---|---|---|
| Least height, 1 / 2 / 3 lines | 56 / 72 / 88 | 56 / 72 / 88 |
| Leading / trailing space | 16 / 16 | 16 / 16 |
| Vertical padding | 8, 12 on three-line items | 10 (`ItemTopSpace`, `ItemBottomSpace`) |
| Leading → text, text → trailing | 16 / 16 | 12 / 12 (`ItemBetweenSpace`) |
| Between items | 0 | 2 (`SegmentedGap`) |
| Leading / trailing icon | 24 | 20 (`Item*IconExpressiveSize`) |
| Avatar | 40, round, `primaryContainer` / `onPrimaryContainer`, title-medium | same |
| Image | 56 × 56, square corners | 56 × 56, corner small (8) |
| Video, small / large | 100 × 56 / 100 × 69 | 100 × 56 / 114 × 64 |
| Headline | body-large 16/24, `onSurface` | same |
| Supporting text | body-medium 14/20, `onSurfaceVariant` | same |
| Overline, trailing text | label-small 11/16 w500, `onSurfaceVariant` | same |
| Container | `surface`, square | `surface` segments on `surfaceContainer` |
| Corners | none | resting extra-small 4; segment ends large 16; hovered medium 12; focused, pressed, selected and dragged large 16 |
| Selected | trailing icon `primary` | `secondaryContainer`, content `onSecondaryContainer` |
| Dragged | 16% state layer, elevation level 4 | `tertiaryContainer` / `onTertiaryContainer`, level 4 |
| State layer (`onSurface`) | hover 0.08, focus 0.12, pressed 0.12, dragged 0.16 | same |
| Disabled content | 0.38 | 0.38 |
| Focus indicator | `secondary` | `secondary` |
| Divider | 1 px `outlineVariant`: full, inset (from the text), middle (16 / 16) | same |

### Line count

The line count comes from the content, as in Compose:

- **Three lines:** an overline with supporting text, or supporting text on two lines.
- **Two lines:** either an overline or supporting text.
- **One line:** the headline alone.

An item grows past its least height when its content needs more room, for example a large
video or an overline over two supporting lines. On three-line items the leading and trailing
elements align to the top.

### Behaviour

- **Item types:** text (can't be pressed or focused), button, or link (ends with a chevron).
- **Controls:** in the M3 styles, the whole item toggles its checkbox, radio or switch, and
  the control only shows the state. In the shell style, the kit's own control takes its click.
- **Keyboard:** Up and Down move, wrapping as Material Web's lists do. Home and End jump to the
  ends. Enter and Space press. Escape and Tab let go. Disabled items and text items are skipped.
- **Reorder:** drag the handle. A dragged item takes the dragged state, and the other items
  move aside for it.

## What differs from Material

- **Font:** the M3 styles use the shell's font, Atkinson Hyperlegible Next, at M3's sizes.
  Roboto isn't shipped, so text runs a little wider than in Material's images.
- **Focus and pressed opacity:** Material Web v0.192 says 0.12. Compose's ripple uses 0.10
  for both. We follow the tokens.
- **Disabled headline:** Material Web gives the disabled headline 0.30. Compose's
  `ListTokens` 29.0.0 uses 0.38 for all content, and so do we.
- **Ripple:** none. Pressed is a state layer, plus Expressive's shape change.
- **Colours:** the M3 styles use M3's static baseline scheme (seed `#6750A4`), not colours
  derived from the shell's accent.
