# Pending in Penpot

Token work the shell is waiting on. The manual says Penpot is the source of truth (§1.1) and
`tokens.json` is never edited by hand, so these can't be fixed in the repo. Each one is done in the
Penpot file, exported, and brought in (§10.3).

As of 2026-11-09, Penpot and `tokens.json` hold the same tokens with the same values.

## Tokens to add

The code holds these as named constants, each marked `TODO(token)`, until they can be tokens:

| Token (suggested name) | Value | Where it is now |
|---|---|---|
| `sys.effect.glass.far`, `sys.effect.glass.bloom` | light 0.3, 0.08; dark 0.1, 0.01 | `crates/kit/src/look/glass.rs` (`Light::now`). These differ by mode but aren't colours, so §1.4 (enforced by `tokens check`) needs them folded into the rim colour or made mode-free first |
| `sys.glass.material` | `glass` | `crates/kit/src/look/glass_region.rs` (`MATERIAL`: niri's material that glass falls back on). It's a name, and Penpot has no string token type |
| `sys.glass.adapt.*` | 0.65, 0.35 | `crates/kit/src/look/glass_region.rs` (`TURN_LIGHT`, `TURN_DARK`: where small glass turns light or dark over its backdrop) |

Durations are numbers of milliseconds (Penpot has no duration type), read through
`emaki_kit::tokens::ms`. The shared steps are `ref.duration.*`.

## Tokens the code doesn't read yet

These were made in Penpot (2026-11-09) while drawing, from values the code still holds raw. Each
replaces a constant marked `TODO(token)`, `px()` value or plain constant. Until the code reads them,
`tokens::contract::every_token_is_read_or_listed` fails:

- `launcher.*` (but `launcher.open.scale`, `.mode-button.slide` and `.paging.*`, which are read):
  `apps/shell/src/surfaces/launcher/` (`shape.rs`, `mod.rs`, `header.rs`, `grid.rs`, `results.rs`).
- `form.*`, `settings.*`, the Settings colours (`sys.color.control.fill`, `.field.border`,
  `.surface.group.*`, `.separator-soft`, `.shadow`, `.sidebar.edge`, `.tile.*`, `.swatch.*`),
  `sys.radius.window` and the type roles `sys.type.subtitle`, `.group-header` and `.page-title`:
  `crates/kit/src/look/style.rs`, `containers/form.rs`, the controls and `apps/settings`.
- `calendar.*`: `crates/kit/src/controls/calendar.rs`.
- `messages.*`: the Messages app.
- `dock.icon.size` (the settings' default icon size), `dock.rule.clear` and `dock.section.inset`.

`components_read_only_sys_and_their_own_tokens` also fails: `platform/settings/status_bar.rs`
reads `bar.*`.

## Roles to consider

- `sys.color.surface.panel.stroke`: the panel surface has no stroke, though its cards are
  translucent. §1.7 says every surface has `bg`, `fg` and `stroke`, and §11.1 pairs every
  translucent surface with a stroke. The panels draw no edge until it exists.
- `sys.color.state.pressed`: the manual's state rule (§6.1) gives it the surface's `fg` at 10 %.
  Nothing draws a pressed fill today; buttons shrink instead (`sys.motion.press-scale`).
- `sys.color.focus.ring`: a shared keyboard focus outline. Only the launcher draws one, from its
  own tokens (`launcher.focus-ring.*`).
- `sys.color.scrim`: dimming behind a modal surface. Nothing dims behind the alert today.
- `sys.type.body`: the vocabulary has a `body` type role (§4.2), but no text uses one yet;
  [typography.md](typography.md) lists the roles there are.
- `sys.color.accent.text` isn't read by any code yet. Keep it if something is coming that needs
  it.
