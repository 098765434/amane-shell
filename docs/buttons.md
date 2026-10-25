# Buttons

The shell's shared controls (`src/components/buttons/`). Each follows its Material 3
counterpart in structure (variants, shapes, a hover state layer, a disabled state), but is drawn
in the shell's macOS/iOS look and sized like its other controls. They read `sys.*` tokens
only, so any component can use them.

| Component | M3 counterpart | Used for |
|---|---|---|
| `PushButton` | [Buttons](https://m3.material.io/components/buttons/overview) | "Connect", notification actions, "Clear All", "Show Less", the bar's clock |
| `IconButton` | [Icon buttons](https://m3.material.io/components/icon-buttons/overview) | mute, Wi-Fi scan, close/clear, Do Not Disturb, the bar's glyph items |
| `Switch` | [Switch](https://m3.material.io/components/switch/overview) | Wi-Fi and Bluetooth power |
| `DrawnButton` | — | the bar's workspace pips: content of its own, drawn in the button's colour |
| `SegmentedButton` | [Segmented buttons](https://m3.material.io/components/segmented-buttons/overview) | the power profile |

## Style and shape

`PushButton`, `IconButton` and `DrawnButton` are one `ButtonBuilder` with a different kind, so
they share every builder method (`style`, `shape`, `color`, `height`, `disabled`, `on_press`,
`on_secondary`, `on_middle`, `on_scroll`, `tooltip`, `highlight`, `active`). They
take a `Style` and a `Shape`:

| `Style` | M3 | Fill | Content by default |
|---|---|---|---|
| `Filled` | filled | `sys.color.accent.bg` | `sys.color.accent.fg` |
| `Tonal` | tonal | `sys.color.surface.raised.bg` | `sys.color.surface.panel.fg` |
| `Plain` | text / standard | none | `sys.color.surface.panel.fg` |

`Shape::Round` is a capsule (a circle when square). `Shape::Square` is the rounded rectangle
chips and rows use (`sys.radius.raised`). A button on another surface passes that surface's
`fg` with `.color(…)`. An `IconButton` that's `.selected(true)` is a toggle that's on, and its
glyph turns `sys.color.accent.bg`.

A `PushButton` is control height and hugs its label plus `sys.space.inset.m` either side,
unless given `.width(…)`. An `IconButton` is a control-height square. Bar items pass
`.height(Parent)`. `.on_press` is the left click, `.on_secondary` the right click and
`.on_middle` the middle click.

`.highlight(bg, fg)` replaces the hover layer: while hovered, or while `.active(true)`, the
button fades to a `bg` fill with its content in `fg`. The status bar passes
`bar.indicator.active.*` for its white tiles; the colours come from the caller, so the buttons
still read `sys.*` only.

## State

amane rebuilds the view every frame, so a button can't hold state of its own. Hover, and the
switch's and segmented button's motion, are kept in tables in the module, keyed by each
control's **id**. Ids must be unique and stable,
for example `amane.qs.wifi.connect` or `amane.notification.{id}.close`. While hovered, the fill
is `sys.color.state.hover` composited over it (`over()`). This is the state-layer rule from
the token manual (§6.1, §11.3), and it also works on `Plain` buttons and translucent
surfaces. Disabled buttons are drawn at 38 % opacity and take no input. Segments show the same
hover layer, and so do clickable [list items](components.md#list).

The `Switch` and `SegmentedButton` reflect a service's state the same way Quick Settings' tiles do. A
click shows the new state at once, and if the service hasn't agreed within 1.5 s the control
goes back to what the service says.

## Switch

An iOS track and knob at `sys.size.switch.{width,height}`. Off, the track is
`sys.color.state.selected`, an overlay, so it reads on any surface. Turning on, it blends to
`sys.color.accent.bg`. The `sys.color.accent.fg` knob, inset by `sys.space.gap.2xs`, springs
across.

## Pending tokens

These are constants until they exist in Penpot (marked `TODO(token)` in the code):

- `sys.opacity.disabled`: 0.38 (`buttons/mod.rs`).
- `sys.motion.*`: spring 450 ms with a 0.15 bounce, fade 200 ms (`motion.rs`).
- `sys.color.state.pressed`. amane reports a press as the start of a drag and the release
  through `on_release`, so a held pressed layer is possible; until the token exists a press
  is shown as a timed pulse (`press_scale`, `sys.motion.press-scale` to be).
