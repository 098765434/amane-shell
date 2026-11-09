# Emaki Settings

`emaki-settings` (`apps/settings/`) is the shell's settings app, built on the kit's
[app scaffolding](../../developer/kit/app.md). Each control changes one of the shell's settings files, which the
running shell follows at once; Emaki Settings' own window redraws in the new look too. Editing a
file by hand works the same way.

Only one copy runs: launching it again brings the open window forward, and `--page <name>`
(`general`, `apps`, `services`, `power`, `storage`, `history`, `update`, `about`; or a Shell
page's name, like `displays`) opens that page, cold or in the running copy (`App::on_activate`): the launcher's
system actions and the update toast use it. `--package <attr>` opens Apps with that nixpkgs
package's details. The flake installs it with a `.desktop` entry, so
the launcher lists it as *Emaki Settings*.

## The window

An `app_window` titled "Emaki Settings", 760 × 760 (narrower down to where a 440 px page still
fits), with a 200 px [`sidebar`](../../developer/kit/app.md#sidebar) in two sections, **Shell** (eight pages,
the shell's own settings and the displays) and **System** (eight, the NixOS configuration), each
page's glyph on a colored tile, under a **Search** field. Beside it, a toolbar row holds the
back and forward pill, the page's title, and a page's own button where it has one (Quick
Settings' **Customize…**). The settings are grouped rows
([`form`](../../developer/kit/components.md#form)): rounded cards of rows, each a title (and a grey subtitle)
with its control at the trailing edge, small pictures to choose from
([`ThumbnailPicker`](../../developer/kit/components.md#form)), a slider across its row, captions above the
groups and notes below them. A setting that only means something while the one above is on is
set in under it, and dimmed while it's off. The groups are as wide as the window allows, from
440 to 680 px, centred beyond that.

**Moving around.** Where the window is, is a history of places: a page and the views opened over
it. A view is a page's sub-page (Status Bar's Clock, Weather and Items; Displays' Advanced;
About This Computer's Configuration; About's Settings Files), the changes an Apply makes, the
build log, or a package's details; a row with a chevron opens one. The pill (or Alt+Left and
Alt+Right) goes back and forward through the places, and Escape leaves a view. `--page` takes
a sub-page too, as `page/sub` (`status-bar/clock`).

**Search.** Typing in the field (Ctrl+F puts the keys there) narrows the sidebar to the pages
with something found and shows what's found in the page's place: each a title and where it is
(*Status Bar › Clock*). Shell pages' settings are listed in `search.rs` with other words people
use for them ("dark mode", "wifi", "burn in"); the System pages' settings are found by their
labels as the configuration reads. Matching is the launcher's (the kit's `fuzzy`): a title's
start, a word's start, its initials ("qs"), anywhere in it, or its letters in order; another word
for a setting counts half as much, so a title comes first. A result (or Return, for the first)
opens its place and lights its row for a moment; Escape clears the field. Tests hold the index to
the pages: every page and sub-page has an entry, every entry's place opens, and every row it
lights is a row on a page.

**While things load,** a row shows a small spinner in place of its value (the kit's
[`Spinner`](../../developer/kit/components.md#form)), and a page whose rows haven't come in keeps its caption
and note. Something that went wrong is said plainly on a row with a warning tile, with
**Details…** for what the tool itself said. Fields keep what's typed on Return, or when the field
is left with something new. Each page scrolls in an amane `ScrollArea` that keeps its own
offset. Ctrl+W closes it (`standard_shortcut`), unless a system build is running: then it asks
first ("Stop the build?"), as does the compositor's close.

Its content is drawn on the opaque window background; only the sidebar shows the frost behind the
window. That frost, and the frosted popups (the dropdowns' menus), come from the `window-rule`
the generated config has for `emaki-settings` ([layer-rules.md](../../developer/integration/layer-rules.md)).
Without it the sidebar is an opaque panel. A niri window rule can still round the window like
other apps:

```kdl
window-rule {
    match app-id="^emaki-settings$"
    geometry-corner-radius 12
    clip-to-geometry true
}
```

## Pages

### Appearance

Writes `appearance.json` (`apps/settings/src/appearance.rs`).

| Setting | Control | Choices | Field |
|---|---|---|---|
| Appearance | pictures | Light, Dark, Auto (follows the system) | `mode`: `"light"`, `"dark"`, `null` |
| Accent color | dots, the hovered one named | System (multicolor), Blue, Purple, Pink, Red, Orange, Yellow, Green, Graphite (the design kit's colors) | `accent`: `null` or `#rrggbb` |
| Density | segmented | Comfortable, Compact | `density` |
| Text size | dropdown | Smaller 0.9, Default 1, Larger 1.1, Large 1.25, Largest 1.5 (the nearest one shows for a hand-set value) | `text_scale` |
| Glass | pictures | Clear, Tinted, Solid | `glass` |
| Adaptive glass | switch, off under Solid glass | small glass turning light or dark with its backdrop (an experiment) | `adaptive_glass` |
| Vibrant labels | switch, off under Solid glass | labels blending with the frosted backdrop, through niri's vibrancy (an experiment) | `vibrancy` |
| Motion | segmented | Automatic, Reduced, Full | `reduce_motion`: `null`, `true`, `false` |

An accent saved before the design kit's colors shows as its new counterpart; picking it again saves
the new color. What each setting does to the look is in
[materials.md](../../developer/design-system/materials.md#settings) and [tokens.md](../../developer/design-system/tokens.md).

### Desktop

Writes `desktop.json` (`desktop.rs`): **Show widgets on the desktop** (`show_widgets`, a switch)
and, set in under it, **While windows are open** (`widget_style`, as pictures): Dim with windows
(`automatic`, the default), Always full (`full`) or Always dimmed (`dim`); and **Edit Widgets…**,
which asks the shell to edit them (`widgets edit`). See [widgets.md](widgets.md).

### Displays

Each screen niri drives, with what it can do read from the screen itself (OLED, HDR, the
variable refresh range, the gamut, the bit depth): resolution, refresh rate, variable refresh,
scale, rotation and arrangement, written to niri's `displays.kdl`, plus brightness, Night Light's
schedule and OLED care. The selected display's settings come first, then **All Displays**
(Slower on battery), Night Light and OLED care; **Advanced** (a custom scale or mode, the
overview's backdrop, what the display reports) is a sub-page. Changes that could leave a screen
unreadable, moving one in the arrangement too, are kept only when confirmed. On demand variable
refresh can add an app that's open from a menu. See [displays.md](displays.md).

### Input

Key repeat (delay and rate, with a field to try it), Num Lock at login, and the touchpad's and
mouse's tap to click, natural scrolling, off while typing, speed, acceleration, right click and
scroll method, written to niri's `input.kdl`. The layout stays under General. **Keyboard
Shortcuts** is a sub-page listing every bind in niri's config, read-only. See [input.md](input.md).

### Status Bar

Writes `status-bar.json` (`status_bar.rs`); see [status-bar.md](status-bar.md#settings).

| Setting | Control | Field |
|---|---|---|
| Position | segmented: Bottom, Top | `position` |
| Hide automatically | dropdown: Never, Always, On the desktop only, In full screen only | `auto_hide` |
| Material | pictures: Clear, Frosted, Glass, Solid (black in dark mode, white in light) | `style` |
| Clock › (sub-page) | pictures: Digital, Analog; switches: 24-hour time, Show seconds, Flash the time separators (dimmed under Analog), Show the day of the week; dropdowns: Show the date (When space allows, Always, Never), Date in the tooltip | `clock_style`, `clock_24h`, `clock_seconds`, `flash_separators`, `show_weekday`, `show_date`, `date_format` |
| Weather › (sub-page) | switch: Show the weather; field: Place (kept on Return or when left) | `weather`, `weather_place` |
| Items › (sub-page) | every item in the bar's order, each with buttons to move it a place and, for the optional ones, a switch; Reset Order… (asks first); Allow in the Status Bar, a switch per program seen with its own item (`app:<id>` in `hidden`) and per app seen in the tray | `order`, `hidden`, `tray_denied` |
| Activities › (sub-page) | switch: Show Activities; segmented: Show with their text, 1–3; Now Playing's Always show; Allow Activities, a switch per app (Now Playing, Timer, Screen Sharing, then each seen) | `hidden` (`activities`), `activities_compact`, `media_always`, `activities_denied` |
| Workspaces | switches: Show workspaces, and under it Scroll to switch | `show_workspaces`, `scroll_workspaces` |

The main page's Clock, Weather, Items and Activities rows sum up what's set ("24-hour · date
when space allows", "Oslo", "13 shown", "2 with their text").

### Dock

Writes `dock.json` (`dock.rs`); see [dock.md](dock.md).

| Setting | Control | Field |
|---|---|---|
| Icon size | slider, 32–64 pt | `icon_size` |
| Magnification | segmented: Off, Small, Medium, Large | `magnification` |
| Hiding | switches: hide until the pointer meets the edge, and under it show with the overview | `auto_hide`, `rise_with_overview` |
| Icons | switches: dots under open apps, an app's name over its icon, animate | `running_dots`, `tooltips`, `animations` |

### Notifications

Writes `notifications.json` (`notifications.rs`); see
[notifications.md](notifications.md#per-app-rules).

| Setting | Control | Field |
|---|---|---|
| Apps | a row per app that has sent a notification (from the cache's `notifiers-seen.json`), with its icon and a menu: Banners and List, List Only, Off | `apps` (an allowed app is left out) |

### Quick Settings

The layout is the shell's own state, so this page (`quick_settings.rs`) has no file of its own: it
calls the running shell over IPC ([ipc.md](../../developer/integration/ipc.md#quick-settings)) and watches
`$XDG_STATE_HOME/emaki/quick-settings.json` to stay in step with Customize.

- **Customize…** (`quick-settings customize`) and **Reset Layout…** (`reset-layout`, after a
  dialog).
- **Controls**: a switch per control this machine has (`controls`, then `show` or `hide`).
- While the shell doesn't answer, a line says so, with **Try Again**.

### Launcher

Writes `launcher.json` (`launcher.rs`); see [launcher.md](launcher.md).

| Setting | Control | Field |
|---|---|---|
| Search | a switch per source: Applications, Actions, Files, Calculations and conversions, Web addresses, Web search | `excluded_providers`: the ids switched off (`apps`, `actions`, `files`, `answers`, `url`, `web`) |
| Show applications as | pictures: Grid, List | `apps_layout` |
| Clipboard history | Keep what's copied, for Clipboard; Keep copied images too; a caption with how many it keeps and the apps it leaves out | `clipboard.enabled`, `clipboard.images` (`max_items` and `exclude_apps` only by hand) |
| Quick keys | each action's keys with **Remove**, and **Remove All…** (asks first) | `quick_keys` |

Quick keys are set in the launcher itself (Actions, Ctrl+K); this page only lists and removes
them.

### Drop

Writes emaki-drop's `grids.json` and `sharing.json` (`drop.rs`); see [drop.md](drop.md).

| Setting | Control | Field |
|---|---|---|
| Grids | a row per grid with its tile count: **Rename…** (a dialog), up and down, ✕ (asks first, when there are more than one) | `grids` |
| Tiles a row | dropdown, 3–6 | `columns` |
| Grids and their tiles | **Add Grid**, **Open Drop** (`emaki-drop open`) | |
| After an upload | switches: Copy the link, Show a notification | `copy_link`, `notify` |
| Imgur | Client ID | `imgur_client_id` |
| SFTP | Server, Port, User name, Folder, Web address; Private key (**Choose…**, ✕); Password or Key passphrase | `sftp.*` |
| S3 | Endpoint, Region, Bucket, Folder, Access key ID; Secret access key; Web address; Anyone can read uploads | `s3.*` |

Send to Phone and 0x0.st need nothing; their rows say so. Text fields keep what's typed on Return
or once they lose the keys. A password field empties once its text is kept in the keyring
(`emaki_kit::secrets`, through `secret-tool`); its row then says "In the keyring" and offers
**Forget**. Where no keyring answers, the S3 group's note says secrets are in
`~/.config/emaki-drop/secrets.json`, readable only by the user.

### Passwords

Bitwarden, through rbw ([passwords.md](passwords.md)). The page reads the vault's state from the
shell's `passwords status` every two seconds, and runs every command through the shell, so an
unlock is always the shell's own, and its dialog asks.

- **Bitwarden:** locked or not, how many logins, and Unlock…, Lock, Sync or Sign In…;
  Register… when the server asks for it.
- **Account:** the server and email; Save points rbw's pinentry at the shell. Changing the
  account removes the old one's copy of the vault, after asking.
- **Security:** lock after 5–60 minutes, and lock with the screen.
- **Fill:**
  - the toggles: fill in apps, username fields, the launcher, and hiding while sharing;
  - a warning when another input method holds the seat, or niri can't fill safely.
- **Remembered for apps:** each entry linked to an app, by desktop id and package, with ×. The
  apps never offered fill are listed too.

### About

The app's card (its icon, name and version), and **Settings files ›**: a sub-page with each
file's path (from `~`, shortened in the middle when long) and **Show in Folder**, and a note that
`EMAKI_THEME`, `EMAKI_DENSITY` and `EMAKI_TEXT_SCALE` win over these settings when set.

## System

The NixOS configuration in `~/nixos-config`, through the contract in
[nixos.md](../../developer/integration/nixos.md): read from the flake's `emakiSettings` output, changed in
`managed/settings.json`, built as the user and activated through polkit. The pages are
`apps/settings/src/system/`.

### About This Computer

`system/about.rs`. The computer's card (its name and the NixOS it runs), then a status row
saying whether the running system is this configuration ("Up to date", or why not: settings
changed, uncommitted files, commits not applied, a differing evaluation), with **Show Changes**
and **Apply**; then the NixOS version and nixpkgs commit, the kernel, the generation running
("79", or "79 (next boot: 80)" after a Try or an Apply at Restart), the commit the running system
was built from with its subject, and the commit the folder is at. **Configuration folder ›**
opens a sub-page with the folder and the flake's machine name (`nixos.json`, kept on Return or
when a field is left) and **Refresh**. Then the registry's controls for this page: **Time zone**.

A setting another file sets shows locked, with the file and line ("Set in configuration.nix:46")
and **Move to Settings…** when the line is a plain `name = value;`: a dialog shows the line, and
**Move** comments it out, writes the value to `managed/settings.json` and commits both as
`move: … from …` ([nixos.md](../../developer/integration/nixos.md#move-to-settings)).

### General

`system/general.rs`. **Computer name** (a field, kept on Return or when left; lowercase letters,
digits and dashes), **Language** (a dropdown of the registry's locales; needs a restart), **Keyboard
layout** and **Keyboard variant** (fields checked against `localectl list-x11-keymap-layouts`
/ `-variants <layout>`; the console keymap follows the layout).

### Apps

`system/apps.rs`, two sections: *Add App* and *Installed*. *Add App*: the field searches as you type (after a 150 ms pause, two
characters or more; Enter or **Search** at once) in the nixpkgs catalog
(`system/catalog.rs`): every package `nix search --json --inputs-from <repo> nixpkgs ^` finds in
the configuration's own nixpkgs, less nixpkgs' `apps/shell/tests`, one tab-separated line each (attribute,
`pname`, version, description) in `$XDG_CACHE_HOME/emaki/nixpkgs/<rev>.tsv`, where `<rev>`
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
`managed/settings.json`'s `packages` (nixpkgs attribute paths, `emaki.packages` in the repo),
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
(`# github-cli is in \`packages\` in managed/settings.json (Emaki Settings).`), the attribute
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

### Default Apps

Which app opens links, folders and each kind of file, through amane's `MimeApps`
([apps-and-processes](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/system/apps-and-processes.md)),
which reads every `mimeapps.list` as `xdg-open` does and writes only `~/.config/mimeapps.list`.

- **Open With** has one dropdown per role, each with a tile:
  - Web browser, Mail, Folders, Text editor, Images, Video, Music, PDF documents, Archives and
    Calendar invitations.
  - Each role is known by one kind (a browser by `https` links). Its dropdown lists the apps that
    open that kind and shows the one that opens it now, falling back to `mimeinfo.cache` as
    `xdg-open` does.
  - Choosing an app sets the role's kinds. For links and mail that is a fixed set (`http`,
    `https`, `text/html`, `application/xhtml+xml`). For images, video, music and text it is every
    kind under that prefix the app lists, except that the text editor takes neither `text/html`
    nor calendar or contact files.
- **Terminal** shows while `xdg-terminal-exec` is installed. It lists the `TerminalEmulator`
  apps and writes the chosen one first in `~/.config/xdg-terminals.list`. niri's Super+T and
  Super+Return spawn `xdg-terminal-exec`.
- **File Types** (sub-page `default-apps/file-types`): type an extension (`md`, `.md`,
  `notes.md`) or a type (`text/markdown`) to pick its app. **Chosen by You** lists every default
  set in the user's file, each with its app and **Reset**, which hands the kind back to the
  system's choice.

### Services

`system/services.rs`. A switch per service the registry puts on this page (SSH, Avahi,
Syncthing, KDE Connect, Flatpak, the fingerprint reader, fwupd, Bluetooth, Tailscale,
Sunshine), with the unit's state beside it ("Running · 79 MB", "Stopped") from
`systemctl show -p Id,LoadState,ActiveState,SubState,MemoryCurrent` every five seconds
(`--user` for a user unit such as Sunshine's), and a dot before its name: green running, yellow
starting or stopping, red failed, grey off. A spinner shows until the first look. A service
that's installed but not started at boot has **Start only when needed** set in under it
(`emaki.services.<unit>.onDemand`, which empties the unit's `wantedBy`). **Start** / **Stop**
act now through `systemctl start|stop` (systemd asks through polkit) and don't survive a
restart; they show only for a unit systemd has (not for one switched on and not yet applied).
Stopping SSH, Tailscale or Sunshine asks first, since a remote session may ride on it. The
switches wait for Apply.

### Power & Lock

`system/power.rs`. **Dim the screen after**, **Lock the screen after** and **Turn the screen
off after**: dropdowns over 1–30 minutes and *Never*, the `emaki.idle.*` options that build
hypridle's listeners (home-manager restarts hypridle on Apply). A note says when a step is set
to come before the one it follows ("The screen locks before it dims."). **Idle service** shows
whether hypridle is running.

### History

`system/history.rs`. Every generation (`/nix/var/nix/profiles/system-N-link`), newest first:
when it was made and the subject of the commit it came from (else its NixOS version), marked
*current* and *booted*. Picking one shows, under it, **Try…** (a dialog, then it runs without
being recorded; a restart undoes it) and **Restore…** (a dialog, then a new generation made from
it); picking it again puts them away.

### Storage

`system/storage.rs`. How full *System and apps* (`/nix`) and the *Startup partition* (`/boot`)
are (progress bars). **Clean up**: **Check** counts what nothing uses
(`nix-collect-garbage --dry-run`: "5,139 unused items can be removed"), then **Clean Up…** asks
and runs `nix-store --gc` through the daemon, as the user: what only old, already removed
versions used goes; the versions themselves stay for History ("Freed 3.2 GB"). The weekly
clean-up removes versions older than 14 days. Then the registry's controls for this page:
**Boot entries kept** (a stepper, 1–50). Whole-number settings from the registry are steppers.

### Software Update

`system/update.rs`. **Check for Updates** writes a fresh lock beside the repo
(`nix flake update --output-lock-file $XDG_STATE_HOME/emaki/nixos/update.lock`), and if
any root input moved, builds the working tree with it (`nix build --reference-lock-file`) and
compares it with the running system, through the Apply bar like any build; a row then says
"Update ready: 2 inputs change · 143 packages change · needs a restart" with **Show Changes**
(the inputs' commits first, then the packages). The Apply bar's **Apply Now** / **Apply at
Restart** / **Try** activate it; after a `switch` or `boot` the lock goes into the repo as
`flake.lock` and is committed with `managed/settings.json` as `update: nixpkgs 26f4107 →
1a2b3c4, …`. Nothing moved: "Everything is up to date." Below, under *Components*, the inputs
from `flake.lock` (by a plain name where there's one: nixpkgs is *NixOS packages*, niri the
*Compositor*), each with how long ago it was updated and its commit (the old and new commits
while a check is around, *not pinned* for none), and **Update** to check just that input.

The scheduled check ([nixos.md](../../developer/integration/nixos.md#the-scheduled-check)) fills the same
row between checks: "Checked 2 hours ago · updates for nixpkgs, emaki" with **Update
All**, "Checked yesterday · up to date", or why it couldn't; the inputs it saw move read `from
→ to` in the list. **Check automatically** (Daily, Weekly, Never) is the timer's setting, a
managed setting like the others. The shell shows the check's toast, and its Software Update
tile (Quick Settings, Add Controls → Utilities) names the inputs while something waits.

### The Apply bar

`system/apply_bar.rs`, under whichever page shows while there's something to apply:

| State | Says | Buttons |
|---|---|---|
| Waiting | "2 changes waiting · Time zone → Europe/Oslo · Tailscale off" | **Discard** (a dialog), **Try** (builds, then activates in `test` mode), **Apply** (builds) |
| Building | a progress bar (a spinner while evaluating) and "Building emaki (3/7) · 1:12", "Evaluating…", or "Checking for updates" | **Show Log**, **Cancel** (asks first) |
| Built | "Ready: 12 packages change · needs a restart" ("Update ready: …" for a check) | **Show Changes**, **Cancel**, **Try**, **Apply at Restart** (when the kernel changes), **Apply Now** |
| Up to date | "Everything is up to date." (a check that found nothing) | **Dismiss** |
| Applying | "Applying… authenticate when asked." | **Show Log** |
| Done | "Applied · generation 67 · settings: …", "Tried · Apply to keep it", or "Applied, but not committed: …" | **Show Log**, **Dismiss** |
| Failed | "Build failed: …" in the critical color | **Show Log**, **Dismiss** |

**Show Changes** and **Show Log** take the page's place (the settings waiting, then the
packages that update, are added and removed; the build's last 300 lines, with **Copy Log**),
with back in the toolbar's pill.

## The files

`appearance.json`, `desktop.json`, `status-bar.json`, `dock.json`, `launcher.json`, `displays.json`
(`battery_refresh`) and `nixos.json`
(`repo`, `host`: where the NixOS configuration is) are JSON in
`$XDG_CONFIG_HOME/emaki/` (else `~/.config/emaki/`), read and
written through amane's `Config` by `emaki_kit::settings`. A missing field takes its default, so
an older or hand-written file still works:

```json
{ "mode": null, "accent": null, "density": "comfortable", "text_scale": 1.0, "glass": "tinted",
  "reduce_motion": null, "adaptive_glass": false, "vibrancy": false }
```

`EMAKI_THEME`, `EMAKI_DENSITY`, `EMAKI_TEXT_SCALE`, `EMAKI_GLASS`, `EMAKI_ADAPTIVE_GLASS=1` and
`EMAKI_VIBRANCY=1` win over the file when set. The shell's other files, its state, are in
[services.md](../../developer/integration/services.md#settings-and-state).
