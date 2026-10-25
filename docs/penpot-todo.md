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
| `sys.motion.duration.hover`, `.press`, `.close` | 120 ms, 220 ms, 160 ms | `components/motion.rs` (`HOVER`, `PRESS`, `CLOSE`) |
| `sys.motion.duration.pop`, `sys.motion.pop.bounce` | 380 ms, 0.08 | `components/motion.rs` (`POP`: a popover swelling in) |
| `sys.motion.zoom.bounce` | 0.05 | `components/motion.rs` (`ZOOM_BOUNCE`: a page growing out of its control) |
| `sys.motion.enter-scale` | 0.9 | `components/motion.rs` (`ENTER_SCALE`: a card's or control's entrance) |
| `sys.motion.press-scale` | 0.96 | `buttons/mod.rs` (`PRESS_SCALE`; `quick.press.scale` holds the same) |
| `quick.drag.band` | 0.1 | `quick_settings/edit.rs` (`BAND`: the dead band before a held control changes cell) |
| `quick.page.min-rows` | 5 | `quick_settings/mod.rs` (`MIN_PAGE_ROWS`) |
| `quick.open.lift` | 12 | `quick_settings/mod.rs` (`OPEN_LIFT`: how far below its place the card starts) |
| `quick.open.cascade`, `.cascade-scale`, `.cascade-rise` | 90 ms, 0.9, 10 | `quick_settings/tiles/mod.rs` (`CASCADE*`: the modules arriving in a wave) |
| `quick.page.tile-fade`, `quick.page.grid-fade` | 0.25, 0.6 | `quick_settings/mod.rs` (`TILE_FADE`, `GRID_FADE`: shares of the zoom) |
| `quick.page.reveal.*` (header, body, step, span, rise) | 0.35–0.7, 0.45, 0.06, 0.35, 8 | `quick_settings/detail.rs` (`REVEAL_*`: a page's content coming in) |
| `sys.glass.material` | `glass` | `components/glass_region.rs` (`MATERIAL`: the compositor material the glass's frost asks for; shared by Quick Settings and the dock) |
| `sys.size.page-dot` | 6 | `components/page_dots.rs` (`DOT`) |
| `dock.icon.size`, `dock.radius`, `dock.reveal.strip` | 44, 18, 2 | `dock/mod.rs` (`ICON`, `RADIUS`, `STRIP`) |
| `dock.running.size`, `dock.running.max` | 4, 3 | `dock/item.rs` (`RUNNING`, `MOST_RUNNING`) |
| `dock.motion.*` (reveal, hide delay, page, flick, rubber band, resize, item, magnify scales and lifts, hover, tooltip) | see `DOCK` | `dock/motion.rs` (`DOCK`); add `dock` to `tokens::COMPONENTS` with the first `dock.*` token |
| `launcher.grid.cols`, `.rows`, `.cell`, `.icon`, `.highlight` | 5, 4, 150, 64, 84 | `launcher/mod.rs` (`COLS`, `ROWS`, `CELL`, `ICON`, `ICON_BOX`) |
| `launcher.panel.inset`, `.radius`, `launcher.header.height`, `launcher.pills.height` | 15, 22, 40, 40 | `launcher/mod.rs` (`INSET`, `RADIUS`, `HEADER`, `PILLS`) |
| `launcher.result.height`, `.gap`, `.icon` | 56, 6, 40 | `launcher/mod.rs` (`ROW`, `ROW_GAP`), `launcher/results.rs` (`RESULT_ICON`) |
| `launcher.open.scale`, `launcher.scroll.wheel-step` | 0.96, 60 | `launcher/mod.rs` (`OPEN_SCALE`, `WHEEL_STEP`) |
| `launcher.search.size`, `.caret`, `launcher.logo.size` | 18, 2×22, 24 | `launcher/header.rs` (`SEARCH_TEXT`, `CARET`, `LOGO`) |
| `launcher.highlight.radius`, `launcher.scrollbar.width`, `.thumb-min` | 14, 6, 32 | `launcher/grid.rs` (`HIGHLIGHT_RADIUS`, `SCROLLBAR`, `THUMB_MIN`); add `launcher` to `tokens::COMPONENTS` with the first `launcher.*` token |
| `sys.opacity.disabled` (also) | 0.38 | `quick_settings/tiles/mod.rs` (`DIMMED`: a placeholder for a control without its hardware) |
| `sys.opacity.disabled` | 0.38 | `buttons/mod.rs` (`DISABLED_OPACITY`) |
| `sys.color.glass.rim`, `sys.color.glass.glow` | light `#ffffffe6`, `#ffffffbf`; dark `#ffffff4d`, `#ffffff14` | `components/glass.rs` (`LIGHT`, `DARK`) |
| `sys.effect.glass.far`, `sys.effect.glass.bloom` | light 0.5, 0.3; dark 0.2, 0.05 | `components/glass.rs` (`LIGHT`, `DARK`) |
| `sys.effect.glass.rim-width`, `.bloom-width`, `.glow-blur` | 1.5, 14, 5 | `components/glass.rs` |
| `sys.shadow.popover` | `0 8 16 #00000073` | `tokens/layer_rules.rs` (`SHADOW_*`) |
| `sys.motion.duration.tooltip` | 500 ms | `components/tooltip.rs` (`DELAY`) |
| `tooltip.*` (text style, inset) | `sys.type.caption`, `sys.space.inset.s` | `components/tooltip.rs` |
| `bar.scroll.step` | 3 | `status_bar/mod.rs` (`SCROLL_STEP`: lines of touchpad scroll per workspace) |

`motion` and `opacity` are already `sys` categories in the vocabulary (§4.2). Add `ref.duration.*`
primitives for the durations first.

The glass `far` and `bloom` strengths differ by mode, so they go in the `sys/color/*` sets beside
the two colours, though they are numbers. `sys.shadow.popover` is read by the niri rule generator,
whose output is static: it can't follow the mode, so it holds one value for both.

## Tokens to recreate or delete

- `bar.overview.height` is no longer read: the bar keeps its compact height in the overview, where
  the [dock](dock.md) rises instead. Delete it in Penpot.
- `bar.window-item.max-width`, `bar.window-item.radius`, `bar.pill.padding-y` and
  `bar.volume.width` are no longer read: the [status bar](status-bar.md) has no window title or
  pills any more. Delete them in Penpot.
- `bar.indicator.active.bg` (`#ffffff`) and `bar.indicator.active.fg` (`#000000`), the status
  bar's highlight in both modes, were added to `tokens.json` by hand, and `bar.indicator.width`
  was raised by hand from 18 to 28 to give the highlight room. Recreate them in Penpot.
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
  `quick.open.scale` has since been retuned by hand from 0.98 to 0.92, so the card's swell reads.
- `ref.font.family.ui` was reordered by hand to put "Atkinson Hyperlegible Next" first
  (then "Atkinson Hyperlegible", "Inter", `sans-serif`). Make the same change to the stack in
  Penpot, or the next export puts SF Pro Text back.
- `menu.label.advance-ratio` is no longer read: menus measure their text now. Delete it.
- `bar.volume.width` is no longer read: the volume is a glyph button now, its level in a tooltip.
  Delete it.
- The bar's Wi‑Fi, Bluetooth, Sound and power panels are gone (Quick Settings has their pages), so
  `wifi.button.width`, `wifi.icon-button.width`, `sys.size.popover.height` and
  `sys.size.list.max-height` are no longer read. Delete them, and the `wifi` and `power`
  component prefixes with them (`tokens::COMPONENTS`, manual §3).
- `sys.color.accent.text` isn't read by any code yet (the report in
  `tokens check` lists it). Keep it if something is coming that needs it.

## Roles to consider

- `sys.color.surface.panel.stroke`: the panel surface has no stroke, though its cards are
  translucent. §1.7 says every surface has `bg`, `fg` and `stroke`, and §11.1 pairs every
  translucent surface with a stroke. The panels draw no edge until it exists.
- `sys.shadow.*`: there's no shadow role yet, so the shared `Card` has no M3 elevated variant.
  The frosted surfaces' shadow is niri's, from `sys.shadow.popover` above.
