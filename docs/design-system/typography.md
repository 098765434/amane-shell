# Typography

The shell's text styles are Penpot typography tokens (`sys.type.*` and a few component ones),
read through amane-kit's `Styled`: `Text::new(…).style(sys::r#type::label())`.

## Roles

| Token | Value | For |
|---|---|---|
| `sys.type.caption` | 12 / regular | secondary lines, small controls, a setting's name, tooltips |
| `sys.type.label` | 13 / regular | most text: rows, buttons, menus, fields |
| `sys.type.title` | 13 / medium | headings, focused titles |
| `sys.type.numeric` | 13 / medium | the clock, the timer, percentages |
| `sys.type.icon` | `fontFamilies` only | the icon font stack, for glyphs (`layout::icon`) |

All take `ref.font.family.ui`: Atkinson Hyperlegible Next, then Atkinson Hyperlegible, Inter and
`sans-serif`. The icon stack, `ref.font.family.icon`, is Material Symbols Rounded, then Material
Icons Round and `sans-serif`. The weights are `ref.font.weight.regular` 400, `.medium` 500 and
`.bold` 700; the sizes `ref.font.size.11` to `.14`. There is no `sys.type.body` yet ([penpot-backlog.md](penpot-backlog.md), *Roles to consider*).

## Component typographies

Where a design sets its own type, the component has a typography of its own:

| Token | Value |
|---|---|
| `notification.card.title` | 13 / bold, 16 px lines, letter spacing −0.26 |
| `notification.card.body` | 13 / regular, 16 px lines, letter spacing −0.1 |
| `notification.card.time` | 11 / regular, 14 px lines |
| `dialog.title` | 13 / bold, 16 px lines |
| `dialog.message` | 11 / regular, 14 px lines |
| `dialog.button.label` | 13 / medium, 16 px lines |
| `quick.type.title`, `quick.type.caption` | 14 / medium and 12 / regular, on `quick.type.line` (17 px) lines |

## Font stacks

amane takes one family name and fontconfig always answers with some font, so the kit picks the
first family in a token's stack that is really installed (or a CSS generic name, which
fontconfig maps), else the last entry (`kit/src/tokens/font.rs`). Each stack is looked up once.
`AMANE_DEBUG=1` prints which family each stack chose.

## Text scale

`TextStyle` multiplies the size and the letter spacing by the text scale: `AMANE_TEXT_SCALE`, else
`text_scale` in `appearance.json` (Shell Settings' Text size), kept within 0.5–3. Layouts must let
rows grow with it. Nothing about it is in Penpot.

## What reaches the text

Every member Penpot gives a typography reaches the drawn text: `Text::new(…).style(role)`
(the kit's `Styled`) sets family, size, weight (`"400 italic"` turns on italic), `lineHeights`,
`letterSpacing`, `textCase` and `textDecoration`.

| Member | Penpot value | In amane |
|---|---|---|
| `fontWeights` | 100–950 or a name, optional ` italic` | exact on a variable font's `wght` axis; a family of static files takes the closest file (CSS matching), so an in-between weight snaps when both kinds are installed |
| `lineHeights` | multiplier, `%`, or px (needs `fontSizes` in the same typography) | stored as a multiple of the size; the text's box is then the whole line box, like CSS and Penpot. Left out, lines take the font's own spacing and the box hugs ascent to descent, as before |
| `letterSpacing` | px (`%` is refused, as in Penpot) | pixels after every letter, scaled with the text size setting |
| `textCase` | `none`, `uppercase`, `lowercase`, `capitalize` | applied to the string before it is laid out |
| `textDecoration` | `none`, `underline`, `strike-through` | drawn at the font's own offsets and thickness, in the text's colour |

Text align (`start`/`center`/`end`/`justify`) and direction (`auto`/`ltr`/`rtl`) are Penpot
panel settings, not tokens: set them in code with `.align(TextAlign::…)` and
`.direction(TextDirection::…)`. Vertical alignment and grow type are the parent's job
(`align_child`, `Size::Fixed`/`Parent`). amane shapes text with parley (kerning, ligatures,
fallback for every script, bidi), and a variable font's other axes are set with
`.variation(b"FILL", 1.0)`.

## Rules

- Use a role. Don't add sizes per component; if text must be smaller in one place, use `caption`.
- Clocks and percentages use `numeric`.
- Add a role only when a size is used by two components and matches no role; a design that sets
  its own type (a banner, an alert) gets a component typography instead.
