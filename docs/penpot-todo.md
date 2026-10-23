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
| `sys.motion.enter-scale` | 0.9 | `notifications/center.rs` (a card's entrance) |
| `sys.opacity.disabled` | 0.38 | `buttons/mod.rs` (`DISABLED_OPACITY`) |

`motion` and `opacity` are already `sys` categories in the vocabulary (§4.2). Add `ref.duration.*`
primitives for the durations first.

## Tokens to recreate or delete

- The `notification.*` tokens and the `sys.color.surface.notification.*` pair were added to
  `tokens.json` by hand (see [notifications.md](notifications.md)). Recreate them in Penpot under
  the same names, or the next export drops them.
- `menu.label.advance-ratio` is no longer read: menus measure their text now. Delete it.
- `sys.color.accent.text` and `sys.space.inset.s` aren't read by any code yet (the report in
  `tokens check` lists them). Keep them if something is coming that needs them.

## Roles to consider

- `sys.color.surface.panel.stroke`: the panel surface has no stroke, though its cards are
  translucent. §1.7 says every surface has `bg`, `fg` and `stroke`, and §11.1 pairs every
  translucent surface with a stroke. The panels draw no edge until it exists.
- `sys.shadow.*`: there's no shadow role yet, so the shared `Card` has no M3 elevated variant.
