# Shared components

The building blocks the panels, the notification center and the context menu are made of. Like the
[buttons](buttons.md), each is named and structured after its Material 3 counterpart and drawn in
the shell's macOS look. They read `sys.*` tokens only, so any component can use them.

| Component | File | M3 counterpart | Used for |
|---|---|---|---|
| `Card` | `card.rs` | [Cards](https://m3.material.io/components/cards/overview) | every panel, the notification center's sheet, context menus |
| `TopAppBar` | `app_bar.rs` | [Top app bar](https://m3.material.io/components/app-bars/overview) (small) | the title row of each panel and of the center |
| `List`, `ListItem` | `list.rs` | [Lists](https://m3.material.io/components/lists/overview) | networks, Bluetooth devices, audio devices, peripherals |
| `Divider` | `divider.rs` | [Divider](https://m3.material.io/components/divider/overview) | context-menu separators |
| `Slider` | `slider.rs` | [Sliders](https://m3.material.io/components/sliders/overview) | output and microphone volume, Quick Settings' sliders |
| `tooltip` | `tooltip.rs` | [Tooltips](https://m3.material.io/components/tooltips/overview) (plain) | the bar's volume level and battery charge |

## Card

`Card::new(content)` fills its parent and stacks `content` `.gap(…)` apart inside a
`sys.space.inset.l` inset (`.padding(…)` changes it). It's drawn on a `Surface`, which gives one
complete `bg` / `fg` / `stroke` set (manual §1.7). Whatever sits on the card takes its colours from
the same surface:

| `Surface` | Tokens | Drawn on it |
|---|---|---|
| `Panel` | `sys.color.surface.panel.*` | the bar's panels (no `stroke` role yet) |
| `Popover` | `sys.color.surface.popover.*` | menus, the notification center |
| `Notification` | `sys.color.surface.notification.*` | toasts |

`Variant::Filled` is the fill alone, and `Variant::Outlined` adds a `sys.border.hairline` in the
surface's stroke. M3's elevated card is left out because there's no `sys.shadow` role yet.
`.blur(true)` frosts what's below with `sys.effect.backdrop-blur`, a macOS departure from M3; the
bar's panels leave it off, since niri frosts their whole surface through a layer rule and under a
root card there's nothing of the window's own to blur. A
card also records where the pointer is over it (`layout::cursor()`), so a right-click on its
content can anchor a menu.

## TopAppBar

`TopAppBar::new(title)` has M3's slots: `.back(id, action)` (a back chevron, for a page drilled
into, like Quick Settings'), `.navigation_icon(glyph)` (here the panel's glyph),
`.subtitle(…)`, and `.actions(…)` at the trailing end. It uses its surface's `fg` and secondary,
`Surface::Panel` by default, and `.color(…)` overrides the title (the power panel turns it
critical on a low battery). Unlike M3's, it has no fill and is no taller than its content.

## List

`ListItem::new(id, headline)` is a one-line M3 list item. It has a `.leading(glyph)`, the
headline (elided), and `.trailing(…)` items such as `list::check()`, a lock glyph, or
`list::trailing_text(…)`. A `.selected(true)` item is filled `sys.color.state.selected` and its
headline is full-strength. The rest read `sys.color.content.secondary`, and `.color(…)`
overrides that. With `.on_press` or `.on_secondary` it's clickable and shows the
`sys.color.state.hover` layer, keyed by `id` like a button's. Supporting and overline text are
the next M3 slots to add when a list needs them.

`List::new(items)` stacks items `sys.space.gap.xs` apart, under an optional `.header(…)`
caption, with an `.empty(…)` note while there are none.

## Divider and Slider

The thin `Slider` takes `fill` and `thickness` in place of the accent and `sys.size.slider.height`:
Quick Settings' glass cards fill it with the panel's `fg`, as macOS does, a documented departure
from the one-accent rule (§11.2).


`Divider::build()` is a `sys.border.hairline` line in `sys.color.separator`, with `gap.xs` above
and below. `Divider::height()` is what it takes up, for menus that measure themselves.

`Slider::new(value, width)` sets a 0–100 level when pressed, dragged or scrolled. Its thin
`sys.size.slider.height` track is filled with the accent, or greyed with `.dimmed(true)`. The hit
area is a full control tall. One scroll notch moves it `slider::STEP` (5). The bar's volume button
uses the same `slider::scrolled`. `.on_value_change` takes any closure.

`.large(glyph)` is the macOS Control Center slider: a track a full control tall with the glyph
inside it, at the start of a fill that's never narrower than the glyph's circle, so the glyph is
the knob. `.on_icon_press(id, action)` makes the glyph a button (Quick Settings' mute).

Quick Settings' own parts (the tile, the grid, the page) are in
[quick-settings.md](quick-settings.md).

## Tooltip

A plain tooltip: one `sys.type.caption` line on a frosted, glass-edged `Card` on `Surface::Panel`,
like the bar's panels, with a `sys.space.inset.s` inset. niri blurs behind it from the bar's
`popups {}` layer rule, which has the same `sys.radius.popover` corners. It shows after the pointer rests on its
control for half a second. It's an xdg-popup with no grab, so it never takes input or focus. It
hangs under the control's surface and is centred above the pointer, as macOS's are, sliding inward
at a screen edge.

Buttons take one with `.tooltip(parent_ns, text)`, where `parent_ns` is the namespace of the
surface they're on (the bar's is `BAR_NS`). Moving off the button hides it. Clicking also hides it
until the pointer leaves. Any other control can use it by calling `tooltip::offer(id, parent_ns,
text)` every frame it's built while hovered, `tooltip::suppress(id)` on click and
`tooltip::dismiss(id)` on leave. Only one shows at a time, in the one popup slot `tooltip::popup`,
which `main.rs` registers. amane never resizes or moves an open popup, so when the text's size or
the control changes, the tooltip closes for one frame and reopens.

## Under the components

- `layout.rs` holds the layout primitives: `boxed`, `pad`, `icon` (every Material Symbols
  glyph), `caption`, `split_row`, `clipped_label`, `blank`, `slot`, `text_width` (measured),
  `inner_width` and cursor tracking.
- `icons.rs` holds the glyphs and their pickers (`battery_glyph`, `wifi_glyph`, `audio_glyph` …).
- `color.rs` holds `over`, `faded` and `with_alpha`.
- `motion.rs` holds the shared springs and fades (`spring`, `sheet`, `fade`) and the keyed list
  motion the notifications use.
