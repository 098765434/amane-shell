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
| `Slider` | `slider.rs` | [Sliders](https://m3.material.io/components/sliders/overview) | output and microphone volume |

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
`.blur(true)` frosts what's below with `sys.effect.backdrop-blur`, a macOS departure from M3. A
card also records where the pointer is over it (`layout::cursor()`), so a right-click on its
content can anchor a menu.

## TopAppBar

`TopAppBar::new(title)` has M3's slots: `.navigation_icon(glyph)` (here the panel's glyph),
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

`Divider::build()` is a `sys.border.hairline` line in `sys.color.separator`, with `gap.xs` above
and below. `Divider::height()` is what it takes up, for menus that measure themselves.

`Slider::new(value, width)` sets a 0–100 level when pressed, dragged or scrolled. Its thin
`sys.size.slider.height` track is filled with the accent, or greyed with `.dimmed(true)`. The hit
area is a full control tall. One scroll notch moves it `slider::STEP` (5). The bar's volume pill
uses the same `slider::scrolled`.

## Under the components

- `layout.rs` holds the layout primitives: `boxed`, `pad`, `icon` (every Material Symbols
  glyph), `caption`, `split_row`, `clipped_label`, `blank`, `slot`, `text_width` (measured),
  `inner_width` and cursor tracking.
- `icons.rs` holds the glyphs and their pickers (`battery_glyph`, `wifi_glyph`, `audio_glyph` …).
- `color.rs` holds `over`, `faded` and `with_alpha`.
- `motion.rs` holds the shared springs and fades (`spring`, `sheet`, `fade`) and the keyed list
  motion the notifications use.
