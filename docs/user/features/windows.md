# Windows, and niri's other settings

Emaki Settings' **Windows** page (`apps/settings/src/windows.rs`) sets how niri lays windows out,
how it marks the focused one and how you switch between them. `emaki-settings --page windows`
opens it (`layout` and `multitasking` open it too).

The same file, `settings.kdl`, also holds:
- Input's **Super and swipe** gesture, and **Show the shortcuts at login** on the Keyboard
  Shortcuts sub-page ([input.md](input.md));
- Appearance's **Pointer** group: the cursor theme and size, hide while typing, hide when still;
- Desktop's **Screenshots** group: the folder they're saved in, or only copying them.

## The page

- **Layout:**
  - Gaps between windows, 0–32 px.
  - New windows' width: a third, half, two thirds, or as the app asks.
  - Center the focused column: never, always, or when it doesn't fit.
  - Center a lone window.
  - An empty workspace above the first.
- **Windows:**
  - **Focus ring**, with its width and **Use the accent color**. While it follows the accent, the
    ring's colour is written again whenever the accent changes, from Appearance or the system.
  - Border, with its width.
  - Shadows.
  - The mat (the emaki fork's backing behind the columns).
  - **Ask apps to leave out their title bars** (`prefer-no-csd`). Apps take it as they start again.
- **Switching:**
  - The Alt+Tab window switcher (`recent-windows`).
  - Overview size, 25–75%.
  - **Hot corner**: none, or which corner opens the overview.
- **Reset Windows Settings** hands every setting on the page back to `config.kdl`.

## Where the settings go

The settings are niri's own, written to **`settings.kdl`** beside `config.kdl`. `config.kdl`
includes it as its last line:

```kdl
// Emaki Settings writes this file; it comes last, so its settings win:
include optional=true "/home/ocbk/nixos-config/niri/settings.kdl"
```

The file holds only what was changed in Settings, unlike `input.kdl` and `displays.kdl`, which own
their whole sections. That works because of how niri merges an included file:
- It merges `layout`, `cursor`, `gestures`, `overview`, `recent-windows` and `hotkey-overlay` field
  by field.
- A later file's field wins.
- `prefer-no-csd` and `screenshot-path` are single settings, and the last one wins.

So a setting written here wins over `config.kdl`'s, and removing it from the file (Reset) brings
`config.kdl`'s back. Nothing moves out of `config.kdl`. A page shows what's in force: the file's
value, else `config.kdl`'s, else niri's default.

A few of niri's rules shape what's written:
- **On and off are explicit.** A section's `off` in `config.kdl` holds unless a later file writes
  `on`, so the file writes `on` or `off`, and `true` or `false` for a flag.
- **The mat has no `on`.** A `mat {}` block without `off` turns it on, so on is an empty block.
- **Some blocks are taken whole.** `hot-corners`, `mod-swipe` and `default-column-width` are
  written in full each time.

Until `config.kdl` includes the file, the pages show `config.kdl`'s settings without changing
them, under **Set Up…**. Setting up does three things:
1. Writes an empty `settings.kdl`.
2. Puts the include at the end of `config.kdl`.
3. Commits both.

It refuses while `config.kdl` has uncommitted changes.

The settings that belong to niri's `input` section go to `input.kdl` instead, because that file
owns the whole section ([input.md](input.md#where-the-settings-go)). These are focus follows the
pointer, the pointer jumping to the focused window, left-handed, scroll speed, middle click and
the touchpad off with a mouse.

`settings.kdl` has the same guarantees as the other two files, with the shared code in
`niri_files.rs`:
- **One writer.** `niri_settings/kdl.rs` writes it, and a test holds every other file to that.
- **Whole writes.** It's written whole.
- **Hand edits survive.** Lines it doesn't know are kept.
- **Commits.** Each change is committed alone (`settings: gaps`).
- **Refused changes are put back.** If niri's config load fails within 5 s of a write, the file is
  put back and the page says so.

### The cursor

niri's `cursor` section draws the pointer and hands the theme and size to the apps it starts. GTK
apps read them from GNOME's settings, so a change also runs `gsettings set
org.gnome.desktop.interface cursor-theme|cursor-size`. Other apps that are already open take it
when they start again. The theme menu lists the icon themes with a `cursors` folder in the XDG data
folders, `~/.icons` and `~/.local/share/icons`.

### Screenshots

**Save in** sets `screenshot-path` to the chosen folder with niri's own file name pattern
(`Screenshot from %Y-%m-%d %H-%M-%S.png`). **Only copy screenshots** sets it to `null`, so niri
copies each one to the clipboard and saves no file.

## Not yet

- niri's animations, and linking them to Appearance → Motion.
- Window corner radius: it's a window rule, and one added last would override the shell's own
  windows' rules.
- Rules for one app at a time.
- Named workspaces.
- Struts, the preset column widths and the tab indicator.
- A focus ring colour of your own; it follows the accent or keeps `config.kdl`'s.
