# Controls

amane-kit's buttons and controls (`crates/kit/src/controls/buttons/`, `amane_kit::buttons`). Each follows its Material 3
counterpart in structure (variants, shapes, a hover state layer, a disabled state), but is drawn
in the shell's glass look and sized like its other controls. They read `sys.*` tokens
only, so any component can use them.

| Component | M3 counterpart | Used for |
|---|---|---|
| `PushButton` | [Buttons](https://m3.material.io/components/buttons/overview) | "Connect", notification actions, "Clear All", "Show Less", the bar's clock |
| `IconButton` | [Icon buttons](https://m3.material.io/components/icon-buttons/overview) | mute, Wi-Fi scan, close/clear, Do Not Disturb, the bar's glyph items |
| `Switch` | [Switch](https://m3.material.io/components/switch/overview) | Wi-Fi and Bluetooth power |
| `DrawnButton` | — | the bar's workspace shape: content of its own, drawn in the button's color |
| `SegmentedButton` | [Segmented buttons](https://m3.material.io/components/segmented-buttons/overview) | the power profile, the keyboard light's level, Keep Awake's limit |
| `Checkbox` | [Checkbox](https://m3.material.io/components/checkbox/overview) | choices confirmed together, like a dialog's; nothing in the shell yet |
| `Radio` | [Radio button](https://m3.material.io/components/radio-button/overview) | Shell Settings' choices: appearance, density, glass, motion, widget style, the apps' layout |

## Style and shape

`PushButton`, `IconButton` and `DrawnButton` are one `ButtonBuilder` with a different kind, so
they share every builder method (`style`, `shape`, `color`, `height`, `disabled`, `on_press`,
`on_secondary`, `on_middle`, `on_scroll`, `tooltip`, `highlight`, `active`). They
take a `Style` and a `Shape`:

| `Style` | M3 | Fill | Content by default |
|---|---|---|---|
| `Filled` | filled | `sys.color.accent.bg` | `sys.color.accent.fg` |
| `Tonal` | tonal | `sys.color.vibrant.fill.secondary` | `sys.color.vibrant.label.primary` |
| `Plain` | text / standard | none | `sys.color.vibrant.label.primary` |
| `Destructive` | — | `sys.color.status.critical.hover` | `sys.color.status.critical.text` (an alert's "Don't Save" or "Log Out") |

`Shape::Round` is a capsule (a circle when square). `Shape::Square` is the rounded rectangle
of the current control size (`sys.radius.control.<size>`, 6 at Medium), and a capsule from Large
up ([control sizes](#control-sizes)). A button on another surface passes that surface's
`fg` with `.color(…)`. An `IconButton` that's `.selected(true)` is a toggle that's on, and its
glyph turns `sys.color.accent.bg`.

A `PushButton` is control height and hugs its label plus `sys.space.inset.m` either side,
unless given `.width(…)`. An `IconButton` is a control-height square. Bar items pass
`.height(Parent)`. `.on_press` is the left click, `.on_secondary` the right click and
`.on_middle` the middle click.

`.highlight(bg, fg)` replaces the hover layer: while hovered, or while `.active(true)`, the
button fades to a `bg` fill with its content in `fg`. The status bar passes
`bar.indicator.active.*` for its white tiles; the colors come from the caller, so the buttons
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

A track and pill-shaped knob at the current control size (`control_size::current().switch()`:
54×24 with a 32×20 knob at Medium). Off, the track is `sys.color.state.selected`, an overlay, so
it reads on any surface. Turning on, it blends to `sys.color.accent.bg`. The
`sys.color.accent.fg` knob, centred in the track's height, springs across.

## Checkbox and Radio

`Checkbox::new(id, checked)` and `Radio::new(id, selected)` take an optional `.label(…)` that takes
the click too, `.disabled(…)`, and `.on_toggle(|checked| …)` or `.on_select(|| …)`. They're drawn
in the form controls' look ([`style`](../../crates/kit/src/look/style.rs)): a 16 px mark (times the
density) with the label 3 px after it in the control font (13 medium), the whole control as wide
as its mark and label and `style::control_height()` (24) tall.

- **Checkbox**: off, a box (radius 5.5) of `fieldFill` in a 1 px `fieldBorder`; on, the accent
  around a drawn checkmark (`symbol`). The fill crossfades over 300 ms. A setting that
  applies at once reads better as a `Switch`; a checkbox suits choices confirmed together.
- **Radio**: a group is radios sharing what's chosen, each `selected` when the setting equals
  its value. Chosen, the circle fills with the accent around a 4.8 px white dot; choosing the
  chosen one does nothing. Shell Settings puts a group under its row's title
  (`FormRow::detail`).

## Dropdown

`dropdown::Dropdown::new(id, options, selected)` is one choice out of a list, drawn as a
pop-up button: a `controlFill` button, radius 6, with the chosen option
and a drawn chevron, as wide as the widest option unless given `.width(…)`.
`Dropdown::items(id, Vec<MenuItem>, Option<usize>)` takes options with glyphs (`MenuItem::new(text)
.icon(glyph).trailing(glyph)`) and none chosen, which shows `.fallback(text)`.

A click opens the menu lined up under the button (right edges together, 4 px below, flipped above
when there's no room): an xdg-popup of a `menuFill` panel, radius 8, with 24 px rows, the chosen
one filled with the accent. It fades in with a 4 px slide. The pointer or Up, Down and Enter pick
a row (`.on_select(|index| …)`); Escape or a click outside closes it. The button finds where it is
on its window with `on_track`. In a normal window the menu finds its parent window itself; on a
layer window, pass the surface's namespace with `.layer(ns)`. For a few options that all fit,
radios read better.

`menu::context_menu(id, items, |index| …)` opens the same menu at the pointer, its top-left
corner there (flipped or slid to stay on screen), over the current normal window. Call it from a
right click (`on_click` with `Button::Right`). A second right click while it's open closes it.

## Control sizes

`control_size` has the design kit's five sizes, Mini, Small, Medium, Large and XL. The switch,
checkbox, radio, thin slider's knob and push buttons' corners read the innermost
`control_size::scope`, Medium outside any. From Large up, `Shape::Square` buttons are capsules.
Layout heights (rows, a thin slider's hit area) stay on `sys.size.control.height`, so a size
changes the control drawn, not the room panels leave for it. The thin slider has a pill knob at its
value; an inert slider, a meter, has none.

## The form controls

The kit's `Radio`, `Checkbox`, `Dropdown`, `field_box`, `ListItem::leading_tile` and the
sidebar's tiles draw in one look, ported from an older Quickshell set: its colors, sizes and
fonts are [`style`](../../crates/kit/src/look/style.rs), and a few controls are their own types.
`cargo run -p amane-kit --example controls` shows every kit control (app_id `amane-kit-controls`,
so it runs beside the bar; `CONTROLS_PAGE=<n>` opens on a page).

- `stepper::Stepper` (a number field with up and down halves; hold to repeat, reported on
  release; the wheel steps), used for Shell Settings' whole-number settings.
- `nav_buttons::NavButtons`, the back/forward pill, before Shell Settings' title in a sub-view.
- `leading_icon::LeadingIcon`, a 20 px (28 large) tile of an `Accent` (or any color) with a
  white glyph.
- `pop_up_button::PopUpButton`, an 18 px circle that opens a menu.
- `style` maps the original's colors to `sys.*` tokens where the dark values match (labels,
  `fill.secondary`, separator, accent) and keeps the rest as literals with a light twin, each
  marked `TODO(token)`. Sizes are the original's Figma pixels times `sys.density.factor`.
  `color::Accent` holds the 13 system accents the tiles take.
- `symbol` draws the symbols the controls use (chevrons, checkmark, ellipsis, info) as
  round-joined strokes, which stay on the quad pipeline.
- Left out: the Material press ripple and the menu's scale-in (a fade and a 4 px slide instead).

## Pending tokens

These are constants until they exist in Penpot (marked `TODO(token)` in the code):

- `sys.opacity.disabled`: 0.38 (`buttons/mod.rs`).
- `sys.motion.*`: spring 450 ms with a 0.15 bounce, fade 200 ms (`motion.rs`).
- `sys.color.state.pressed`. amane reports a press as the start of a drag and the release
  through `on_release`, so a held pressed layer is possible; until the token exists a press
  is shown as a timed pulse (`press_scale`, `sys.motion.press-scale` to be).
