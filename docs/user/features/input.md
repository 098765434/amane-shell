# Input

Emaki Settings' **Input** page (`apps/settings/src/input/`) sets how the keyboard repeats, how the
touchpad and mouse move and scroll, and lists niri's keyboard shortcuts.
`emaki-settings --page input` opens it (`keyboard`, `mouse` and `touchpad` open it too), and
`--page input/shortcuts` opens the shortcuts.

## The page

- **Keyboard:**
  - Delay before repeat: 150–1000 ms, niri's default 600.
  - Repeat rate: 10–60 a second, default 25.
  - A **Try it** field to hold a key in.
  - Num Lock on at login.
  - **Layout**, which opens General: the layout is a NixOS setting there (`keyboardLayout`), so
    niri reads it from `locale1` and this page doesn't set it.
- **Touchpad**, shown when udev knows one (`ID_INPUT_TOUCHPAD`, else a device named like one):
  - Tap to click, natural scrolling, and off while typing (`dwt`).
  - Speed: −1 to 1 on the slider, shown as "30% slower".
  - Acceleration: adaptive or flat.
  - Right click: the bottom right corner (`button-areas`) or two fingers (`clickfinger`).
  - Scroll with: two fingers or the edge.
  - Each dropdown also has **Default**, which leaves the setting out so libinput picks it.
- **Mouse:** natural scrolling, speed and acceleration. It's always shown, since a mouse can be
  plugged in at any time.
- **Keyboard Shortcuts**, a sub-page listing every bind in niri's config, with a Find field.
  - The groups are Shell, Apps, Windows and Columns, Workspaces, Monitors, Screenshots, Media and
    Hardware, and Session.
  - A bind's name is its `hotkey-overlay-title`. Without one, the name is what the bind does: the
    shell request it sends over the socket ("Shell: osd volume up"), the command it runs, or niri's
    action in words ("Focus column left").
  - Keys read as people say them: `Mod+Shift+Slash` shows as Super+Shift+/.

Switches and dropdowns write at once. A stepper or slider waits 0.4 s after the last change, so
a drag is one write and one commit.

## Where the settings go

The settings are niri's own `input` block, written to **`input.kdl`** beside niri's `config.kdl`,
which includes it:

```kdl
// Input is set in Emaki Settings, which writes this file:
include optional=true "/home/ocbk/nixos-config/niri/input.kdl"
```

The file owns the whole `input` block, not just the settings the page changes, because of how
niri merges an included file:
- It merges `keyboard` field by field.
- It takes a `touchpad` or `mouse` section whole, dropping the one before.

An included `touchpad { dwt }` would drop `tap` set in `config.kdl`. So until `config.kdl`'s own
block has moved, the page shows its settings without changing them, under **Move to
Settings…**. Moving it:
1. Comments the block out.
2. Puts the include in its place.
3. Writes `input.kdl` with the same settings.
4. Commits both files together.

It refuses while `config.kdl` has uncommitted changes, as the Displays page does.

`input.kdl` has the same guarantees as `displays.kdl` ([displays.md](displays.md#where-the-settings-go)),
with the shared code in `apps/settings/src/niri_files.rs`:
- **One writer.** Settings is the only program that writes it (`input/kdl.rs`); a test fails if
  any other file names it.
- **Whole writes.** The file is written whole through a rename.
- **Hand edits survive.** Lines the writer doesn't know, at any depth, are kept: `xkb`,
  `trackpoint`, `focus-follows-mouse`, comments. A known setting with a comment of its own is kept
  as it was.
- **Commits.** Each change is committed alone (`input: touchpad.tap`), never pushed.

niri reloads the file by itself. The page follows niri's config loads (`ConfigLoaded` on the
event stream), so it shows a change made anywhere. If a load fails within 5 s of a write of its
own, it puts the file back as it was and says niri didn't take the change.

## The shortcuts

niri's IPC doesn't report binds, so `input/binds.rs` reads the `binds` blocks:
- **Which files.** `config.kdl` first, then each file it includes, in order. A key bound again
  later replaces the earlier bind, as in niri.
- **Comments.** Commented (`//`) and slashdashed (`/-`) binds are left out.
- **Quoting.** Strings are read with their escapes, so the long `printf … | nc -U` binds come out
  whole.

The list is read-only: shortcuts are changed in `config.kdl`.

## Pointer and gestures

- **Pointer** group, in `input.kdl`:
  - focus follows the pointer (`focus-follows-mouse`);
  - the pointer jumps to a window focused from the keyboard (`warp-mouse-to-focus`);
  - left-handed, for the mouse and the touchpad together.
- **Touchpad** and **Mouse** groups:
  - scroll speed, a quarter to twice (`scroll-factor`);
  - middle click by pressing both buttons (mouse);
  - off while a mouse is connected (touchpad).
- **Gestures**, shown with a touchpad: **Super and swipe moves windows** (`mod-swipe`, the emaki
  fork's). It's niri's `gestures` section, so it's kept in `settings.kdl`
  ([windows.md](windows.md#where-the-settings-go)).
- On the **Keyboard Shortcuts** sub-page, **Show the shortcuts at login**
  (`hotkey-overlay skip-at-startup`), also in `settings.kdl`.

## Not yet

- Editing shortcuts.
- Gestures beyond Super and swipe ([backlog](../../developer/backlog.md) #15).
- Trackpoint, tablet and touchscreen settings, which are kept as they are.
- Sticky and slow keys.
- A per-device setting: niri sets all touchpads alike and all mice alike.
