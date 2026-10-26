# Launcher

macOS Tahoe's Spotlight (`src/components/launcher/`): a glass search pill high on the screen,
with round buttons beside it for its modes, that grows down into a panel as you type. Its
Applications mode is the Launchpad grid the launcher started as, after KDE's
[Tahoe Launcher](https://github.com/EliverLara/TahoeLauncher). It's opened over IPC from a
keybind. Its namespace is `amane.launcher`, and its surface covers the output, clear outside the
glass, so a click anywhere but the glass closes it.

## What it shows

- **The pill**, a fifth of the way down the screen: a magnifier, the search field
  ("Spotlight Search") and, while there's a query, a button that clears it.
  - The text, the caret and the selection are drawn by the launcher itself, so every key
    reaches it (see [Typing](#typing)).
- **Four round buttons** beside the pill, for the modes: Applications, Files, Actions and
  Clipboard.
  - Under the pointer, a button lifts, and the empty pill says the mode's name with the keys
    that pick it (`Ctrl` `1` … `4`).
  - On a screen too narrow for them beside the pill, they sit under it.
- **The panel.** Typing, or picking a mode, grows the pill down into a panel. The buttons
  slide in under it and fade. One spring carries the glass and its frost between the two.

### Searching

From the pill, a query lists ([`provider.rs`](../../src/components/launcher/provider.rs)):

1. **An answer**, in an accent card over the list. Enter copies it.
   - A sum: `+ - * / ( )` over numbers.
   - A unit conversion (`answers.rs`): length, mass, volume, time, data and temperature, like
     `5 km in mi`, `70f to c` or `1.5 GiB to MB`.
     - Byte units keep their case (`Mb` isn't `MB`) unless typed all in lower case.
- **Chips**: up to four places to narrow the search to, from the best rows. For now that's an
  app with desktop actions. A click narrows to it, as Tab does on its row.
- **The rows**, ranked (`rank.rs`):
  - **Apps**, by a fuzzy match on the name. The tiers are the whole name, its start, a word's
    start, the words' initials ("vsc"), anywhere in it, then its letters in order (three or
    more of them). The start of a `Keywords` entry also counts.
  - **Actions** (see [Actions](#actions)), by name or another word for them ("reboot",
    "caffeine"), a little under an app with the same match.
    - An action's **quick keys** typed exactly pin it to the top.
    - An action that asks for something can have it typed after its name: `timer 5m`,
      `keep awake for 2h`, `set power mode power saver`.
  - Each match is lifted by how much it's been used (a score that halves each week) and by
    how often it was picked for the same first letters before. So "f" learns Firefox.
- **Files**, from three letters typed: the five whose names match best, under the apps and
  actions (see [Files](#files)).
- **Open the URL**, for a query that reads as one, then **a web search**, always last.

Each row has its icon (with a badge on its corner for things that come from somewhere), its
title, a line under it, and a note at its end. **Holding Super** shows each row's fuller
line in place of the one under it:
- an app's desktop file and command;
- the URL a row opens.

Super only counts once it's been let go after opening, so the Mod of Mod+Space doesn't set it
off.

### Narrowing to an app

Tab on an app with desktop actions, or a click on its chip, narrows the search to it.
- Its name shows as a token in the field.
- The list is "Open *App*" and its `[Desktop Action]`s, like Firefox's "New Private Window",
  filtered by what's typed.
- Backspace in the empty field, Escape, or the back button leaves it.

### Modes

A mode shows its glyph where the magnifier was and its name in the empty field. Under the
pointer, the glyph turns into a back button (a chevron on a soft square), back to the pill.

- **Applications** (`Ctrl+1`): the category pills over the apps, five across and three rows in
  view, in a panel as wide as the grid.
  - All Applications leads with the **suggested** row: the five apps used most from the
    launcher, filled out with the first of the rest, with a hairline under them. It's left
    out until an app has been used from the launcher.
  - The ⋯ button at the end of the row shows the apps as a **Grid** or a **List**
    (`apps_layout` in `~/.config/amane-shell/launcher.json`).
  - Typing searches the apps only.
- **Actions** (`Ctrl+3`): every action, a section at a time (see [Actions](#actions)), in a
  panel of one size. Typing searches the actions only.
- **Clipboard** (`Ctrl+4`): what's been copied, newest first (see
  [Clipboard](#clipboard)), in a panel as wide as Applications'.
- **Files** (`Ctrl+2`): files by name, and by what's in them with LocalSearch (see
  [Files](#files)), beside a preview of the one picked.

## Actions

What the launcher does without opening an app (`actions.rs`), Spotlight's Actions:

| Section | Actions |
|---|---|
| Control Center | each of Quick Settings' toggles and buttons this machine has, as it is now ("Turn Night Light On" while it's off, Screenshot), Set Power Mode (a choice of the modes the daemon offers), Keep Awake For (how long) |
| System | Lock Screen, Log Out, Suspend, Restart, Power Off, each when logind allows it |
| Clock | Start Timer (how long), and Cancel Timer while one runs |
| *an app* | each of its `[Desktop Action]`s, like Firefox's "New Private Window" |

Unsearched, Actions lists them a section at a time under bold headings, the five used most
first under **Suggestions**.

**Asking first.** An action that needs something (how long, which mode) narrows the search to
it on Enter:
- Its name shows as a token, and the field says what to type ("For how long? 5m, 1h30m, 1:30").
- A choice lists its options.
- A duration reads as `5m`, `5 min`, `1h30m`, `90s`, `2 hours`, `1:30`, or a bare number of
  minutes; the row says what it will do ("Start Timer · 5 min").
- Typing it after the name from the pill skips the step: `timer 5m`.

**Quick keys.** In Actions each row ends with **Add Quick Keys**, or the keys it has.
- A click on it, or Ctrl+K on the row picked, starts typing them: one to four letters or
  digits, Return to save (none clears them), Escape to give up.
- Keys another action has are refused, saying which.
- Typed in the launcher, an action's quick keys put it first, its keys shown at its end.
- They're kept in `launcher.json` (`quick_keys`), with the action's name. Shell Settings lists
  them and can remove them or reset them all.

**The timer** (`services/timer.rs`): one at a time; a new one replaces it. While it runs the
bar shows a ⏱ chip counting down by the second, which opens Actions. When it runs out, a
notification says so.

## Files

Spotlight's Files (`services/file_search.rs`). A search runs off the UI thread a moment after
the last key, the newest replacing any before it.

- **Where it looks.**
  - With GNOME's LocalSearch running, its index, through `tinysparql`: names, and what's in
    the files (full-text). Newest first.
  - Without it, file names under the home folder, through `fd` (which leaves out hidden
    files and what `.gitignore`s ignore).
- **Before anything's typed**, the files changed lately, under **Recents**: the index's
  newest, or `fd`'s files changed in the last week.
- **Chips** narrow it to a kind: PDF, Images, Documents, Folders. Typing `/` lists every kind
  (Spreadsheets, Presentations, Music, Movies, Archives too) to pick from.
- **A folder**: Tab narrows the search to what's in it, its name as the token.
- **Each row** is the file's kind glyph (an image itself, small), its name, what it is, how
  big and the folder it's in, and how long ago it changed. Super shows its whole path.
- **The preview** beside the list: an image (PNG, JPEG, SVG) fitted, the start of a text, a
  folder's files, or the name of anything else.
- **Return** opens it in its app (`xdg-open`); **Ctrl+Return** shows it in the file manager
  (`org.freedesktop.FileManager1`).
- **Right-click**: Open, **Open With ›** (the apps whose desktop entries open its kind), and
  Show in Files.

To search inside files, turn LocalSearch on in NixOS:

```nix
services.gnome.localsearch.enable = true;
services.gnome.tinysparql.enable = true;
```

It indexes the home folder by default (dconf `org.freedesktop.Tracker3.Miner.Files`
`index-recursive-directories`).

## Clipboard

Spotlight's clipboard history (`services/clipboard_history.rs`). It's **off until asked for**:
the first time, Clipboard explains what it would keep, with **Not Now** (back to the pill) and
**Enable**.

Once on:
- **What's kept.** Each new copy, in its best kind: a PNG or JPEG image, a list of files, or
  text.
  - Up to 200, newest first. A copy made again moves to the top.
  - Copies over 4 MB aren't kept, and all of them together stay under 64 MB.
  - A copy a password manager marks as one (`x-kde-passwordManagerHint`) isn't kept, nor one
    made while an app it's told to leave out has the focus (KeePassXC, Bitwarden and 1Password
    by default).
  - They're kept in `$XDG_STATE_HOME/amane-shell/clipboard/`: `index.json` and a file a copy.
- **The list.** Each row is its start, or the image small, what it is and how big, the app it
  was copied in, and how long ago. Typing narrows it to the copies with that in them.
- **The preview.** The copy picked shows beside the list: its text wrapped, or its image
  fitted.
- **Return** pastes it: puts it back on the clipboard (`wl-copy`), closes the launcher, and
  once the window under it has the keyboard again types Ctrl+V into it (`wtype`; Ctrl+Shift+V
  in a terminal).
- **Shift+Return** only puts it back on the clipboard.
- **Delete** forgets it.
- The **⋯** menu clears the history, or turns it off.

It watches the clipboard through wl-clipboard (`wl-paste --watch`), which follows the
compositor's selection without needing the keyboard. It starts and stops as the setting
changes (`clipboard.enabled` in `launcher.json`, or Shell Settings' Spotlight page).

To open it from a key, bind `launcher toggle clipboard`, in place of another history's picker:

```kdl
Mod+Ctrl+V hotkey-overlay-title="Clipboard" {
    spawn "sh" "-c" "printf 'launcher\\ntoggle\\nclipboard\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
}
```

## Using it

| Do | What happens |
|---|---|
| `Mod+Space` (see [IPC](#ipc)) | Opens the pill, closing Quick Settings or the notification center; again closes it |
| Type | Searches; the pill grows into a panel listing what's found |
| `Ctrl+1` … `4`, or a round button | Applications, Files, Actions, Clipboard; the mode showing again goes back to the pill |
| Up / Down (empty field) | The searches run before, newest first, and back |
| Down (search field) | Into the list, or to the pills in Applications |
| Up / Down (list) | Move the highlight a row; Up from the top goes back to the field |
| Arrows (apps) | Move the highlight a cell, or a row; Up from the top row goes to the pills |
| Left / Right (pills) | The previous or next category, sliding there |
| Tab | Narrows to the row picked when it narrows to anything; else moves the focus (search field → pills → apps) |
| Ctrl+K (an action) | Starts typing its quick keys |
| Shift+Return / Delete (a copy) | Puts it back on the clipboard without pasting / forgets it |
| Ctrl+Return (a file) | Shows it in the file manager |
| `/` (Files) | Lists the kinds of file to narrow to |
| Shift+Tab | Moves the focus back |
| Enter | Runs the highlighted row or app, else the first |
| Escape | Clears the query, then leaves the scope, then the mode; then closes the launcher |
| Hold Super | Each row's fuller line |
| Two fingers up / down | Scroll the grid or the list, coasting on after a flick |
| Two fingers sideways (apps) | Slide to the previous or next category, following the fingers |
| Wheel | Scrolls the grid or the list; a sideways tilt steps the category |
| Hover / click | Highlights / runs an app or row |
| Right-click an app | Its menu (see [App menu](#app-menu)) |
| Click outside the glass | Closes the launcher |

Running anything closes the launcher and counts it as used. The next time it opens, it starts
afresh on the pill.

### Focus

The keys go to one of three places at a time:
- **The search field**: its caret shows.
- **The pills** (Applications): a ring goes round the picked one.
- **The apps or rows**: the highlight shows at full strength; elsewhere it's dimmed.

Typing always goes to the search, wherever the focus is. While listing, Left and Right move the
caret and Up and Down move through the rows, whichever has the focus.

### Gestures

Two fingers on the grid either scroll it or turn the category: the first few pixels decide which
(`AxisLock`), and the gesture keeps to it until the fingers lift.
- **Up and down**, the grid follows them one to one, pulling on a rubber band past the ends. When
  they lift, it coasts as far as their speed carries it and slows to a stop, or springs back from
  past an end (`Kinetic`). Fingers that rested before lifting leave no momentum.
- **Sideways**, the next category's apps slide in beside the current ones. When the fingers lift
  it settles on the nearer category, or the next one on a flick (`Pager`). A pill clicked next
  door slides the same way; one further away fades in instead.

The compositor only reports the fingers moving and lifting, so the momentum is the launcher's
(`amane_kit::kinetic`, `kit/src/kinetic.rs`), not amane's.

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
`modifiers()`, which reports the modifier keys and delivers Ctrl+letter as the letter. amane
redraws the focused surface when a modifier goes down or up, which is what lets Super show the
fuller lines.

## What it remembers

- **Use**, in `$XDG_STATE_HOME/amane-shell/launcher-usage.json` (`usage.rs`), written when
  something runs. It holds:
  - how much each result was used, by a key like `app:firefox.desktop` (amane's
    `DesktopApp::id()`);
  - what was picked for a query's first eight letters;
  - the last 50 searches run.

  Kept to 500 results and 2000 query starts, the least used going first.
- **Settings**, in `~/.config/amane-shell/launcher.json` (`amane_kit::settings::Launcher`),
  followed live:
  - `apps_layout`: `grid` or `list`.
  - `excluded_providers`: the sources the search leaves out, by id: `answers`, `apps`,
    `actions`, `files`, `clipboard`, `url`, `web`.
  - `quick_keys`: each action's quick keys, with its key and name.
  - `clipboard`: whether the history is on, how many copies it keeps, whether images too,
    and the apps it leaves out.

  Shell Settings' **Spotlight** page changes them.

## IPC

`launcher [toggle | open | close] [apps | files | actions | clipboard]`, one argument per line on
`$XDG_RUNTIME_DIR/amane.sock`:
- With a mode, `open` opens the launcher in it.
- With a mode, `toggle` closes the launcher when that mode is showing, and otherwise switches
  to it.

```sh
printf 'launcher\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
printf 'launcher\nopen\napps\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
```

As niri keybinds:

```kdl
binds {
    Mod+Space hotkey-overlay-title="Spotlight" {
        spawn "sh" "-c" "printf 'launcher\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
    Mod+A hotkey-overlay-title="Applications" {
        spawn "sh" "-c" "printf 'launcher\\ntoggle\\napps\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
}
```

## Glass

The pill, the panel and the round buttons are the same glass as Quick Settings and the dock:
- The surface is clear, and tells the compositor to frost behind the sheet
  (`Material::Sheet`) and each button (`Material::Control`) (`shape::Geometry::frost`).
- The geometry blends, so the frost follows the spring as the pill grows and shrinks, and the
  buttons' frost fades with them.
- Opening, the glass swells in about the pill's centre, the frost with it.
- Where the compositor has no glass material of its own, the sheet and buttons draw the
  shader's lit edge (`Material::lit`).

The panel's content is laid out for the size the sheet is heading to, and clipped to the size it
is, so growing reveals the rows rather than squeezing them.

The `amane.launcher` layer rule in `niri-layer-rule.kdl` sets `xray false`, so the frost blurs
the windows under it rather than the wallpaper, and frosts its popups, so an app's menu is a
frosted card. Install it with `./gen-layer-rules.sh` (see
[layer-rules.md](../integration/layer-rules.md)).

The surface covers the output (clear of what the bar reserves) and keeps that size, open or
closing, so it is never resized while it moves. Outside the glass the surface takes clicks,
which close the launcher; the glass keeps its own. While closed the surface is hidden, so it
never blocks anything.

## How it's built

| File | What |
|---|---|
| `mod.rs` | the surface, its state, what it shows (`View`, `Kind`, `Layout`), IPC, focus, keys and gestures |
| `shape.rs` | where the glass is: the pill, the panel and the buttons (`Geometry`, which blends), and its frost (pure, tested) |
| `mode.rs` | the modes, their glyphs, names and keys (pure, tested) |
| `provider.rs` | the sources a search draws on (`Provider`) and how their finds are arranged (`search`; tested) |
| `services/file_search.rs` | file searches through LocalSearch or `fd`, the kinds, and the SPARQL (tested) |
| `actions.rs` | the actions (`Spec`), what they ask for, and an argument typed after a name (tested) |
| `rank.rs` | the fuzzy match and the frecency score (pure, tested) |
| `usage.rs` | what's remembered of use, and the searches run (tested) |
| `answers.rs` | unit conversions (pure, tested) |
| `entries.rs` | a row (`Entry`), what narrows a search (`Scope`), and what each row does |
| `header.rs` | the search row, the round mode buttons, the ⋯ menu, and the pill bar with its sliding fill |
| `grid.rs` | the category pages, each a grid with its highlight, the suggested row on the first |
| `results.rs` | the answer card, the chips, the rows with their section headings and quick keys, and the empty states |
| `app_menu.rs` | an app's and a file's right-click menus |
| `model.rs` | categories, sums, URLs, session-action names, the history's steps, durations and quick keys (pure, tested) |
| `components/text_edit.rs` | the search text, caret and selection (pure, tested; shared with Quick Settings' Customize window) |

It's built on the kit's [`ScrollView`, `Kinetic`, `AxisLock`, `Pager` and
`app_icon`](../kit/components.md), and the shell's
[`workspace_menu`](dock.md#workspace_menu) and
[`text_edit`](quick-settings.md#searchfield-and-text_edit).

Sizes particular to the launcher are `TODO(token)` constants until Penpot has a `launcher.*` set
(see [penpot-backlog.md](../design-system/penpot-backlog.md)). Everything else reads `sys.*`, so the launcher follows the
light and dark themes and the density.

## Known limits

- The clipboard history needs wl-clipboard (`wl-paste`, `wl-copy`), and pasting needs `wtype`
  and niri's IPC. Images other than PNG and JPEG are kept only as the text or files offered
  with them, since amane can't show them.
- Quick Settings' toggles act through the same callbacks as their tiles, so one that only
  opens its page (Power Off…, Display, Sound) isn't an action.
- The grid scrolls by a negative top padding inside a clipped viewport (`ScrollView`), as amane's
  `ScrollArea` does, so hit areas follow what's drawn. The pill bar scrolls the same way, sideways.
- amane's `Key` has no Menu or F10 key, so an app's menu only opens from a right-click.
- The app menu's keyboard navigation relies on niri giving the popup grab the keyboard while the
  launcher holds it exclusively.
- An app's name is one line, elided. amane's `Text` can't centre each line of wrapped text, so
  names don't wrap onto two centred lines.
- The web search goes to Google (`model::SEARCH_URL`).
- If niri ever binds a lone Super press, Super won't reach the launcher to show the fuller lines.
