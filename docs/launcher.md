# Launcher

An app launcher after macOS Launchpad, by way of KDE's
[Tahoe Launcher](https://github.com/EliverLara/TahoeLauncher) (`src/components/launcher/`). It is a
glass panel in the middle of the screen, opened over IPC from a keybind. Its namespace is
`amane.launcher`.

## What it shows

- **A search field** with the launcher's mark beside it. The text, the caret and the selection are
  drawn by the launcher itself, so every key reaches it (see [Typing](#typing)).
- **Category pills**: All Applications, Chrome Apps, Development, Education, Games, Graphics, Help,
  Internet, Multimedia, Office and Utilities.
  - Only pills with at least one app under them show.
  - Picking a pill narrows the grid to it.
  - When the pills don't fit, the bar scrolls sideways.
- **A grid of the installed apps** (amane's `Apps`), five across and four rows in view.
  - Each cell is the app's icon (or its first letter while none resolves) over its name.
  - One highlight glides to the app under the pointer or the arrow keys.
  - The grid scrolls smoothly, with a scrollbar when there are more rows than fit.
- **Search results** in the grid's place while there's a query (see [Search](#search)).
- **An empty state**: "Loading…" while apps are first read, and "Nothing in this category" for an
  empty pill.

## Using it

| Do | What happens |
|---|---|
| `Mod+Space` (see [IPC](#ipc)) | Opens it, closing Quick Settings or the notification center; again closes it |
| Type | Searches; the grid turns into the results list |
| Arrows (grid) | Move the highlight a cell, or a row up and down |
| Up / Down (list) | Move the highlight a row |
| Tab | The next app or row, wrapping |
| Enter | Runs the highlighted app or row, else the first |
| Escape | Clears the search; with none, closes the launcher |
| Wheel / two fingers | Scroll the grid or the list; over the pills, scroll them sideways |
| Hover / click | Highlights / runs an app or row |

Running anything closes the launcher. When it's next opened it starts afresh: no query, All
Applications, at the top.

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

The `amane.launcher` layer rule in `niri-layer-rule.kdl` only sets `xray false`, so the frost
blurs the windows under it rather than the wallpaper. Install it with `./gen-layer-rules.sh` (see
[layer-rules.md](layer-rules.md)).

The surface is one size, open or closing, and is never resized while it moves.

## How it's built

| File | What |
|---|---|
| `mod.rs` | the surface, its state, IPC, keys and scrolling |
| `header.rs` | the search field and the pill bar |
| `grid.rs` | the app grid, its highlight and scrollbar |
| `results.rs` | the results list and the empty state |
| `entries.rs` | what a search finds and what each entry does |
| `model.rs` | categories, sums, URLs and session-action names (pure, tested) |
| `edit.rs` | the search text, caret and selection (pure, tested) |

Sizes particular to the launcher are `TODO(token)` constants until Penpot has a `launcher.*` set
(see [penpot-todo.md](penpot-todo.md)). Everything else reads `sys.*`, so the launcher follows the
light and dark themes and the density.

## Known limits

- The grid scrolls by a negative top padding inside a clipped viewport, as amane's `ScrollArea`
  does, so hit areas follow what's drawn. The pill bar scrolls the same way, sideways.
- An app's name is one line, elided. amane's `Text` can't centre each line of wrapped text, so
  names don't wrap onto two centred lines.
- The web search goes to Google (`model::SEARCH_URL`).
