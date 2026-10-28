# Shell Settings

`amane-settings` (`apps/settings/`) is the shell's settings app, built on the kit's
[app scaffolding](../kit/app.md). Each control changes one of the shell's settings files, which the
running shell follows at once; Shell Settings' own window redraws in the new look too. Editing a
file by hand works the same way.

Only one copy runs: launching it again brings the open window forward, and `--page <name>`
(`general`, `apps`, `services`, `power`, `storage`, `history`, `update`, `about`; or a Shell
page's name, like `displays`) opens that page, cold or in the running copy (`App::on_activate`): Spotlight's
system actions and the update toast use it. `--package <attr>` opens Apps with that nixpkgs
package's details. The flake installs it with a `.desktop` entry, so
the launcher lists it as *Shell Settings*.

## The window

An `app_window` titled "Shell Settings", 760 × 760 (at least 600 × 740), with a 200 px
[`sidebar`](../kit/app.md#sidebar) in two sections, **Shell** (eight pages, the shell's own
settings and the displays) and **System** (eight, the NixOS configuration), and the page's title over its settings,
each a caption over its control. Each page scrolls in an amane `ScrollArea` that keeps its own
offset. Ctrl+W closes it (`standard_shortcut`), unless a system build is running: then it asks
first ("Stop the build?"), as does the compositor's close.

Its content is drawn on the opaque window background; only the sidebar shows the frost behind the
window. That frost, and the frosted popups (the dropdowns' menus), come from the `window-rule`
the generated config has for `amane-settings` ([layer-rules.md](../integration/layer-rules.md)).
Without it the sidebar is an opaque panel. A niri window rule can still round the window like
other apps:

```kdl
window-rule {
    match app-id="^amane-settings$"
    geometry-corner-radius 12
    clip-to-geometry true
}
```

## Pages

### Appearance

Writes `appearance.json` (`apps/settings/src/appearance.rs`).

| Setting | Control | Choices | Field |
|---|---|---|---|
| Appearance | radios | Follow the system, Light, Dark | `mode`: `null`, `"light"`, `"dark"` |
| Accent colour | dropdown | System, Blue, Purple, Pink, Red, Orange, Yellow, Green, Graphite (macOS 27's colours) | `accent`: `null` or `#rrggbb` |
| Density | radios | Comfortable, Compact | `density` |
| Text size | dropdown | Smaller 0.9, Default 1, Larger 1.1, Large 1.25, Largest 1.5 (the nearest one shows for a hand-set value) | `text_scale` |
| Glass | radios | Clear, Tinted, Solid | `glass` |
| Adaptive glass | switch, off under Solid glass | small glass turning light or dark with its backdrop (an experiment) | `adaptive_glass` |
| Vibrant labels | switch, off under Solid glass | labels blending with the frosted backdrop, through niri's vibrancy (an experiment) | `vibrancy` |
| Motion | radios | Follow the system, Reduce motion, Full motion | `reduce_motion`: `null`, `true`, `false` |

An accent saved before macOS 27's colours shows as its new counterpart; picking it again saves
the new colour. What each setting does to the look is in
[materials.md](../design-system/materials.md#settings) and [tokens.md](../design-system/tokens.md).

### Desktop

Writes `desktop.json` (`desktop.rs`): **Show widgets on the desktop** (`show_widgets`, a switch)
and **Widget style** (`widget_style`): Dim while windows are open (`automatic`, the default),
Always full (`full`) or Always dimmed (`dim`). See [widgets.md](widgets.md).

### Displays

Each screen niri drives, with what it can do read from the screen itself (OLED, HDR, the
variable refresh range, the gamut, the bit depth): resolution, refresh rate, variable refresh,
scale, rotation and arrangement, written to niri's `displays.kdl`, plus brightness, Night Light's
schedule and OLED care. Changes that could leave a screen unreadable are kept only when confirmed.
See [displays.md](displays.md).

### Status Bar

Writes `status-bar.json` (`status_bar.rs`); see [status-bar.md](status-bar.md#settings).

| Setting | Control | Field |
|---|---|---|
| Position | segmented: Bottom, Top | `position` |
| Hiding | switch: Hide the status bar until the pointer meets the screen's edge | `auto_hide` |
| Material | radios: Frosted, Glass, Solid (black in dark mode, white in light) | `style` |
| Clock | switches: 24-hour time, Show seconds | `clock_24h`, `clock_seconds` |
| Date in the clock's tooltip | dropdown: Sunday 25 October, Sun 25 Oct, the locale's numbers | `date_format` |
| Workspaces | switches: Show workspaces, Scroll over them to move between workspaces | `show_workspaces`, `scroll_workspaces` |
| Show in the status bar | a switch per optional icon | `hidden` |

### Dock

Writes `dock.json` (`dock.rs`); see [dock.md](dock.md).

| Setting | Control | Field |
|---|---|---|
| Icon size | slider, 32–64 pt | `icon_size` |
| Magnification | segmented: Off, Small, Medium, Large | `magnification` |
| Hiding | switches: hide until the pointer meets the edge, show with the overview | `auto_hide`, `rise_with_overview` |
| Icons | switches: dots under open apps, an app's name over its icon, animate | `running_dots`, `tooltips`, `animations` |

### Quick Settings

The layout is the shell's own state, so this page (`quick_settings.rs`) has no file of its own: it
calls the running shell over IPC ([ipc.md](../integration/ipc.md#quick-settings)) and watches
`$XDG_STATE_HOME/amane-shell/quick-settings` to stay in step with Customize.

- **Customize…** (`quick-settings customize`) and **Reset Layout…** (`reset-layout`, after a
  dialog).
- **Controls**: a switch per control this machine has (`controls`, then `show` or `hide`).
- While the shell doesn't answer, a line says so, with **Try Again**.

### Spotlight

Writes `launcher.json` (`launcher.rs`); see [launcher.md](launcher.md).

| Setting | Control | Field |
|---|---|---|
| Search | a switch per source: Applications, Actions, Files, Calculations and conversions, Web addresses, Web search | `excluded_providers`: the ids switched off (`apps`, `actions`, `files`, `answers`, `url`, `web`) |
| Show applications as | radios: Grid, List | `apps_layout` |
| Clipboard history | Keep what's copied, for Clipboard; Keep copied images too; a caption with how many it keeps and the apps it leaves out | `clipboard.enabled`, `clipboard.images` (`max_items` and `exclude_apps` only by hand) |
| Quick keys | each action's keys with **Remove**, and **Reset All Quick Keys** | `quick_keys` |

Quick keys are set in the launcher itself (Actions, Ctrl+K); this page only lists and removes
them.

### About

The app's version, where the settings files are, and a note that `AMANE_THEME`, `AMANE_DENSITY`
and `AMANE_TEXT_SCALE` win over these settings when set.

## System

The NixOS configuration in `~/nixos-config`, through the contract in
[nixos.md](../integration/nixos.md): read from the flake's `amaneSettings` output, changed in
`managed/settings.json`, built as the user and activated through polkit. The pages are
`apps/settings/src/system/`.

### About This Computer

`system/about.rs`. A banner saying whether the running system is this configuration ("Up to
date", or why not: settings changed, uncommitted files, commits not applied, a differing
evaluation), with **Show Changes** and **Apply…**; then the NixOS version and nixpkgs commit,
the kernel, the generation ("65 of 66"), the commit the running system was built from with its
subject, the commit the repo is at, the repo path and host attribute (`nixos.json`, Enter to
set), and **Refresh**. Under *Managed settings*, the registry's controls for this page: **Time
zone** (a field with **Set**).

A setting another file sets shows locked, with the file and line ("Set in configuration.nix:46")
and **Move to Settings…** when the line is a plain `name = value;`: a dialog shows the line, and
**Move** comments it out, writes the value to `managed/settings.json` and commits both as
`move: … from …` ([nixos.md](../integration/nixos.md#move-to-settings)).

### General

`system/general.rs`. **Computer name** (a field with **Set**; lowercase letters, digits and
dashes), **Language** (a dropdown of the registry's locales; needs a restart), **Keyboard
layout** and **Keyboard variant** (fields checked against `localectl list-x11-keymap-layouts`
/ `-variants <layout>`; the console keymap follows the layout).

### Apps

`system/apps.rs`, two sections: *Add App* and *Installed*. *Add App*: the field searches as you type (after a 150 ms pause, two
characters or more; Enter or **Search** at once) in the nixpkgs catalog
(`system/catalog.rs`): every package `nix search --json --inputs-from <repo> nixpkgs ^` finds in
the configuration's own nixpkgs, less nixpkgs' `tests`, one tab-separated line each (attribute,
`pname`, version, description) in `$XDG_CACHE_HOME/amane-shell/nixpkgs/<rev>.tsv`, where `<rev>`
is the nixpkgs revision `flake.lock` pins for the root's `nixpkgs` input. The page builds it
when it opens and there's none for that revision (*Preparing the app catalog… m:ss*: seconds
with nix's evaluation cache warm, minutes the first time after a nixpkgs update), streaming
nix's JSON into the file, and deletes the other revisions' files. It looks at `flake.lock`
again at most every ten seconds, so an applied update gets its own catalog. Each search reads
the file again (about 10 MB, 106k packages, under 20 ms) rather than keeping it in memory: a
package matches when every word of the query is in its attribute, `pname` or description,
ignoring case. Typing while the catalog is built runs after it; when it can't be built, the
page says so and searches with `nix search … <query>` as before. Results are ranked:
top-level packages before ones in a set (`python313Packages.…`) unless the query has a dot,
then the name equal to the query, starting with it, containing it, then a description match,
then shorter paths. The best 50 show (*the best 50 of n matches* when there are more), two
lines each, attribute and version over the description, with **Add** (*Added* when it's in
the packages added in Settings, or a top-level package whose `pname` a `.nix` file installs). The field's
placeholder counts the catalog's packages.

Rows show apps as apps (`system/installed.rs`): each desktop entry in
`/run/current-system/sw/share/applications` links into the store path of the package that put
it there, so a package whose `<pname>-<version>` (or `<pname>-unwrapped-<version>`, for a
wrapper such as Steam's) has one in the launcher is drawn with that entry's name and icon
(*Visual Studio Code (vscode)*); the rest get a package glyph. The packages added in Settings
find their `pname` and version in the catalog. Pressing a row (a result, or an installed
package whose attribute is known) shows the
package's details in the page's place (`system/package.rs`): `nix eval --json
.#nixosConfigurations.<host>.pkgs.<attr> --apply …` (half a second; through the
configuration's own `pkgs`, so its nixpkgs config counts and an unfree package is available
when unfree ones are allowed), kept per nixpkgs revision. It shows the name and icon,
description and long description, where it stands (installed, *Installs on Apply*, *Installed
by modules/apps.nix*, not installed), the licence (*Unfree* when it is), the command it
installs (`meta.mainProgram`, with a terminal glyph when there's no app), the homepage with
**Open**, and **Add** / **Remove** / **Undo**, and **Open App**. A package nixpkgs would refuse
to build (`meta.broken`, `knownVulnerabilities`, or not `meta.available`) says why and can't be
added. **Add** in the results reads the same first (*Checking…*), and shows the details instead
of adding when the build would refuse it. A change waits
for Apply like any other.

*Installed* (`system/inventory.rs`) is one list of everything on the system: the packages in
`managed/settings.json`'s `packages` (nixpkgs attribute paths, `amane.packages` in the repo),
each with where it stands against HEAD's copy (what Apply last committed: *Added in Settings*,
with **Remove**; *Installs on Apply* or *Removed on Apply*, with **Undo**), listed first while
pending; the packages each repo file installs (the snapshot's `systemPackages`, with the file's
name); and the Flatpaks (`flatpak list --app`, looked at when the page first shows and every
half minute after, with **Uninstall…**: a dialog, then `flatpak uninstall -y
--noninteractive`; flatpak asks through polkit for a system-wide app). Each row is the app's
name and icon, over its attribute, version and where it's from. *All · Apps · Command-line ·
Flatpak* (with counts; Flatpak left out when flatpak isn't installed) and a filter field (every
word in the name, attribute or description) narrow it down; an app is a Flatpak, or a package
with an entry in the launcher. A file's package is matched to its attribute by the bare names
the file lists on lines of their own (`github-cli  # gh`), looked up in the catalog (`gh` is
`github-cli`'s `pname`); with one, the row opens its details and has **Move…**: Move to
Settings for a package (`adopt.rs`), a dialog, then the line listing it alone becomes a comment
(`# github-cli is in \`packages\` in managed/settings.json (Shell Settings).`), the attribute
joins `packages`, and both are one commit, `move: github-cli from modules/apps.nix`. Nothing is
installed or removed, so the system doesn't change. A package listed twice, or with another on
its line, says to edit the file by hand; one from a flake input (amane) has no attribute and no
**Move…**. A package added in Settings that this nixpkgs no longer has (the snapshot's
`missingPackages`: renamed or dropped by an update) is left out of the build with a warning
rather than failing it; its row is red, *no longer in this nixpkgs, so not installed*, with the
catalog's closest package by the last part of its name (*try python313Packages.requests*) and
**Remove**.

After an Apply that installs packages added in Settings, the apply bar says so by name instead
of the commit message, *Installed Zed, ripgrep (rg) · generation 68*: an app by its launcher
name, a command-line tool with its `meta.mainProgram` when the check before **Add** read it.
When exactly one of them is an app, **Open Zed** opens it. The job takes the list (the
packages in `settings.json` and not in HEAD's copy) just before it commits.

### Services

`system/services.rs`. A switch per service the registry puts on this page (SSH, Avahi,
Syncthing, KDE Connect, Flatpak, the fingerprint reader, fwupd, Bluetooth, Tailscale,
Sunshine), with the unit's state beside it ("Running · 79 MB", "Stopped") from
`systemctl show -p Id,ActiveState,SubState,MemoryCurrent` every five seconds (`--user` for a
user unit such as Sunshine's). A service that's installed but not started at boot has **Only
when needed** under it (`amane.services.<unit>.onDemand`, which empties the unit's `wantedBy`).
**Start** / **Stop** act now through `systemctl start|stop` (systemd asks through polkit) and
don't survive a restart; the switches wait for Apply.

### Power & Lock

`system/power.rs`. **Dim the screen after**, **Lock the screen after** and **Turn the screen
off after**: dropdowns over 1–30 minutes and *Never*, the `amane.idle.*` options that build
hypridle's listeners (home-manager restarts hypridle on Apply). Whether hypridle is running is
shown under them.

### History

`system/history.rs`. Every generation (`/nix/var/nix/profiles/system-N-link`), newest first:
when it was made and the subject of the commit it came from (else its NixOS version), marked
*current* and *booted*. Picking one shows its number, version and commit, with **Try** (activates
it without recording it; a restart undoes it) and **Restore…** (a dialog, then a new generation
made from it).

### Storage

`system/storage.rs`. How full `/nix` and `/boot` are (meters), **Check What a Clean-up Frees**
(`nix-collect-garbage --dry-run`), and the registry's controls for this page: **Boot entries
kept** (a slider, 1–50).

### Software Update

`system/update.rs`. **Check for Updates** writes a fresh lock beside the repo
(`nix flake update --output-lock-file $XDG_STATE_HOME/amane-shell/nixos/update.lock`), and if
any root input moved, builds the working tree with it (`nix build --reference-lock-file`) and
compares it with the running system, through the Apply bar like any build; a card then says
"Update ready: 2 inputs change · 143 packages change · needs a restart" with **Show Changes**
(the inputs' commits first, then the packages). The Apply bar's **Apply Now** / **Apply at
Restart** / **Try** activate it; after a `switch` or `boot` the lock goes into the repo as
`flake.lock` and is committed with `managed/settings.json` as `update: nixpkgs 26f4107 →
1a2b3c4, …`. Nothing moved: "Already up to date". Below, the inputs from `flake.lock`, each
with how long ago it was updated and its commit (the old and new commits while a check is
around), and **Update** to check just that input.

The scheduled check ([nixos.md](../integration/nixos.md#the-scheduled-check)) fills the same
card between checks: "Checked 2 hours ago · updates for nixpkgs, amane-shell" with **Update
All**, "Checked yesterday · up to date", or why it couldn't; the inputs it saw move read `from
→ to` in the list. **Check automatically** (Daily, Weekly, Never) is the timer's setting, a
managed setting like the others. The shell shows the check's toast, and its Software Update
tile (Quick Settings, Add Controls → Utilities) names the inputs while something waits.

### The Apply bar

`system/apply_bar.rs`, under whichever page shows while there's something to apply:

| State | Says | Buttons |
|---|---|---|
| Waiting | "2 changes waiting · Time zone → Europe/Oslo · Tailscale off" | **Discard** (a dialog), **Try** (builds, then activates in `test` mode), **Apply…** (builds) |
| Building | a meter and "building amane-shell (3/7) · 1:12", "Evaluating…", or "checking for updates" | **Show Log**, **Cancel** |
| Built | "Ready: 12 packages change · needs a restart" ("Update ready: …" for a check) | **Show Changes**, **Cancel**, **Try**, **Apply at Restart** (when the kernel changes), **Apply Now** |
| Up to date | "Already up to date: every input is at its newest." (a check that found nothing) | **Dismiss** |
| Applying | "Applying… authenticate when asked." | **Show Log** |
| Done | "Applied · generation 67 · settings: …", "Tried · Apply to keep it", or "Applied, but not committed: …" | **Show Log**, **Dismiss** |
| Failed | "Build failed: …" in the critical colour | **Show Log**, **Dismiss** |

**Show Changes** and **Show Log** take the page's place (the settings waiting, then the
packages that update, are added and removed; the build's last 300 lines, with **Copy Log**),
with **Back**.

## The files

`appearance.json`, `desktop.json`, `status-bar.json`, `dock.json`, `launcher.json`, `displays.json`
(`battery_refresh`) and `nixos.json`
(`repo`, `host`: where the NixOS configuration is) are JSON in
`$XDG_CONFIG_HOME/amane-shell/` (else `~/.config/amane-shell/`), read and
written through amane's `Config` by `amane_kit::settings`. A missing field takes its default, so
an older or hand-written file still works:

```json
{ "mode": null, "accent": null, "density": "comfortable", "text_scale": 1.0, "glass": "tinted",
  "reduce_motion": null, "adaptive_glass": false, "vibrancy": false }
```

`AMANE_THEME`, `AMANE_DENSITY`, `AMANE_TEXT_SCALE`, `AMANE_GLASS`, `AMANE_ADAPTIVE_GLASS=1` and
`AMANE_VIBRANCY=1` win over the file when set. The shell's other files, its state, are in
[services.md](../integration/services.md#settings-and-state).
