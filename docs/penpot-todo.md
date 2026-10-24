# Pending in Penpot

Token work the shell is waiting on. The manual says Penpot is the source of truth (§1.1) and
`tokens.json` is never edited by hand, so these can't be fixed in the repo. Each one is done in the
Penpot file, exported, and brought in (§10.3).

## Tokens to add

The code holds these as named constants, each marked `TODO(token)`, until Penpot has them:

| Token (suggested name) | Value | Where it is now |
|---|---|---|
| `sys.motion.duration.spring`, `sys.motion.spring.bounce` | 450 ms, 0.15 | `components/motion.rs` (`SPRING`) |
| `sys.motion.duration.sheet`, `sys.motion.sheet.bounce` | 500 ms, 0.1 | `components/motion.rs` (`SHEET`) |
| `sys.motion.duration.fade`, `sys.motion.duration.exit` | 200 ms, 300 ms | `components/motion.rs` (`FADE`, `EXIT`) |
| `sys.motion.duration.hover`, `.press`, `.open`, `.close` | 120 ms, 220 ms, 220 ms, 160 ms | `components/motion.rs` (`HOVER`, `PRESS`, `OPEN`, `CLOSE`) |
| `sys.color.glass.module-rim` | the panel's `fg` at 0.14 | `quick_settings/tile.rs` (`RIM_ALPHA`) |
| `sys.motion.enter-scale` | 0.9 | `components/motion.rs` (`ENTER_SCALE`: a card's or control's entrance) |
| `sys.motion.press-scale` | 0.96 | `buttons/mod.rs` (`PRESS_SCALE`; `quick.press.scale` holds the same) |
| `quick.drag.band` | 0.1 | `quick_settings/edit.rs` (`BAND`: the dead band before a held control changes cell) |
| `quick.page.min-rows` | 5 | `quick_settings/mod.rs` (`MIN_PAGE_ROWS`) |
| `quick.page.grid-recede` | 0.25 | `quick_settings/mod.rs` (`GRID_RECEDE`: how far the grid moves as a page slides in) |
| `sys.opacity.disabled` (also) | 0.38 | `quick_settings/tiles/mod.rs` (`DIMMED`: a placeholder for a control without its hardware) |
| `sys.opacity.disabled` | 0.38 | `buttons/mod.rs` (`DISABLED_OPACITY`) |
| `sys.color.glass.rim`, `sys.color.glass.glow` | light `#ffffffe6`, `#ffffffbf`; dark `#ffffff4d`, `#ffffff14` | `components/glass.rs` (`LIGHT`, `DARK`) |
| `sys.effect.glass.far`, `sys.effect.glass.bloom` | light 0.5, 0.3; dark 0.2, 0.05 | `components/glass.rs` (`LIGHT`, `DARK`) |
| `sys.effect.glass.rim-width`, `.bloom-width`, `.glow-blur` | 1.5, 14, 5 | `components/glass.rs` |
| `sys.shadow.popover` | `0 8 16 #00000073` | `tokens/layer_rules.rs` (`SHADOW_*`) |
| `sys.motion.duration.tooltip` | 500 ms | `components/tooltip.rs` (`DELAY`) |
| `tooltip.*` (text style, inset) | `sys.type.caption`, `sys.space.inset.s` | `components/tooltip.rs` |

`motion` and `opacity` are already `sys` categories in the vocabulary (§4.2). Add `ref.duration.*`
primitives for the durations first.

The glass `far` and `bloom` strengths differ by mode, so they go in the `sys/color/*` sets beside
the two colours, though they are numbers. `sys.shadow.popover` is read by the niri rule generator,
whose output is static: it can't follow the mode, so it holds one value for both.

## Tokens to recreate or delete

- The `notification.*` tokens and the `sys.color.surface.notification.*` pair were added to
  `tokens.json` by hand (see [notifications.md](notifications.md)). Recreate them in Penpot under
  the same names, or the next export drops them.
- The `quick.*` tokens (`quick.grid.cell`, `.cols`, `.rows`, `.gap`, `.margin`, `quick.tile.pad`,
  `.gap`, `.disc`, `quick.card.radius`, `quick.media.art`, `.art-radius`, `.play`,
  `quick.slider.track`, `quick.shadow`, `quick.type.title`, `.caption`, `.line`,
  `quick.page.recede`, `quick.open.scale`, `quick.press.scale`, `quick.drag.scale`,
  `quick.tile.glyph`, `quick.button.glyph`) and
  `ref.font.size.14` were added to `tokens.json` by hand too (see
  [quick-settings.md](quick-settings.md)). Recreate them in Penpot under the same names.
- `ref.font.family.ui` was reordered by hand to put "Atkinson Hyperlegible Next" first
  (then "Atkinson Hyperlegible", "Inter", `sans-serif`). Make the same change to the stack in
  Penpot, or the next export puts SF Pro Text back.
- `menu.label.advance-ratio` is no longer read: menus measure their text now. Delete it.
- `bar.volume.width` is no longer read: the volume is a glyph button now, its level in a tooltip.
  Delete it.
- `sys.color.accent.text` isn't read by any code yet (the report in
  `tokens check` lists it). Keep it if something is coming that needs it.

## Roles to consider

- `sys.color.surface.panel.stroke`: the panel surface has no stroke, though its cards are
  translucent. §1.7 says every surface has `bg`, `fg` and `stroke`, and §11.1 pairs every
  translucent surface with a stroke. The panels draw no edge until it exists.
- `sys.shadow.*`: there's no shadow role yet, so the shared `Card` has no M3 elevated variant.
  The frosted surfaces' shadow is niri's, from `sys.shadow.popover` above.
