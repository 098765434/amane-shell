# Context menu

A reusable, macOS-style context menu with arbitrarily nested submenus
(`src/components/context_menu/`). Each menu level is an **xdg-popup**, so the compositor positions
it, keeps it on screen, grabs input for it and dismisses it on an outside click. The cards are
frosted by the niri layer rule for `amane.context-menu-anchor` ([layer-rules.md](layer-rules.md)).

| File | Contents |
|------|----------|
| `mod.rs` | public types (`ContextMenu`, `MenuItem`, `Icon`), the anchor surface, the per-level popups, row rendering, pointer and keyboard handling, sizing |
| `runtime.rs` | the state: registered menus, the open stack of levels, and the open/close/arm functions |

## Using it

### 1. Build and register a menu

Menus are built once with a builder and registered under a name:

```rust
use components::context_menu::{self, ContextMenu, Icon, MenuItem};

let open_with = ContextMenu::new()
    .item(MenuItem::label("Firefox").on_select(|| eprintln!("firefox")))
    .item(MenuItem::label("Preview").on_select(|| eprintln!("preview")));

let menu = ContextMenu::new()
    .item(
        MenuItem::label("Open")
            .icon(Icon::glyph("\u{1F4C2}"))
            .shortcut("\u{2318}O")
            .on_select(|| eprintln!("open")),
    )
    .item(MenuItem::label("Open With").submenu(open_with))
    .separator()
    .item(MenuItem::label("Duplicate").disabled())
    .item(MenuItem::label("Move to Trash").destructive().on_select(|| eprintln!("trash")));

context_menu::register("file", menu);
```

| Builder | Effect |
|---------|--------|
| `MenuItem::label(text)` | a row |
| `.icon(Icon::glyph(ch))` | leading glyph in the label font; `Icon::glyph_with_font(ch, family)` for another font (e.g. an icon font); `Icon::image(path)` for an image |
| `.shortcut(text)` | right-aligned hint text; display only, it doesn't bind a key |
| `.submenu(ContextMenu)` | the row opens a nested menu and shows a chevron (`›`) |
| `.destructive()` | red text and a red hover fill |
| `.disabled()` | dimmed; not hoverable, clickable or keyboard-focusable |
| `.on_select(fn)` | runs when the row is chosen; the menu then closes |
| `ContextMenu::separator()` | a hairline divider |

When no row in a menu has an icon, the icon column is dropped so labels hug the left padding.

### 2. Register the surfaces on the `App`

The menu needs its anchor window and one popup slot per depth level:

```rust
let mut app = App::new().window(context_menu::anchor);
for level in context_menu::LEVELS {
    app = app.popup(level);
}
```

### 3. Open it

| Call | Use |
|------|-----|
| `arm(name)` | the reliable way from outside the shell (see below); the next right-click opens `name` at the cursor |
| `show_under(name, menu, parent_ns, (x, y))` | register `menu` and open it hanging under another of the shell's surfaces, at a point in its coordinates: for a menu built fresh for what was clicked. The taskbar's per-window menu, a Bluetooth device's menu and the Do Not Disturb menu use it; a bottom-anchored parent makes the menu open upward |
| `open_under(name, parent_ns, x, y, w, h)` | the same for a menu already registered, anchored to a rect |
| `open_at(name, x, y)` / `open_named(name, x, y)` | open at a screen position (logical px) |
| `close()` | close every level |

`main.rs` exposes these over amane's IPC socket:

```sh
printf 'arm\n'            | nc -U -N $XDG_RUNTIME_DIR/amane.sock   # arm "file"
printf 'arm file\n'       | nc -U -N $XDG_RUNTIME_DIR/amane.sock
printf 'open file 240 180\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
printf 'close\n'          | nc -U -N $XDG_RUNTIME_DIR/amane.sock
```

The niri config binds `Mod+Shift+M` to `arm`.

## Why "arming"

`xdg_popup.grab()` needs the serial of a recent user input event on the client. A menu opened from
IPC has none, so the compositor would refuse the grab: no keyboard focus, no dismiss on an outside
click. Arming solves that:

1. `arm(name)` switches the anchor from a click-through 1×1 surface to a fullscreen transparent one
   (`Phase::Arming`).
2. The anchor tracks the pointer; the next **right-click** gives a fresh serial and opens the menu
   at the cursor. Any other click, or Escape, cancels.
3. The anchor shrinks back to 1×1 with an empty input region, so it never blocks other apps.

`open_under` doesn't need arming when it is called from a click on the parent surface, because
that click provides the serial.

## Behaviour

**Levels.** Level 0 is the root menu; each submenu is the next level, up to `MAX_DEPTH` (8). Each
level is its own popup with its own namespace (`amane.context-menu.0` … `.7`), parented to the
level above (level 0 to the anchor or to `open_under`'s parent).

**Placement.** Level 0 opens below-right of its anchor point and may slide or flip on both axes to
stay on screen. A submenu is anchored to its row's right edge, shifted by `menu.submenu.offset`,
and flips to the left side when there is no room.

**Pointer.**
- Hovering a row highlights it. Hovering a submenu row opens its submenu; hovering a plain row
  closes any deeper levels.
- Clicking a plain row runs `on_select` and closes the menu; clicking a submenu row opens it.
- Clicking outside every level closes the menu.

**Keyboard** (only the deepest open level receives keys):

| Key | Action |
|-----|--------|
| `Up` / `Down` | move between enabled rows, wrapping |
| `Right` | open the focused row's submenu and focus its first row |
| `Enter` | the same on a submenu row; otherwise run the row and close |
| `Left` | close the current level, back to its parent |
| `Escape` | close every level |

Keyboard focus draws the row with the accent (the selected style); pointer focus uses the hover
overlay. Destructive rows use the destructive hover fill for both. Keyboard navigation needs the parent surface to accept keyboard focus: the anchor uses
`Keyboard::OnDemand` for that reason.

## Styling

Every value is a design token from Penpot ([design-token-manual.md](design-token-manual.md)), read
through the generated functions (`sys::…`, `menu::…`). Menus follow light/dark mode, the system
accent, density and text scale while open.

| Part | Tokens |
|------|--------|
| Card | `sys.color.surface.popover.bg`, `.stroke`; `sys.radius.popover`; `sys.border.hairline`; `menu.card.padding` |
| Row | `menu.row.height`, `menu.row.radius`; horizontal padding `sys.space.inset.l` |
| Label, shortcut, chevron | `sys.type.label` |
| Text colours | `sys.color.surface.popover.fg`; disabled `…popover.fg-secondary`; destructive `sys.color.status.critical.text` |
| Hover fill | `sys.color.state.hover`; destructive `sys.color.status.critical.hover` |
| Keyboard selection | fill `sys.color.accent.bg`, text `sys.color.accent.fg` |
| Shortcut text | `…popover.fg-secondary`; on a keyboard-selected row `sys.color.accent.fg`, on any other focused row `…popover.fg`, both at `menu.shortcut.hover-opacity` |
| Icon and chevron slots | `menu.icon-slot.width`, `menu.chevron-slot.width`; gap after the icon `sys.space.gap.m` |
| Card | an outlined popover [`Card`](components.md#card), padding `menu.card.padding` |
| Separator | a [`Divider`](components.md#divider-and-slider): `sys.color.separator`, `sys.border.hairline`, margin `sys.space.gap.xs` |
| Submenu offset | `menu.submenu.offset` |
| Width limits | `menu.min-width`, `menu.max-width` |

## Sizing

A popup's size has to be known before it is drawn, so `size_for` measures it, with amane's
text measurement (`Text::width`):

- **Width:** the widest row, clamped to `menu.min-width` … `menu.max-width`. A row's width is:
  - the label, measured in `sys.type.label`;
  - the shortcut, measured the same way, plus `sys.space.gap.xl`;
  - the chevron slot plus `sys.space.gap.s`;
  - the icon column, when the menu has one;
  - row padding on both sides, then card padding on both sides.
- **Height:** card padding, plus `menu.row.height` per row, plus `Divider::height()` per
  separator (`entry_height`). The submenu anchor adds up the same heights (`row_offset`).

`menu.label.advance-ratio`, which the old estimate used, is no longer read: delete it in Penpot.

## Internals

All state lives in a thread-local `Runtime` (`runtime.rs`):

| Field | Meaning |
|-------|---------|
| `registry` | registered menus by name |
| `phase` | `Idle`, `Arming` or `Open` |
| `stack` | the open levels, outermost first; each holds its entries, focused row, whether focus came from pointer or keyboard, and its anchor rect |
| `pending` | the menu `arm` will open |
| `cursor` | the last pointer position on the armed anchor |
| `root_parent` | `open_under`'s parent namespace, if any |

The popup view functions in `LEVELS` render `stack[k]` for their depth and stay registered but
hidden when the stack is shallower. Every state change calls `request_frame()`.

## Limits

- **Depth:** 8 levels. To go deeper, raise `MAX_DEPTH` and add entries to `NS` and `LEVELS`.
- **Static menus:** a menu is copied from the registry when it opens. To change a menu's items
  (like the taskbar's per-window menu), register it again under the same name before opening.
- **Single-threaded:** callbacks are `Rc<dyn Fn()>` and the state is thread-local, so open and
  register from the UI thread (view functions, input handlers, IPC handlers).
- **Window menus from the bar** use `show_under` with the taskbar as parent. The taskbar is
  `Keyboard::None` so it never steals focus, which means keyboard navigation doesn't reach those
  menus; pointer use works.

## Related

- amane's popup support: [../../amane/docs/popups.md](../../amane/docs/popups.md)
- niri layer rules and namespaces: [layer-rules.md](layer-rules.md)
- The taskbar's per-window menu: [taskbar.md](taskbar.md)
