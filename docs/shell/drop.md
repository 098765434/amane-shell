# Drop (`amane-drop`)

A drop shelf in the status bar (`apps/drop`). Its item in the bar opens a
**grid** of folders, apps and actions to let files go on, with a **Drop Bar** under them to hold
files for later. It runs as its own program and puts its item up over the bar's
[status apps](../integration/status-apps.md) service. Its work shows as an activity in the
bar.

## Usage

- **Click** Drop's item (the tray glyph) to open the grid under it. Escape or a click outside
  closes it. 1–9 and ← → pick a grid.
- **Drag files over the item** and the grid opens at once. Carry on into it and let go on a
  tile, and it closes once the files leave.
- **Right-click** the item for its menu: the grids, Recently Shared, Edit Grid, Drop
  Settings…, Clear Drop Bar, and Quit Drop.

Files let go on the item itself go onto the Drop Bar.

| Tile | Let go on it | Click |
|---|---|---|
| A folder | copies them in, or moves them (switched in Edit) | opens the folder |
| An app | opens them with the app | opens the app |
| Copy To… | copies them into a folder chosen the first time | opens that folder |
| Move To… | moves them there | opens that folder |
| Rename & Move | asks for a name, then moves them. One file keeps its extension; several are numbered ("Trip 1.jpg") | opens that folder |
| Save Text | text let go on it is saved as "Dropped Text.txt" | opens that folder |
| Send to Phone | sends them to the phone over KDE Connect; text or a link opens on it | |
| 0x0.st, Imgur, SFTP, S3 | uploads them, or text as a file, and copies the link | opens Settings when it isn't set up |

- **Conflicts.** A name already taken asks first: **Keep Both** (the new one gets a number),
  **Replace**, or **Stop**.
- **Progress.** A task shows a ring on its tile, then a check or a cross. It also shows as a Live
  Activity in the bar, with Cancel. A failure marks the item until the grid is next opened.
- **Moving.** A move within one disk is a rename. Across disks it's a copy, then the original is
  removed. A move cancelled halfway leaves the original whole.

## Sharing

A sharing tile works like the others: progress on its tile and as an activity, with Cancel.

- **Send to Phone** goes to the phone paired with KDE Connect (Quick Settings › Phone). With
  several connected it asks which, unless the tile names one: a click on it while editing goes
  through the paired phones and back to asking. Files land in the phone's downloads.
- **0x0.st** needs no account. Anyone with the link can see the file; it's kept from 30 days to a
  year, the bigger the shorter.
- **Imgur** takes pictures and videos, anonymously, with a client ID from an app registered at
  imgur.com.
- **SFTP** puts the file in a folder on a server (from the home folder unless it starts with `/`,
  made when missing) and gives the server's web address with the file's name after it. It logs
  in with a private key or a password. The server has to be in `~/.ssh/known_hosts`: connect
  once with `ssh` first.
- **S3** puts the file in a bucket at AWS or another S3-compatible store (R2, MinIO: give its
  endpoint), optionally readable by anyone, and gives the web address the bucket is served from,
  else the object's own.

Once an upload is done its link goes on the clipboard and a notification says so (both can be
turned off), and the activity offers **Open**. The last ten go under **Recently Shared** in the
item's menu; picking one copies it again. They're kept in
`$XDG_STATE_HOME/amane-drop/shared.json`.

Uploads run `curl`. Passwords and keys go to curl on its standard input, never as arguments
(which `ps` would show). They're kept in the keyring through `secret-tool`, or in
`~/.config/amane-drop/secrets.json` (mode 0600) when no keyring answers. Folders aren't
uploaded.

## Editing

**Edit** (top right) changes the grid:

- each tile gets a ✕;
- a click on a folder switches it between copying and moving;
- a click on an action chooses its folder again;
- **Folder…** adds a folder;
- **Grid** adds a grid, and ✕ removes this one;
- a click on an upload tile opens its settings;
- the actions and ways to share this grid hasn't got can be added; their buttons wrap to the
  card's width.

Dropping folders or `.desktop` files between the tiles adds them as tiles, edit or not.

The **Drop Bar** holds each drop as a stack, whose picture is its first file and whose badge is
how many it holds. Drag a stack out to another program or onto a tile. A right-click lets it go,
and ✕ clears them all. It keeps references only, so a file moved away shows faded. Each grid has
its own, kept in `$XDG_STATE_HOME/amane-drop/dropbar.json`.

## Commands

A launch while it runs goes to the copy running, as does the same over its socket
(`$XDG_RUNTIME_DIR/amane-drop.sock`, one argument a line):

| Command | Does |
|---|---|
| `amane-drop toggle` | opens or closes the grid (for a keybind) |
| `amane-drop open [n]` | opens it, on grid `n` (from 1) |
| `amane-drop close` | closes it |
| `amane-drop edit` | opens it, editing |
| `amane-drop run <tile> <file>…` | what letting the files go on tile `<tile>` (from 1, in the grid showing) does |
| `amane-drop hold <file>…` | puts the files on the Drop Bar as a stack |

```sh
printf 'run\n1\n/home/me/report.pdf\n' | nc -U -N $XDG_RUNTIME_DIR/amane-drop.sock
```

## Settings

Shell Settings › Drop changes everything below (see
[settings-app.md](settings-app.md#drop)).

The grids are `~/.config/amane-drop/grids.json` (`amane_kit::settings::DropGrids`), followed live:

- `grids`: each grid's `name` and `tiles`;
- each tile is `{"kind": "folder", "path", "mode": "copy" | "move"}`, `app` (`id`, a desktop
  file id), `copy-to`, `move-to`, `rename-move` or `save-text` (each with an optional `folder`),
  `phone` (an optional `device`, KDE Connect's id) or `upload` (`to`: `0x0`, `imgur`, `sftp`
  or `s3`);
- `columns`: tiles a row, 3–6.

The accounts are `~/.config/amane-drop/sharing.json` (`DropSharing`): `sftp` (`host`, `port`,
`user`, `folder`, `url`, `key`), `s3` (`endpoint`, `region`, `bucket`, `prefix`, `key_id`,
`url`, `public`), `imgur_client_id`, `copy_link` and `notify`. The secrets are under
`application amane-drop`, `account sftp` or `s3`.

A fresh install starts with ~/Downloads and ~/Desktop (when they're there) and the four actions.

## Installing

The shell's flake builds `amane-drop` with the rest of the workspace. Its wrapper puts `curl`,
`secret-tool` (libsecret) and `wl-copy` (wl-clipboard) after the system's `PATH`, so they're
there even when the system lacks them, and it installs `amane-drop.desktop`, so the launcher finds
"Drop". nixos-config runs it as a user service and binds a key:

```nix
# home.nix: a unit of its own, outside amane-shell's cgroup, so a restart of
# the shell doesn't take it down; it puts its item back once the bar is there
systemd.user.services.amane-drop = {
  Unit = {
    Description = "amane-drop (Drop: a grid of folders, apps and uploads)";
    PartOf = [ "graphical-session.target" ];
    After = [ "graphical-session.target" ];
  };
  Service = {
    ExecStart = "${amane-shell}/bin/amane-drop";
    Restart = "on-failure";
  };
  Install.WantedBy = [ "graphical-session.target" ];
};
```

```kdl
// niri: the grid, or start Drop when it isn't running
Mod+Shift+D hotkey-overlay-title="Drop" { spawn-sh "printf 'toggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane-drop.sock || systemctl --user start amane-drop.service"; }
```

The grid's frost needs the `amane.drop` layer rule. It's in `niri-layer-rule.kdl`, and
`gen-layer-rules.sh` splices it into niri's config ([layer-rules.md](../integration/layer-rules.md)).

**Quit Drop** in its menu exits cleanly, so the unit stays stopped until the next login or
Mod+Shift+D. A crash restarts it.

## How it's built

| File | What |
|---|---|
| `main.rs` | the status item (`amane_status::StatusItem`) and its events; launches and IPC |
| `state.rs` | what the grid shows, shared between the bus thread, tasks and the window; the Drop Bar on disk |
| `grid.rs` | the window: a clear full-screen Overlay surface frosting only its card (a blur shape, layer rule `amane.drop`) |
| `tasks.rs` | jobs on threads of their own, each a `LiveActivity` with Cancel |
| `share.rs` | uploads through `curl`: its arguments, its progress bar read, killed on Cancel |
| `shared.rs` | Recently Shared, and a link copied (`wl-copy`, else amane's clipboard) and told |

How the pieces fit together, and how to test Drop without touching the real clipboard, keyring
or phone, is in [development/status-apps.md](../development/status-apps.md).
| `files.rs` | copying and moving a chunk at a time, folders whole; free names ("photo 2.jpg") |

Opened by a click, it takes the keys (`Keyboard::Exclusive`) and its whole surface takes input, so
a click outside closes it. Opened by a drag, only the card takes input (`input_region`), so the
drag can still go to other programs. Everything is rectangles, glyphs and arcs:
`AMANE_FRAMES=1` with `amane/vello-stats` shows `vello 0`.

To try uploads without sending anything out, `AMANE_DROP_0X0_URL` points the 0x0.st tile at
another server, and an S3 endpoint can be a local one.
