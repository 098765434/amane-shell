# Launcher

An app launcher after macOS Launchpad, by way of KDE's
[Tahoe Launcher](https://github.com/EliverLara/TahoeLauncher) (`src/components/launcher/`). It is a
glass panel in the middle of the screen, opened over IPC from a keybind. Its namespace is
`amane.launcher`. Its surface covers the output, clear outside the panel, so a click anywhere but
the panel closes it.

## What it shows

- **A search field** with the launcher's mark beside it, and a clear (×) button while there's a
  query. The text, the caret and the selection are drawn by the launcher itself, so every key
  reaches it (see [Typing](#typing)).
- **Category pills**: All Applications, Chrome Apps, Development, Education, Games, Graphics, Help,
  Internet, Multimedia, Office and Utilities.
  - Only pills with at least one app under them show.
  - Picking a pill narrows the grid to it. The picked pill's accent fill slides from pill to pill,
    following the fingers while they swipe between categories.
  - When the pills don't fit, the bar scrolls sideways, and keeps the picked pill in view.
- **A grid of the installed apps** (amane's `Apps`), five across and four rows in view.
  - Each cell is the app's icon (or its first letter while none resolves) over its name.
  - One highlight glides to the app under the pointer or the arrow keys.
  - The grid follows two fingers one to one and coasts on after a flick, with a scrollbar that
    shows while it moves.
  - Each category is a page: swiping sideways slides the next category's apps in.
- **Search results** in the grid's place while there's a query (see [Search](#search)).
- **An empty state**, a glyph over "Loading…" while apps are first read, or over "Nothing in this
  category" for an empty pill.

## Using it

| Do | What happens |
|---|---|
| `Mod+Space` (see [IPC](#ipc)) | Opens it, closing Quick Settings or the notification center; again closes it |
| Type | Searches from wherever the focus is; the grid turns into the results list |
| Tab / Shift+Tab | Moves the focus: search field → pills → apps (search field ↔ results while searching) |
| Arrows (apps) | Move the highlight a cell, or a row up and down; Up from the top row goes to the pills |
| Left / Right (pills) | The previous or next category, sliding there; Down or Enter goes to the apps |
| Down (search field) | To the pills (to the results while searching) |
| Up / Down (results) | Move the highlight a row |
| Enter | Runs the highlighted app or row, else the first |
| Escape | Clears the search; with none, closes the launcher |
| Two fingers up / down | Scroll the grid or the list, coasting on after a flick |
| Two fingers sideways | Slide to the previous or next category, following the fingers |
| Wheel | Scrolls the grid or the list; a sideways tilt steps the category |
| Over the pills | A swipe or the wheel scrolls them sideways |
| Hover / click | Highlights / runs an app or row |
| Right-click an app | Its menu (see [App menu](#app-menu)) |
| Click outside the panel | Closes the launcher |

Running anything closes the launcher. When it's next opened it starts afresh: no query, All
Applications, at the top, the focus on the apps.

### Focus

The keys go to one of three places at a time, and Tab moves between them:
- **The search field**: its caret shows.
- **The pills**: a ring goes round the picked one.
- **The apps**: the highlight shows at full strength; elsewhere it's dimmed.

Typing always goes to the search, wherever the focus is. Hovering an app brings the focus to the
apps. While searching, Up and Down move through the results and Left and Right move the caret,
whichever of the two has the focus.

### Gestures

Two fingers on the grid either scroll it or turn the category: the first few pixels decide which
(`AxisLock`), and the gesture keeps to it until the fingers lift.
- **Up and down**, the grid follows them one to one, pulling on a rubber band past the ends. When
  they lift, it coasts as far as their speed carries it and slows to a stop, or springs back from
  past an end (`Kinetic`). Fingers that rested before lifting leave no momentum.
- **Sideways**, the next category's apps slide in beside the current ones. When the fingers lift
  it settles on the nearer category, or the next one on a flick (`Pager`). A
  pill clicked next door slides the same way; one further away fades in instead.

The compositor only reports the fingers moving and lifting, so the momentum is the launcher's
(`components/kinetic.rs`), not amane's.

## App menu

Right-clicking an app in the grid or the results opens its menu at the pointer (`app_menu.rs`, on
the shared [context menu](context-menu.md)):
- **Open**.
- **Open in Workspace ›**, the workspaces of the monitor in focus, the empty one niri keeps after
  the last called "New Workspace". Picking one focuses it and then starts the app, both through
  amane's one worker thread, so niri opens the window there.
- The **actions** the app's desktop entry lists (`[Desktop Action …]`, like Firefox's "New Private
  Window"), after a separator, when it has any. amane reads them (`DesktopApp::actions()`).

### Typing

Typing works like a one-line text field:
- Left, Right, Home and End move the caret. Shift with any of them selects.
- Backspace deletes the selection, or the character before the caret.
- Ctrl+A selects all, Ctrl+C copies, Ctrl+X cuts and Ctrl+V pastes. These go through `wl-copy` and
  `wl-paste` from wl-clipboard.

Other chords (Alt, Super) are swallowed rather than typed. All of this needs amane's
`modifiers()`, which reports the modifier keys and delivers Ctrl+letter as the letter.

## Search

A query lists, in this order (`entries.rs`):

1. **A sum's answer**, for a query with `+ - * / ( )` that parses as arithmetic. Enter copies the
   answer.
2. **Matching apps**: by name, or by the start of one of their desktop entry's `Keywords`.
3. **Session actions**: Lock Screen, Log Out, Suspend, Restart and Power Off.
   - Each is found by its name or another word for it ("reboot", "shut down", "sleep").
   - Each shows only when logind allows it, and runs through the `Session` service as the power
     menu does.
4. **Open the URL**, for a query that reads as one (a scheme, or a host with a top-level domain).
5. **A web search**, always last, so the list is never empty.

Opening a URL or a search uses `xdg-open`, then focuses the default browser.
- niri focuses a new browser window on its own.
- A browser that's already open only gets a new tab, so the launcher looks up its window
  (`xdg-settings get default-web-browser`, then `niri msg --json windows`) and focuses it.

## Categories

The pills come from each desktop entry's `Categories` (amane's `DesktopApp::categories()`):

| Pill | Categories |
|---|---|
| Development | `Development` |
| Education | `Education` |
| Games | `Game` |
| Graphics | `Graphics` |
| Internet | `Network` |
| Multimedia | `AudioVideo`, `Audio`, `Video` |
| Office | `Office` |
| Utilities | `Utility`, `System`, `Settings` |
| Help | any category with `Help` in it |
| Chrome Apps | a command line with `--app=` (a Chromium web app) |

Help and Chrome Apps have no freedesktop category of their own, so they're guesses and may stay
empty.

## IPC

`launcher [toggle | open | close]`, one argument per line on `$XDG_RUNTIME_DIR/amane.sock`:

```sh
printf 'launcher\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
```

As a niri keybind:

```kdl
binds {
    Mod+Space hotkey-overlay-title="App Launcher" {
        spawn "sh" "-c" "printf 'launcher\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
}
```

## Glass

The panel is the same glass as Quick Settings and the dock:
- The surface is clear, and tells the compositor to frost behind the panel (`glass_region::module`).
- The frost swells and fades in with the panel.
- Where the compositor has no glass material of its own, the panel draws the shader's lit edge
  (`glass_region::lit`).

The `amane.launcher` layer rule in `niri-layer-rule.kdl` sets `xray false`, so the frost blurs
the windows under it rather than the wallpaper, and frosts its popups, so an app's menu is a
frosted card. Install it with `./gen-layer-rules.sh` (see
[layer-rules.md](layer-rules.md)).

The surface covers the output (clear of what the bar reserves) and keeps that size, open or
closing, so it is never resized while it moves. The panel is drawn in its middle, and the frost
is placed under the panel from the surface's size (`amane::window_size`). Outside the panel the
surface takes clicks, which close the launcher; the panel keeps its own. While closed the
surface is hidden, so it never blocks anything.

## How it's built

| File | What |
|---|---|
| `mod.rs` | the surface, its state, what it shows (`View`, `Layout`), IPC, focus, keys and gestures |
| `header.rs` | the search field and the pill bar with its sliding fill |
| `grid.rs` | the category pages, each a grid with its highlight |
| `results.rs` | the results list and the empty states |
| `app_menu.rs` | an app's right-click menu |
| `entries.rs` | what a search finds and what each entry does |
| `model.rs` | categories, sums, URLs and session-action names (pure, tested) |
| `edit.rs` | the search text, caret and selection (pure, tested) |

It's built on the shared [`ScrollView`, `Kinetic`, `AxisLock`, `Pager`, `workspace_menu` and
`app_icon`](components.md).

Sizes particular to the launcher are `TODO(token)` constants until Penpot has a `launcher.*` set
(see [penpot-todo.md](penpot-todo.md)). Everything else reads `sys.*`, so the launcher follows the
light and dark themes and the density.

## Known limits

- The grid scrolls by a negative top padding inside a clipped viewport (`ScrollView`), as amane's
  `ScrollArea` does, so hit areas follow what's drawn. The pill bar scrolls the same way, sideways.
- amane's `Key` has no Menu or F10 key, so an app's menu only opens from a right-click.
- The app menu's keyboard navigation relies on niri giving the popup grab the keyboard while the
  launcher holds it exclusively.
- An app's name is one line, elided. amane's `Text` can't centre each line of wrapped text, so
  names don't wrap onto two centred lines.
- The web search goes to Google (`model::SEARCH_URL`).
