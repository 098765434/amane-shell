# Desktop Widgets

macOS's desktop widgets on the wallpaper (`src/components/widgets/`). Quick Settings' tiles are
controls; widgets are readings: the batteries, the processor, memory, temperature, the network
and the disks. Each is a frosted card on a clear surface under the windows, laid out on Quick
Settings' grid across the whole screen, and the same kind can be placed more than once, each
with its own settings.

## What there is

| Widget | Sizes | Shows | Click | Show (its settings) |
|---|---|---|---|---|
| Clock | Small, Medium, Large | the time, each digit one pen stroke; a digit that changes is erased along its line and the next drawn in. Small: hours over minutes; Medium: one row with colons, the date under it; Large: hours over minutes, bigger, the date under them. Seconds, when shown, sit under a stacked clock, smaller, and at the end of Medium's row | nothing | **Color** (the accent, or one of the accent presets), **Line** (Thin, Regular, Bold), **Show Seconds**, **24-Hour Time** (on, by default, like the bar) |
| Batteries | Small, Medium, Large | Small: one device's charge in a ring with the device in it, the percentage and its name; Medium: up to four rings; Large: a list, the machine's battery with its time left (or "Charged") | Quick Settings' Battery page | which device a Small one shows (the first, by default) |
| CPU | Small, Medium, Large | how busy the processor is, the cores, a graph; Large adds a bar per core | the first of `missioncenter`, `gnome-system-monitor`, `resources` installed | — |
| Memory | Small, Medium, Large | memory in use, "8.9 GB of 29.1 GB", a graph; Large adds swap | as CPU | — |
| Temperature | Small, Medium, Large | the processor's sensor (else the hottest), its name, a graph; Large adds the others, hottest first | as CPU | which sensor |
| Network | Small, Medium, Large | what comes in and goes out a second, a graph of each, the interface's name | `nm-connection-editor` | which interface (the default route's, by default) |
| Disk | Small, Medium | Small: how full one disk is, in a ring, and what's free; Medium: every disk as a bar | `baobab` or `gnome-disks` | which disk (the root, or the first disk where the root isn't one) |
| System | Large, Extra Large | CPU, memory and disk rings and the network's rates; Extra Large adds each one's graph | as CPU | — |

A reading turns the critical colour past a limit: a processor or memory 90% busy, a disk 90% full, a
sensor at 85 °C, a battery at 15% and not charging. The graphs span the last two minutes, the
newest at the right edge; a widget placed a moment ago starts its graph at the right and fills
leftwards.

## Using them

- **Move**: drag a card, at any time; the cell its top-left is nearest takes it and the others
  make way live, as in Quick Settings' edit mode (one card of its size swaps places with it,
  anything else goes to the first free cells; where nothing fits, it stays as it last fit). Let go
  off the canvas and it goes back.
- **Click** opens what it watches (above).
- **Right-click a card**: its sizes (the one it is checked, one there's no room for greyed out),
  **Show** (what it shows, for the kinds with a choice; the clock has **Color**, **Line** and its
  two switches instead), **Edit Widgets…** and **Remove Widget**.
- **Right-click the desktop**: **Edit Widgets…**.

As on macOS, the cards fade back while a window is open on the monitor's workspace, and come up
again on an empty one. Shell Settings' **Desktop** page has **Show widgets on the desktop** and the
**Widget style**: dim while windows are open (the default), always full, or always dimmed
(`~/.config/amane-shell/desktop.json`).

## Edit Widgets

**Edit Widgets…**, or `widgets edit` over IPC, edits one monitor's desktop. Its surface moves over
the windows (the overlay layer) and takes the keyboard; each
card gets a remove badge at its top-left and, when it comes in more than one size, a resize handle
at its bottom-right; and the **gallery** opens at the bottom of the screen:

- **The sidebar**: a search field ("Search Widgets") over All Widgets, Clock, System and Batteries.
- **The pane**: every kind, live, at its default size and scaled to fit, named under it. Typing
  searches every kind by its name, what it shows and its category.
- **The footer**: the hint, **Undo Remove** (while there's something to undo), **Remove All**
  and **Done**.

Drag a widget from the gallery onto the desktop to place it (the others make way, an accent
outline shows where it will land), or click it to put it in the first free cells. Drag a card onto
the gallery, or off the canvas, to remove it; Undo Remove puts it back, settings and all. Drag a
handle to resize a card between the sizes it comes in. **Done**, or Escape (which first lets go of
a drag, then clears the search), puts the desktop back under the windows.

The edit view is a surface of its own on the overlay layer, shown only while editing, while the
desktop's surface leaves its cards out: niri doesn't frost a surface moved between layers, so
the desktop's isn't. It draws nothing outside its glass (no scrim over the windows): a surface
that paints over its whole area gets no frost from niri, and its glass, which leaves the tint to
niri, would come out clear.

## The grid

The canvas is Quick Settings' grid: 74 px cells, 16 px between them (`grid::geometry`), as many
whole cells as fit on the screen inside a 24 px margin and above the bar, centred in what's left;
on a 1920 × 1080 screen that's 20 × 11. The sizes are macOS's on that grid: Small 2 × 2 (164 ×
164, near macOS's 170 pt), Medium 4 × 2, Large 4 × 4 and Extra Large 8 × 4.

The grid, the layout and the drag are the shared `components::grid`, which Quick Settings is laid
out on too: `geometry` (spans, places, the canvas, its pixels, which cells are taken), `layout`
(where each item sits, and the layout a move, a placement or a resize would leave: swap or push)
and `drag` (what the pointer holds, and the preview a drag makes of the layout it began from).
Each owner brings its own sizes (`GridKey`), its own cells and its own look; the grid reads no
tokens.

## One surface a monitor

Each monitor has a surface of its own (`App::window_per_monitor`) on the background layer under
the windows (and its edit surface, above), covering the output (`Zone::Ignore`; the canvas keeps clear of the bar itself). Its
namespace is `amane.widgets.<output>`, so a card's menu hangs under the surface on its own monitor
(amane parents a popup to the first surface with the namespace asked for). niri hands a
background surface the pointer wherever no window is, so the desktop's clicks reach it, and
draws it with each workspace: the cards slide with a workspace switch and show on every
workspace in the overview, where they take no input.

Every press, drag and click lands on one still layer over the whole surface, the first thing on
it; the cards take no input of their own. amane measures a drag against the hit areas of the last
frame and keeps the target being dragged by its place among them, so a drag measured on a card
that moves every frame overshoots; the layer never moves. A press becomes a drag once the pointer
has moved 6 px, so a click stays a click.

The layout is per monitor, by connector name, in `$XDG_STATE_HOME/amane-shell/widgets`:

```json
{ "version": 1, "next_id": 3, "outputs": { "eDP-1": [
    { "id": 1, "kind": "battery", "cols": 2, "rows": 2, "col": 0, "row": 0 },
    { "id": 2, "kind": "disk", "cols": 4, "rows": 2, "col": 2, "row": 0, "settings": { "mount": "/home" } } ] } }
```

The kinds are `clock`, `battery`, `cpu`, `memory`, `temperature`, `network`, `disk` and `system`;
the settings `device`, `interface`, `mount`, `sensor`, and the clock's `color` (`#rrggbb`), `line`
(`thin`, `bold`), `seconds` (`on`) and `hours` (`12`). A layout is shown refitted to the screen it's
on: a card that no longer fits moves to the first free cells, or hides where there are none. What
the refit moved is never saved, so a smaller screen or a bigger scale doesn't lose where the cards
were on the bigger one; a card moved by hand is kept where it went. A file from another version is
ignored; unknown kinds are dropped, and overlaps move to the first free cells. With no file at
all, the first monitor starts with the battery (where there is one) and the CPU.

## Frost

Each card is a `Material::Control` blur shape (`glass_region`), as Quick Settings' modules are,
lit in one pass over all of them (`glass::rim_layers`) where niri doesn't draw the material. The
layer rule (`amane.widgets.` matched as a prefix) is `clear`: `xray false`, as niri's xray buffer
for the background layer holds this surface too. The cards' frost fades with them when they dim,
where niri can fade frost (`amane::blur_fades()`).

## Backends

`system_stats` (`src/services/system_stats.rs`) is one service on one 2 s tick, so a desktop of
widgets redraws once a tick: `/proc/stat` (the processor and each core), `/proc/meminfo` (memory
in use is what isn't available, as `free` counts it, and swap), `/proc/net/dev` and
`/proc/net/route` (each interface's rates, and the default route), `/sys/class/hwmon` (each
temperature sensor, named by its driver and label; the processor's is the first of `k10temp`,
`coretemp`, `zenpower` or `cpu_thermal`) and, every 30 s, `statvfs` on the filesystems on a block
device in `/proc/mounts` (a mount whose `statvfs` stalls for a second is skipped for the next few).
It keeps the last 15 minutes of each, starts the first time a widget reads it, and reads nothing
while no system card shows (or the gallery is open). The files are read with no lock held, so a
slow sensor never holds up drawing; the hwmon sensors are looked up once a minute, not every tick.
The batteries are amane's `Battery` (UPower's peripherals too) and `Bluetooth` (the
charge a connected device reports that UPower doesn't know).

## IPC

```sh
printf 'widgets\nadd\ncpu\nmedium\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
printf 'widgets\nlist\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
```

`widgets list` (one line a widget: monitor, id, kind, size, cell), `add <kind> [size] [output]`
(in the first free cells), `remove <id>`, `reset-layout [output]` (no widgets), `edit [output]` and
`done`. Without an output it's the focused workspace's monitor.

## Components

- **`mod.rs`**: the surface, the still layer, moving, the menus, dimming and IPC.
- **`kind.rs`**: `WidgetSize`, `WidgetKind` (its sizes, category, glyph, what a click opens) and
  `WidgetKey` (an instance: its id and kind; the grid's `GridKey`).
- **`store.rs`**: the layouts and settings, the file, refitting and committing.
- **`catalog/`**: every face, and the pieces they're drawn with (a header, a figure, a ring, a
  sparkline and a bar on amane's `Canvas`), each laid out by hand for its size.
- **`edit.rs`** and **`gallery.rs`**: Edit Widgets and the gallery.

Tokens: the code holds its sizes as `TODO(token)` constants until Penpot has a `widget.*` group
(see [penpot-todo.md](penpot-todo.md)); everything else is `sys.*`.

## Related

- Quick Settings, whose grid this is: [quick-settings.md](quick-settings.md).
- The niri layer rule: [layer-rules.md](layer-rules.md).
