# Components

amane-kit's building blocks (`crates/kit/src/`, `amane_kit::*`), which Quick Settings, the notification
center, the context menu, the dock, the launcher and Shell Settings are made of. Like the
[buttons](controls.md), each is named and structured after its Material 3 counterpart and drawn in
the shell's glass look. They read `sys.*` tokens only, so any component can use them.

| Component | `crates/kit/src/` | M3 counterpart | Used for |
|---|---|---|---|
| `Card` | `containers/card.rs` | [Cards](https://m3.material.io/components/cards/overview) | the notification center's sheet, context menus, tooltips |
| `TopAppBar` | `containers/app_bar.rs` | [Top app bar](https://m3.material.io/components/app-bars/overview) (small) | the title row of each Quick Settings page and of the center |
| `List`, `ListItem` | `containers/list.rs` | [Lists](https://m3.material.io/components/lists/overview) | networks, Bluetooth devices, audio devices, peripherals |
| `Divider` | `containers/divider.rs` | [Divider](https://m3.material.io/components/divider/overview) | context-menu separators, the dock's rules between workspaces (`Divider::vertical`) |
| `Slider` | `controls/slider.rs` | [Sliders](https://m3.material.io/components/sliders/overview) | output and microphone volume, Quick Settings' sliders |
| `tooltip` | `containers/tooltip.rs` | [Tooltips](https://m3.material.io/components/tooltips/overview) (plain) | the bar's volume level and battery charge |
| `app_icon` | `platform/app_icon.rs` | none | the dock's icons, notification cards' app icons, the launcher's apps |
| `EdgeReveal` | `animation/edge_reveal.rs` | none | the dock hiding at the bottom edge, the status bar at either |
| `ScrollView` | `containers/scroll_view.rs` | none | the launcher's grid, results and pill bar, the Customize window's panes |
| `Flow` | `containers/flow.rs` | none | the Customize window's controls, flowing into rows |
| `Dialog` | `containers/dialog.rs` | [Dialogs](https://m3.material.io/components/dialogs/overview) (basic) | the shell's alert, an app's modal dialog |
| `Tag` | `controls/tag.rs` | [Chips](https://m3.material.io/components/chips/overview) (assist, without the action) | the Displays page's facts about a screen |
| `Arrangement` | `containers/arrangement.rs` | none | the Displays page's screens, dragged into place |

## Card

`Card::new(content)` fills its parent and stacks `content` `.gap(…)` apart inside a
`sys.space.inset.l` inset (`.padding(…)` changes it). It's made of a `Material`
(`.material(…)`, `Material::Popover` by default; see [materials.md](../design-system/materials.md)), whose tint fills
it. What sits on it takes the vibrant labels and fills (`material::label`, `fill_role`).

`Variant::Filled` is the fill alone, and `Variant::Outlined` adds a `sys.border.hairline` in the
material's separator. M3's elevated card is left out because there's no `sys.shadow` role yet.
`.blur(true)` frosts what's below with `sys.effect.backdrop-blur`, a departure from M3. It
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

`ListItem::new(id, headline)` is a one-line M3 list item, or two-line with `.supporting(text)`
(a caption under the headline, elided too; the row is `sys.size.row.two-line-height`). It has a `.leading(glyph)` (or
`.leading_tile(glyph)`, the glyph on a `MacLeadingIcon` tile of the accent, as a sidebar names its
sections, or `.leading_tile_accent(glyph, Accent)` on a tile of one of the system accents, or
`.leading_widget(…)`, any widget in its place, such as an app's `app_icon::entry_tile` at
`list::leading_side()`), the
headline (elided), and `.trailing(…)` items such as `list::check()`, a lock glyph, or
`list::trailing_text(…)`. A `.selected(true)` item is filled `sys.color.state.selected` and its
headline is full-strength. The rest read the secondary label, and `.color(…)`
overrides that. With `.on_press` or `.on_secondary` it's clickable and shows the
`sys.color.state.hover` layer, keyed by `id` like a button's. `.bare(true)` leaves out the
secondary fill, for rows on a surface that reads as one piece (the Customize window's
sidebar): only the selected and hover fills show. Overline text is the next M3 slot to add when
a list needs it.

`List::new(items)` stacks items `sys.space.gap.xs` apart, under an optional `.header(…)`
caption, with an `.empty(…)` note while there are none.

## Form

`form` lays settings out as grouped rows, and is what Shell Settings' pages are
built from. A `Group::new()` is a rounded card (radius 12, `groupFill`) of rows with hairlines
between them, starting past each row's tile; `.header(text)` is a caption above it and
`.footer(text)` a note below it, wrapped to its `.width(…)` (504 by default, Shell Settings'
page). `.row(FormRow)`, `.rows(…)`, `.slider(SliderRow)` and `.custom(widget, height)` add to it,
and `.build()` gives a `Column`. Every row knows its height, so a group sizes itself
(`Group::height()`).

- `FormRow::new(id, title)`: 42 px tall with one line (more with a `.subtitle(…)`, which wraps),
  12 px in. `.icon(glyph, Accent)` puts a `MacLeadingIcon` before the title, or `.leading(widget,
  side)` anything else (an app's icon). `.trailing(widget)` adds a control at the trailing edge
  (several sit side by side), `.value(text)` a grey value. `.detail(widget, height)` puts a block
  under the title, lined up with it (a radio group, a field and its button, tags).
  `.on_press(…)` makes the whole row a button with a chevron, to open what it names;
  `.selected(true)` fills it as the one picked; `.disabled(true)` dims it. `.child()` sets it in
  under the row before it (a setting that belongs to that one); `.loading()` puts a `Spinner` at
  its trailing edge and greys its title while its value is read; `.highlight(true)`, or
  `form::highlight_row(Some(id))` for whichever row has that id, lights it in the accent (the
  row a search led to). A title too long for the room left is cut short with an ellipsis.
- `SliderRow::new(title, |width| slider)`: the title and its `.value(…)` over a slider across the
  row, with a `.glyph(…)` or `.icon(…)` before them.

A group with only a caption or note keeps them (`is_empty()` is false; `has_rows()` says whether
it has rows), so a page whose rows are still loading isn't blank.

Pieces that go in rows:

- `spinner::Spinner::new(size)`: an indeterminate spinner of eight fading spokes, turning about
  ten times a second while a window shows it (each build asks for the next step, so a window
  that stops showing one rests); under reduced motion it breathes instead.
  `spinner::ProgressBar::new(fraction, width)`: a thin accent track.
- `macform::ThumbnailPicker::new(id, width, height)`: choices as small pictures (`.option(widget,
  label)`), the `.chosen(…)` one ringed in the accent, wrapping to `.width(…)`; `.height()` for a
  row's detail. Shell Settings draws its pictures with flat shapes, so they stay on quads.
- `macform::ColorWells::new(id, colors, chosen)`: colors as dots, `None` for a multicolor
  "System" one; `.shown_name()` is the hovered or chosen one's name, for the row's subtitle.

## Divider and Slider

The thin `Slider` takes `fill` and `thickness` in place of the accent and `sys.size.slider.height`:
Quick Settings' glass cards fill it with the panel's `fg`, a documented departure
from the one-accent rule (§11.2).


`Divider::build()` is a `sys.border.hairline` line in `sys.color.separator`, with `gap.xs` above
and below. `Divider::height()` is what it takes up, for menus that measure themselves.

`Slider::new(value, width)` sets a 0–100 level when pressed, dragged or scrolled. Its thin
`sys.size.slider.height` track, outlined in `sys.color.progress.track-stroke`, is filled with the
accent, or greyed with `.dimmed(true)`. A pill knob at the current control size
(`control_size::current().slider_knob()`, 20×16 at Medium) sits at the level; an `.inert(true)`
slider is a meter and has none. The hit area is a full control tall. One scroll notch moves it `slider::STEP` (5). The bar's volume button
uses the same `slider::scrolled`. `.on_value_change` takes any closure.

`.large(glyph)` is Quick Settings' large slider: a track a full control tall with the glyph
inside it, at the start of a fill that's never narrower than the glyph's circle, so the glyph is
the knob. `.on_icon_press(id, action)` makes the glyph a button (Quick Settings' mute).

Quick Settings' own parts (the tile, the grid, the page) are in
[quick-settings.md](../shell/quick-settings.md).

## Tooltip

A plain tooltip: one `sys.type.caption` line on a frosted, glass-edged `Card` of `Material::ToolTip`, with a `sys.space.inset.s` inset. niri blurs behind it from the bar's
`popups {}` layer rule, which has the same `sys.radius.popover` corners. It shows after the pointer rests on its
control for half a second. It's an xdg-popup with no grab, so it never takes input or focus. It
hangs under the control's surface and is centred above the pointer, sliding inward
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
the bottom edge (the top, made `at_top(true)` or `set_top(true)`) while hidden, and `shown` too
while any of it is out. Once the pointer has been gone for `delay`, it goes back in.

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

## Flow

`Flow::new(width, gap_x, gap_y)` lays fixed-size `.item(width, height, widget)`s out in rows:
each row takes items until the next wouldn't fit (an item wider than the row has a row to
itself), the items at the top of their row, the rows as tall as their tallest. `build()` gives
the widget, how tall it is and where each item went (`Placed`), so a caller can record where its
items are drawn. The row breaking is `flow::place`, pure and tested.

## Dialog

`kit::dialog::Dialog` is the shell's alert (`dialog/dialog-*.css`): a 260-wide card of thick glass
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

An app shows it with `.open(parent)`, in a modal window over the app's window. niri's generated
rule for a floating window of an amane app (a dialog floats) gives it the corners, the shadow
(`0 18 48`) and the thick glass. Without that rule it's on the window background. The shell shows
the same card as its alert, in the middle of the screen
([quick-settings.md](../shell/quick-settings.md#the-alert)).

`.countdown(seconds, f)` makes it answer by itself: a line under the message counts down
("Going back in 15 seconds."), and at zero the dialog closes and runs `f`, usually the cancel
button's action. The count starts when it opens. A `Tick` service redraws the dialog's window
once a second, and stays quiet while no countdown dialog is open. Shell Settings uses it to
undo a display change nobody confirmed ([displays.md](../shell/displays.md)).

## Tag

`Tag::new(text)` is a short fact as a capsule, `control.mini` tall: a caption on the secondary
fill, or `.accent(true)` for the one fact to see first (the accent's colour on its 16 % tint).
`size()` gives its width and height before it's built. `tag::tags(width, tags)` lays several out
in rows that wrap, through [`Flow`](#flow), and says how tall they came out.

## Arrangement

`Arrangement::new(id, screens, width, height)` draws `Screen`s (a label and a logical-pixel
rectangle each) where they sit relative to each other, scaled to fit the area with a fifth of it
spare around them and centred, the `.selected(index)` one in the accent. A press picks a screen
(`.on_select`); a drag of more than 3 px carries it, drawn over the others, and on release
`arrangement::snap` puts it against the nearest edge of another screen, overlapping along it,
with its edges lined up to that screen's when within a tenth of its size. `.on_move(index, x, y)`
then gets its new top-left in logical pixels. A lone screen can't be dragged.

The drag is measured on one still layer the size of the area, under the screens, which take no
input themselves, so the screen moving under the pointer never changes what the pointer is
measured against. It's all quads.

## Under the components

- `layout.rs` holds the layout primitives: `boxed`, `pad`, `icon` (every Material Symbols
  glyph), `caption`, `split_row`, `clipped_label`, `wrapped_lines` (text broken onto a few
  lines at the spaces, measured, for a caption centred a line at a time), `blank`, `slot`,
  `text_width` (measured), `inner_width` and cursor tracking.
- `icons.rs` holds the glyphs and their pickers (`battery_glyph`, `wifi_glyph`, `audio_glyph` …).
- `color.rs` holds `over`, `faded` and `with_alpha`.
- `motion.rs`, `kinetic.rs` and `pager.rs` are the shared springs and fades, scrolling with
  momentum and turning pages: see [motion.md](motion.md).
- `material.rs` holds the materials: glass by role (`Material::Control`, `Sheet` …), the
  frost it asks niri for (`shape`), what the surface draws where niri lays the tint and edge on
  itself (`fill`, `lit`, `stroke`), and the vibrant labels, fills and separator on it. See
  [materials.md](../design-system/materials.md).
- `glass.rs` (and its shader, `glass.wgsl`) is the lit edge and glow a translucent surface wears
  where niri draws no glass material of its own.
- `marker.rs` holds `marker!`: a service holding nothing, for state kept elsewhere, so only the
  windows that `watch()` it draw again when it's `changed()` ([performance.md](../development/performance.md)).
- `settings.rs` holds the shell's settings files, `appearance.json`, `desktop.json` and
  `launcher.json` under `~/.config/amane-shell/`, which a settings app changes and the shell
  follows live. It also holds amane-drop's `DropGrids` and `DropSharing`, which Drop and Shell
  Settings share ([drop.md](../shell/drop.md#settings)).
- `secrets.rs` keeps an app's passwords and keys in the Secret Service through `secret-tool`
  (`application <app> account <name>`), and in `~/.config/<app>/secrets.json` (0600) when no
  keyring answers. `lookup`, `store` and `forget` wait on `secret-tool`, so they're for a thread
  of their own. `known` is for drawing: it answers from what was last found and looks on a
  thread when it doesn't know yet (`SecretsChanged`). `kept_in_file` says whether a page should
  warn.
- `theme.rs` holds the runtime inputs the tokens resolve with (mode, density, accent, text scale)
  and the ones the materials follow (glass level, higher contrast, reduced motion, adaptive glass,
  vibrancy), each from its `AMANE_*` override, else the settings file, else the system
  ([tokens.md](../design-system/tokens.md)).
- `tokens/` holds the Penpot resolver, the manual's checks (`cargo xtask tokens check`) and the
  functions `build.rs` generates, one per token.
- `glass_region.rs` is what's under the materials: moving shapes with their layer
  (`transformed`, `faded`), and the glass level every tint goes through (`level`: `Clear` 0.8 of
  the tint, `Tinted`, `Solid` opaque, and the system's high contrast, which, like `Solid`, turns
  the material off and brings the hairlines back).
  **Adaptive glass** (an experiment, off by default: `adaptive_glass` in `appearance.json`, or
  `AMANE_ADAPTIVE_GLASS=1`) turns small glass light or dark with its backdrop.
  Each piece names its shape (`shape_id`, sent as `BlurShape::id`), niri reports how
  bright the backdrop behind it is (amane's `Backdrop`, for a material with `report-backdrop true`
  in niri's `materials {}`), and `Material::adapted(id, ..)` builds the piece and its frost with
  the mode that reading gives (light above 0.65, dark again below 0.35). The dock's card, each
  Quick Settings module and each toast do this. Large glass keeps the theme's mode.
