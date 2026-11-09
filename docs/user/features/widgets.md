# Desktop Widgets

Desktop widgets on the wallpaper (`apps/shell/src/surfaces/widgets/`). Quick Settings' tiles are
controls; widgets are readings: the batteries, the processor, memory, temperature, the network
and the disks. Each is a frosted card on a clear surface under the windows, laid out on Quick
Settings' grid across the whole screen (the surface itself is only as big as the cards), and the same kind can be placed more than once, each
with its own settings.

## What there is

| Widget | Sizes | Shows | Click | Show (its settings) |
|---|---|---|---|---|
| Clock | Small, Medium, Large | the time, each digit one pen stroke; a digit that changes is erased along its line and the next drawn in. Small: hours over minutes; Medium: one row with colons, the date under it; Large: hours over minutes, bigger, the date under them. Seconds, when shown, sit under a stacked clock, smaller, and at the end of Medium's row. With 12-hour time, AM or PM goes under the digits, before the date. The date and AM/PM are in the label colour, whatever the digits' colour | nothing | **Color** (the accent, or one of the accent presets), **Line** (Thin, Regular, Bold), **Show Seconds**, **24-Hour Time** (on, by default, like the bar), **Show Date** (on) |
| Batteries | Small, Medium, Large | Small: one device's charge in a ring with the device in it, the percentage and its name; Medium: up to four rings (three and "+2" when there are more); Large: a list, the machine's battery with its time left (or "Charged") | Quick Settings' Battery page | which device a Small one shows (the first, by default) |
| CPU | Small, Medium, Large | how busy the processor is, the cores, a graph; Large adds a bar per core | the first of `missioncenter`, `gnome-system-monitor`, `resources` installed | **Graph** (the last 2 or 15 minutes) |
| Memory | Small, Medium, Large | memory in use, "8.9 GB of 29.1 GB", a graph; Large adds swap | as CPU | **Graph** |
| Temperature | Small, Medium, Large | the processor's sensor (else the hottest), its name ("CPU Tctl", "NVMe 2": the driver by what it watches), a graph scaled to what it read; Large adds the others, hottest first | as CPU | which sensor, **Graph**, **Fahrenheit** |
| Network | Small, Medium, Large | what comes in and goes out a second, a graph of each (what comes in filled, what goes out a line over it), the link's kind and the interface's name ("Wi-Fi · wlp194s0") | Emaki Settings' [Network page](network.md) (`nm-connection-editor` without it) | which interface (the default route's, by default), **Graph**, **Bits per Second** |
| Disk | Small, Medium | Small: how full one disk is, in a ring, and what's free; Medium: every disk as a bar | `baobab` or `gnome-disks` | which disk (the root, or the first disk where the root isn't one) |
| System | Large, Extra Large | CPU, memory and disk rings and the network's rates; Extra Large adds each one's graph | as CPU | — |

A reading turns the critical color past a limit: a processor or memory 90% busy, swap or a disk 90%
full, a sensor where it says it runs hot (hwmon's `max`, else 10 °C under its `crit`, else 85 °C), a
battery at 15% and not charging. The graphs span the last two minutes, or with **Graph** set to the
last 15 minutes, a quarter hour, every 7.5 readings averaged into one point; the newest is at the
right edge, and a widget placed a moment ago starts its graph at the right and fills leftwards.

A sensor, interface or disk a widget was set to that's gone (a drive taken out, a VPN down) shows
the automatic one in its place and says so ("NVMe 2 not found", "wg0 not connected", "/mnt/usb not
mounted") until it's back. A card with nothing to show at all says so in the middle ("No Devices",
"No disk").

## Using them

- **Move**: drag a card, at any time; the cell its top-left is nearest takes it and the others
  make way live, as in Quick Settings' edit mode (one card of its size swaps places with it,
  anything else goes to the first free cells; where nothing fits, it stays as it last fit). Let go
  off the canvas (more than half a cell past its edge) and it goes back.
- **Click** opens what it watches (above).
- **Right-click a card**: its sizes (the one it is checked, one there's no room for greyed out),
  **Show** (what it shows, for the kinds with a choice; the clock has **Color**, **Line** and its
  two switches instead), **Edit Widgets…** and **Remove Widget**.
- **Right-click the desktop**: **Edit Widgets…**.

The cards fade back while a window is open on the monitor's workspace, and come up
again on an empty one; during a workspace switch the fade follows the switch, part way between the
two workspaces as far as it has gone. The cards stay put while the workspaces slide over them, and
fade out in place as the overview opens. Emaki Settings' **Desktop** page has **Show widgets on the desktop** and the
**Widget style**: dim while windows are open (the default), always full, or always dimmed
(`~/.config/emaki/desktop.json`).

## Edit Widgets

**Edit Widgets…**, or `widgets edit` over IPC, edits one monitor's desktop. Its surface moves over
the windows (the overlay layer) and takes the keyboard; each
card gets a remove badge at its top-left and, when it comes in more than one size, a resize handle
at its bottom-right, both fading in with the gallery; and the **gallery** opens at the bottom of
the screen:

- **The sidebar**: a search field ("Search Widgets") over All Widgets, Clock, System and Batteries.
- **The pane**: every kind this machine has something for (no Batteries without a battery or a
  device that reports one), live, scaled to fit, named under it, over a small picture of each size
  it comes in: the one filled is the size it's added at (its default at first), and a click on
  another picks that one, the preview with it. Typing searches every kind by its name, what it
  shows and its category. The pane scrolls when they don't all fit.
- **The footer**: the hint (or how many widgets this screen has no room for, which show again on
  a bigger one; or, for a few seconds, that there's no room for one just clicked), **Undo Remove**
  (while there's something to undo), **Remove All** and **Done**.

An empty desktop says where widgets go. Drag a widget from the gallery onto the desktop to place it
(the others make way, an accent outline shows where it will land), or click it to put it in the
first free cells. Click a card (either button) for its menu, as out of edit mode but without Edit
Widgets…; its Remove Widget can be undone. Remove All takes every card on the screen, which Undo
Remove puts back; widgets this screen has no room for stay. Drag a card onto
the gallery, or off the canvas, to remove it; Undo Remove puts it back, settings and all, where it
was if that's still free (else in the first free cells), leaving what was placed or moved since as
it is; pressed again, it puts back the removal before that, and so on, until Done. Escape during a
drag lets go of it, and the pointer takes nothing more until the button comes up. Drag a
handle to resize a card between the sizes it comes in; an outline shows how far it has been pulled.
**Done**, or Escape, puts the desktop back under the windows.

The keyboard: **Tab** (Shift-Tab) takes each card in turn, row by row, ringed in the accent. With
one taken, the **arrows** move it a cell (the others making way), **Shift** and an arrow steps its
size (right or down bigger, left or up smaller), **Delete** or **Backspace** removes it, and
**Enter** opens its menu. With none taken, **Enter** adds the first widget the gallery shows.
**Ctrl+Z** is Undo Remove. **Escape** lets go of a drag, then of the card taken, then clears the
search, then is Done. Anything else types into the search.

A card under the gallery can't be reached with the pointer while it's open; take it with Tab and
move it with the arrows.

The edit view is a surface of its own on the overlay layer, shown only while editing, while the
desktop's surface leaves its cards out: niri doesn't frost a surface moved between layers, so
the desktop's isn't. It draws nothing outside its glass (no scrim over the windows): a surface
that paints over its whole area gets no frost from niri, and its glass, which leaves the tint to
niri, would come out clear.

## The grid

The canvas is Quick Settings' grid: 74 px cells, 16 px between them at the comfortable density (`quick.grid.cell`, `.gap`; `grid::geometry`), as many
whole cells as fit on the screen inside a 24 px margin and above the bar, centred in what's left;
on a 1920 × 1080 screen that's 20 × 11. The sizes on that grid: Small 2 × 2 (164 ×
164), Medium 4 × 2, Large 4 × 4 and Extra Large 8 × 4.

The grid, the layout and the drag are the kit's `grid`, which Quick Settings is laid
out on too: `geometry` (spans, places, the canvas, its pixels, which cells are taken), `layout`
(where each item sits, and the layout a move, a placement or a resize would leave: swap or push)
and `drag` (what the pointer holds, and the preview a drag makes of the layout it began from).
Each owner brings its own sizes (`GridKey`), its own cells and its own look; the grid reads no
tokens.

## One surface a monitor

Each monitor has a surface of its own (`App::window_per_monitor`) on the background layer under
the windows (and its edit surface, above), covering the output (`Zone::Ignore`; the canvas keeps clear of the bar itself). Its
namespace is `emaki.widgets.<output>`, so a card's menu hangs under the surface on its own monitor
(amane parents a popup to the first surface with the namespace asked for). niri hands a
background surface the pointer wherever no window is, so the desktop's clicks reach it, and
the fork's `desktop` layer rule ([`layout/desktop-layer.md`](https://github.com/emaki-desktop/niri/blob/emaki/docs/fork/layout/desktop-layer.md)) draws it once, on the wallpaper (the overview's backdrop) under
the workspaces, rather than with each workspace: the cards stay put while workspaces slide over
them, keep their size as the overview opens, and fade out with it, frost and all, in step with the
gesture, with no frame of the shell's. They take no input in the overview. A card's menu still
shows above the windows. The workspaces must be transparent (`layout { background-color
"transparent" }`) for the backdrop to show, as for the wallpaper.

While nothing under the windows shows on its monitor (niri's `DesktopObscured`, through amane's
`Desktop` service, [`compositor/services.md`](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/compositor/services.md): the overview fully
open, or windows, each with its gap, covering the screen with no switch going on), the surface is
frozen (`LayerWindow::frozen`): its last frame stays up and nothing is drawn or committed, as the
view reads only what says when it shows again. The readings go on gathering, so when the desktop
shows again the first frame has the graphs with what came in meanwhile, the clock at the time
without drawing its digits' changes, the cards where they are and the fade where it's going. A
translucent window covering the screen counts as covering it. The edit surface never freezes.

Every press, drag and click lands on one still layer over the whole surface, the first thing on
it; the cards take no input of their own. amane measures a drag against the hit areas of the last
frame and keeps the target being dragged by its place among them, so a drag measured on a card
that moves every frame overshoots; the layer never moves. A press becomes a drag once the pointer
has moved 6 px, so a click stays a click.

The layout is per monitor, by connector name, in the state file `widgets.json`
([services.md](../../developer/integration/services.md#settings-and-state)); the older `widgets`, the same
shape with `"version": 1` inside, is read when that isn't there:

```json
{ "version": 1, "data": { "next_id": 3, "outputs": { "eDP-1": [
    { "id": 1, "kind": "battery", "cols": 2, "rows": 2, "col": 0, "row": 0 },
    { "id": 2, "kind": "disk", "cols": 4, "rows": 2, "col": 2, "row": 0, "settings": { "mount": "/home" } } ] } } }
```

The kinds are `clock`, `battery`, `cpu`, `memory`, `temperature`, `network`, `disk` and `system`;
the settings `device`, `interface`, `mount`, `sensor`, `range` (`15`, for CPU, memory, temperature
and network), `unit` (temperature's `f`, network's `bits`), and the clock's `color` (`#rrggbb`),
`line` (`thin`, `bold`), `seconds` (`on`), `hours` (`12`) and `date` (`off`); a setting the kind
doesn't have, or a value it doesn't take, is dropped. A layout is shown refitted to the screen it's
on: a card that no longer fits moves to the first free cells, or hides where there are none. What
the refit moved is never saved, so a smaller screen or a bigger scale doesn't lose where the cards
were on the bigger one; a card moved by hand is kept where it went. A file that can't be read (broken,
or from another version) is copied to `widgets.json.bad` beside it and the default shown; a widget
that can't be read is dropped with the rest kept, as are unknown kinds; and overlaps move to the first free
cells. When a card moved by hand lands on the cells another kept (one shown elsewhere on a smaller
screen), that one moves to the first free cells, so two never share them. With no file at
all, the first monitor starts with the battery (where there is one) and the CPU.

## Frost

Each card is a `Material::Control` blur shape (`glass_region`), as Quick Settings' modules are,
lit in one pass over all of them (`glass::rim_layers`) where niri doesn't draw the material. The
layer rule (`emaki.widgets.` matched as a prefix) is `clear`: `xray false`, as niri's xray buffer
for the background layer holds this surface too, and `desktop true`. The cards' frost fades with
them when they dim, where niri can fade frost (`amane::blur_fades()`); niri fades it with the
overview itself.

## Backends

`system_stats` (`apps/shell/src/services/desktop/system_stats.rs`) is one service on one 2 s tick, so a desktop of
widgets redraws once a tick: `/proc/stat` (the processor and each core), `/proc/meminfo` (memory
in use is what isn't available, as `free` counts it, and swap), `/proc/net/dev` and
`/proc/net/route` (each interface's rates, and the default route: the one with the lowest metric; an
interface that comes up starts its graph on its second reading, not with all it ever carried),
`/sys/class/hwmon` (each temperature sensor, named by its driver and label, with a number after
a name two share, as two drives' "nvme Composite"; the processor's is the first of `k10temp`,
`coretemp`, `zenpower` or `cpu_thermal`; one that doesn't answer, as a drive asleep, is left out
while the rest still show) and, every 30 s, `statvfs` on the filesystems on a block
device in `/proc/mounts` (a mount whose `statvfs` stalls for a second is skipped for the next few).
It keeps the last 15 minutes of each, starts the first time a widget reads it, and reads nothing
while no system card shows (or the gallery is open). The files are read with no lock held, so a
slow sensor never holds up drawing; the hwmon sensors are looked up once a minute, not every tick.
The batteries are amane's `Battery` (UPower's peripherals too) and `Bluetooth` (the
charge a connected device reports that UPower doesn't know).

## IPC

`widgets list [--json] | kinds | add | remove | set | resize | move | undo | reset-layout | edit |
done` lists, places, sets, moves, resizes and removes widgets and opens Edit Widgets from a script
([ipc.md](../../developer/integration/ipc.md#widgets)); `kinds` says what each kind can be set to.
Emaki Settings' Desktop page lists each monitor's widgets with Remove, and Remove All, through the
same commands.

## Components

- **`mod.rs`**: the geometry, each monitor's desk, and what the others share.
- **`window.rs`**: a monitor's window each frame: hidden, frozen under the windows, or the cards.
- **`view.rs`**, **`input.rs`**, **`menu.rs`** and **`ipc.rs`**: drawing and dimming the cards, the
  pointer, the menus, and IPC.
- **`kind.rs`**: `WidgetSize`, `WidgetKind` and its `KindSpec` (everything a kind is in one place:
  its name, sizes, category, glyph, what a click opens, which readings it needs, and its
  settings, each a `SettingSpec`, from which its menu is built), and `WidgetKey` (an instance: its
  id and kind; the grid's `GridKey`). A new kind is one more `KindSpec` and its face in `catalog/`;
  nothing about it falls to a default.
- **`store/`**: the layouts and settings (a widget's settings are its spec's keys and their
  values; anything else in the file is dropped), the file, refitting and committing.
- **`catalog/`**: every face, and the pieces they're drawn with (a header, a figure, a ring, a
  sparkline and a bar on amane's `Canvas`), each laid out by hand for its size.
- **`edit.rs`** and **`gallery.rs`**: Edit Widgets and the gallery. How a press becomes a drag,
  whether a drag let go lands, and the outlines (where a card will land, how far a resize has
  pulled) are shared with Quick Settings' edit mode (`chrome/grid_edit.rs`).

Tokens: the widgets' own sizes are the `widget.*` group (the card's padding and corners, the
dimming, the drag's dead band and threshold, the figures' sizes, the gallery's sizes); what they
share with Quick Settings, whose grid they're laid out on, is `quick.*` (the grid's cell, gap and
margin, the shadow, the lift of a card in hand); everything else is `sys.*`. `widget.*` is in
Penpot, where the faces and the card's menu are drawn ("Widgets — Components" and "Widgets"). The
grid follows the density, as Quick Settings' does.

## Related

- Quick Settings, whose grid this is: [quick-settings.md](quick-settings.md).
- The niri layer rule: [layer-rules.md](../../developer/integration/layer-rules.md); niri's desktop layer:
  [`layout/desktop-layer.md`](https://github.com/emaki-desktop/niri/blob/emaki/docs/fork/layout/desktop-layer.md); its `DesktopObscured` and workspace-switch
  events: [`ipc/events.md`](https://github.com/emaki-desktop/niri/blob/emaki/docs/fork/ipc/events.md).
- amane's `Desktop` and `WorkspaceSwitch` services and frozen layer windows:
  [`compositor/services.md`](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/compositor/services.md).
