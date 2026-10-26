# Shared components

The building blocks Quick Settings, the notification center, the context menu and the dock are made of. Like the
[buttons](controls.md), each is named and structured after its Material 3 counterpart and drawn in
the shell's macOS look. They read `sys.*` tokens only, so any component can use them.

| Component | File | M3 counterpart | Used for |
|---|---|---|---|
| `Card` | `card.rs` | [Cards](https://m3.material.io/components/cards/overview) | the notification center's sheet, context menus, tooltips |
| `TopAppBar` | `app_bar.rs` | [Top app bar](https://m3.material.io/components/app-bars/overview) (small) | the title row of each Quick Settings page and of the center |
| `List`, `ListItem` | `list.rs` | [Lists](https://m3.material.io/components/lists/overview) | networks, Bluetooth devices, audio devices, peripherals |
| `Divider` | `divider.rs` | [Divider](https://m3.material.io/components/divider/overview) | context-menu separators, the dock's rules between workspaces (`Divider::vertical`) |
| `Slider` | `slider.rs` | [Sliders](https://m3.material.io/components/sliders/overview) | output and microphone volume, Quick Settings' sliders |
| `tooltip` | `tooltip.rs` | [Tooltips](https://m3.material.io/components/tooltips/overview) (plain) | the bar's volume level and battery charge |
| `app_icon` | `app_icon.rs` | none | the dock's icons, notification cards' app icons, the launcher's apps |
| `EdgeReveal` | `edge_reveal.rs` | none | the dock hiding at the bottom edge |
| `ScrollView` | `scroll_view.rs` | none | the launcher's grid, results and pill bar, the Customize window's panes |
| `SearchField` | `search_field.rs` | [Search](https://m3.material.io/components/search/overview) (bar) | the Customize window's "Search Controls" |
| `Flow` | `flow.rs` | none | the Customize window's controls, flowing into rows |
| `text_edit::Field` | `text_edit.rs` | none | the launcher's and the Customize window's search text |
| `Kinetic`, `AxisLock` | `kinetic.rs` | none (iOS's scroll physics) | the launcher's scrolling |
| `Pager` | `pager.rs` | none | the launcher's categories |
| `workspace_menu` | `workspace_menu.rs` | none | the dock's "Move to Workspace" and "Open in Workspace", the launcher's "Open in Workspace" |

## Card

`Card::new(content)` fills its parent and stacks `content` `.gap(…)` apart inside a
`sys.space.inset.l` inset (`.padding(…)` changes it). It's made of a `Material`
(`.material(…)`, `Material::Popover` by default; see [materials.md](../design-system/materials.md)), whose tint fills
it. What sits on it takes the vibrant labels and fills (`material::label`, `fill_role`).

`Variant::Filled` is the fill alone, and `Variant::Outlined` adds a `sys.border.hairline` in the
material's separator. M3's elevated card is left out because there's no `sys.shadow` role yet.
`.blur(true)` frosts what's below with `sys.effect.backdrop-blur`, a macOS departure from M3. It
blurs only what the window has drawn under the card, so a card that is its surface's root (the
panels, tooltips, dropdowns) leaves it off: niri frosts those through a layer rule. `.radius(..)`
sets the corners, `sys.radius.popover` by default. A
card also records where the pointer is over it (`layout::cursor()`), so a right-click on its
content can anchor a menu.

## TopAppBar

`TopAppBar::new(title)` has M3's slots: `.back(id, action)` (a back chevron, for a page drilled
into, like Quick Settings'), `.subtitle(…)`, and `.actions(…)` at the trailing end. It uses its
primary and secondary vibrant labels. Unlike M3's, it has no fill and is no
taller than its content.

## List

`ListItem::new(id, headline)` is a one-line M3 list item. It has a `.leading(glyph)` (or
`.leading_tile(glyph)`, the glyph on a small accent tile, as a sidebar names its sections), the
headline (elided), and `.trailing(…)` items such as `list::check()`, a lock glyph, or
`list::trailing_text(…)`. A `.selected(true)` item is filled `sys.color.state.selected` and its
headline is full-strength. The rest read the secondary label, and `.color(…)`
overrides that. With `.on_press` or `.on_secondary` it's clickable and shows the
`sys.color.state.hover` layer, keyed by `id` like a button's. `.bare(true)` leaves out the
secondary fill, for rows on a surface that reads as one piece (the Customize window's
sidebar): only the selected and hover fills show. Supporting and overline text are the next M3
slots to add when a list needs them.

`List::new(items)` stacks items `sys.space.gap.xs` apart, under an optional `.header(…)`
caption, with an `.empty(…)` note while there are none.

## Divider and Slider

The thin `Slider` takes `fill` and `thickness` in place of the accent and `sys.size.slider.height`:
Quick Settings' glass cards fill it with the panel's `fg`, as macOS does, a documented departure
from the one-accent rule (§11.2).


`Divider::build()` is a `sys.border.hairline` line in `sys.color.separator`, with `gap.xs` above
and below. `Divider::height()` is what it takes up, for menus that measure themselves.

`Slider::new(value, width)` sets a 0–100 level when pressed, dragged or scrolled. Its thin
`sys.size.slider.height` track, outlined in `sys.color.progress.track-stroke`, is filled with the
accent, or greyed with `.dimmed(true)`. A pill knob at the current control size
(`control_size::current().slider_knob()`, 20×16 at Medium) sits at the level; an `.inert(true)`
slider is a meter and has none. The hit area is a full control tall. One scroll notch moves it `slider::STEP` (5). The bar's volume button
uses the same `slider::scrolled`. `.on_value_change` takes any closure.

`.large(glyph)` is the macOS Control Center slider: a track a full control tall with the glyph
inside it, at the start of a fill that's never narrower than the glyph's circle, so the glyph is
the knob. `.on_icon_press(id, action)` makes the glyph a button (Quick Settings' mute).

Quick Settings' own parts (the tile, the grid, the page) are in
[quick-settings.md](../shell/quick-settings.md).

## Tooltip

A plain tooltip: one `sys.type.caption` line on a frosted, glass-edged `Card` of `Material::ToolTip`, with a `sys.space.inset.s` inset. niri blurs behind it from the bar's
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

## app_icon

`app_icon::tile(app_id, title, side)` is an app's icon `side` big: the icon its desktop entry
names, else a theme icon named after the app_id or its last dotted part, else a first-letter
placeholder on a raised tile (also shown while the image decodes). `icon_for(app_id)` and
`name_for(app_id)` give the icon file and the name an app goes by.

`app_icon::entry_tile(app, side)` draws an installed app from its `DesktopApp` instead of a window's
app_id: the entry's icon, else the first letter of its name on the same placeholder. The launcher
draws its grid and results with it.

## EdgeReveal

A surface that hides at a screen edge until the pointer touches it, holding no view of its own.
`EdgeReveal::new(motion, delay)` starts hidden. The view calls `hover(bool)` as the pointer comes
onto the surface and leaves it, reads `progress(hold)` once a frame (0 hidden … 1 out, at least
`hold`, which lets something else bring it out, like niri's overview), and sets the surface's
input region from `input_region(width, height, strip, progress, shown)`: only the `strip` along
the bottom edge while hidden, and `shown` too while any of it is out. Once the pointer has been
gone for `delay`, it goes back in.

`keep_out(bool)` holds it out whatever the pointer does: the dock calls it every frame with
whether its menu is open, an icon is held, or files are held over it. Once let go, it goes back in
`delay` later unless the pointer is on it.

## ScrollView

`ScrollView::new(axis, width, height)` is a clipped viewport onto content longer than it.
`.content(length)` says how long the content runs along `axis`, `.offset(px)` how far it's
scrolled (past the ends too, for a rubber band), `.on_scroll(…)` takes the scrolls over it, and
`.scrollbar(opacity)` draws a thin thumb while the content overflows (vertical only). `build(child)`
gives the viewport. It scrolls by a negative padding inside the clip, so the content's hover and
click areas move with it. Where it's scrolled to is the caller's: usually a `Kinetic`.

## SearchField and text_edit

`SearchField::new(id, &field, focused)` is a `surface.raised` capsule a control tall: a magnifier,
the query (or a `.placeholder(…)`, "Search" by default) in `sys.type.label` with its caret (accent,
while `focused`) and selection (a `state.selected` wash), and a clear (×) button while there's a
query and an `.on_clear(…)`. `.width(…)` sets its width. It draws the text itself rather than
focusing a `TextInput`, so every key reaches the surface's `on_key`.

The text is a `text_edit::Field`: the characters, the caret and the selection's other end, with
`insert`, `backspace`, `delete_selection`, `left`/`right`/`move_to` (extending the selection with
Shift) and `select_all`, and `handle(key, modifiers)`, which applies the keys a one-line field
takes — characters, Space, Backspace, Left/Right/Home/End with or without Shift, Ctrl+A/C/X/V
through wl-clipboard (`text_edit::copy`, `paste`) — and says whether the key was the field's.
Line breaks never go in. It's pure and tested; the launcher types into one with its own key
routing, the Customize window through `handle`.

## Flow

`Flow::new(width, gap_x, gap_y)` lays fixed-size `.item(width, height, widget)`s out in rows:
each row takes items until the next wouldn't fit (an item wider than the row has a row to
itself), the items at the top of their row, the rows as tall as their tallest. `build()` gives
the widget, how tall it is and where each item went (`Placed`), so a caller can record where its
items are drawn. The row breaking is `flow::place`, pure and tested.

## Kinetic and AxisLock

`Kinetic` is one axis of scrolling from 0 to `set_max(max)`, in pixels:

- `follow(moved, stop, now)` follows fingers one to one, pulling on a rubber band past the ends.
- When the fingers lift, it coasts as far as their speed carries it (an iOS-like projection eased
  out over a short coast) or springs back from past an end. Fingers that rested before lifting
  leave no momentum.
- `scroll(&scroll, axis, wheel_step)` does either from an amane `Scroll`: fingers are followed, a
  wheel eases on a notch at a time.
- `go(to)` eases there (bringing something into view), `jump(to)` puts it there at once.
- `value()` is where it is this frame; call `settle_stale()` once a frame for a compositor that
  never says the fingers lifted.

`Tracker` is the smoothed finger velocity underneath, shared with `Pager`. `AxisLock` lets one
two-finger gesture either scroll or turn the page: the first few pixels pick the axis, and it
keeps to it until the fingers lift.

## Pager

Turning pages with the fingers (`pager.rs`): `Pager::new(axis, &motion)`, then `scroll(…)` or
`follow(moved, stop, page_px, count, &motion)` while two fingers slide. The page follows them one
to one and settles on the nearest page when they lift, or the next one on a flick (never more
than one). A wheel steps a page a notch (`step`), and `go(page)` / `jump(page)` move there with or
without the spring. `offset(count, &motion)` is the fractional page to draw, rubber-banded past
the ends. `PagerMotion` holds the settle's duration and easing, the flick speed and the rubber
band. The launcher turns it sideways through its categories.

## Dialog and alert

`kit::dialog::Dialog` is macOS 26's alert (`dialog/dialog-*.css`): a 260-wide card of thick glass
(`Material::Sheet`, radius `dialog.radius`) with an optional icon (`Icon::App(path)` or
`Icon::Symbol(glyph)`, `dialog.icon-size`), a bold title (`dialog.title`), a message
(`dialog.message`), and its buttons. Each button has a role:

- `.button(label, f)` is **cancel**, tonal. Escape answers with it.
- `.main_button(label, f)` is the **default**, in the accent. Return answers with it.
- `.destructive(label, f)` is red on a faint red fill (`Style::Destructive`).

The buttons sit **side by side** when there are at most two and every label fits half the row.
They're 32 tall, with cancel on the left and the default on the right. Otherwise they **stack**:
28 tall, the default on top, then the destructive one, with cancel at the bottom.
`.side_by_side()` and `.stacked()` choose for it. `height()` measures the card from its text,
so it's as tall as its content: 152 for the design's side-by-side sample, 216 stacked.

It shows in two places, both drawing `Dialog::content` on their glass:

- **An app's dialog:** `.open(parent)` puts it in a modal window over the app's window. niri's
  generated rule for a floating window of an amane app (a dialog floats) gives it the corners,
  the shadow (`0 18 48`) and the thick glass. Without that rule it's on the window background.
- **The shell's alert:** `alert::show(dialog)` puts it in the middle of the screen, on the
  `amane.alert` surface, which frosts itself and holds the keyboard until answered. Quick
  Settings' Restart…, Power Off… and Log Out… ask with it. In debug builds,
  `printf 'alert-test\nstacked\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock` shows a sample.

## workspace_menu

`workspace_menu::all(pick)` is a submenu of every workspace, `pick` run with the chosen one's
id. `workspace_menu::on_focused_output(pick)` is the focused monitor's workspaces, the empty one
niri keeps after the last called "New Workspace". Workspaces are named as niri names them, or
"Workspace N".

## Under the components

- `layout.rs` holds the layout primitives: `boxed`, `pad`, `icon` (every Material Symbols
  glyph), `caption`, `split_row`, `clipped_label`, `wrapped_lines` (text broken onto a few
  lines at the spaces, measured, for a caption centred a line at a time), `blank`, `slot`,
  `text_width` (measured), `inner_width` and cursor tracking.
- `icons.rs` holds the glyphs and their pickers (`battery_glyph`, `wifi_glyph`, `audio_glyph` …).
- `color.rs` holds `over`, `faded` and `with_alpha`.
- `motion.rs` holds the shared springs and fades (`spring`, `sheet`, `fade`) and the keyed list
  motion the notifications use.
- `material.rs` holds the materials, Apple's: glass by role (`Material::Control`, `Sheet` …), the
  frost it asks niri for (`shape`), what the surface draws where niri lays the tint and edge on
  itself (`fill`, `lit`, `stroke`), and the vibrant labels, fills and separator on it. See
  [materials.md](../design-system/materials.md).
- `glass_region.rs` is what's under the materials: moving shapes with their layer
  (`transformed`, `faded`), and the glass level every tint goes through (`level`: `Clear` 0.8 of
  the tint, `Tinted`, `Solid` opaque, and the system's high contrast, which, like `Solid`, turns
  the material off and brings the hairlines back).
  **Adaptive glass** (an experiment, off by default: `adaptive_glass` in `appearance.json`, or
  `AMANE_ADAPTIVE_GLASS=1`) turns small glass light or dark with its backdrop, as Apple's small
  glass does. Each piece names its shape (`shape_id`, sent as `BlurShape::id`), niri reports how
  bright the backdrop behind it is (amane's `Backdrop`, for a material with `report-backdrop true`
  in niri's `materials {}`), and `Material::adapted(id, ..)` builds the piece and its frost with
  the mode that reading gives (light above 0.65, dark again below 0.35). The dock's card, each
  Quick Settings module and each toast do this. Large glass keeps the theme's mode.
