# IPC and CLI

Every command the `amane-shell` binary takes, on its command line and over its IPC socket
(`src/main.rs`). The surface docs link here rather than repeat it.

## Command line

`amane-shell` with no arguments runs the shell. These run a tool and exit instead:

| Command | Does |
|---|---|
| `amane-shell tokens normalize <file>` | prints a Penpot export normalized, for `design-tokens/tokens.json` |
| `amane-shell tokens check <file>` | runs the manual's rules on a token file and prints how it differs from the committed tokens; fails when a rule does ([tokens.md](../design-system/tokens.md#103-developer-bring-an-export-in)) |
| `amane-shell layer-rules` | prints the niri layer rules generated from the tokens ([layer-rules.md](layer-rules.md)) |

`tokens` with anything else prints its usage and fails. Any other arguments run the shell.

Only one shell runs: amane hands a second launch to the one already running (it says
`amane: amane is already running, it takes this launch`) and exits.

## The socket

The shell listens on amane's IPC socket, `$XDG_RUNTIME_DIR/amane.sock`. A call is the command's
name on the first line and each argument on a line of its own; the caller then closes its sending
side, and the reply is text: `ok`, a usage line, or an error. A name the shell doesn't know
answers `no handler named <name>`.

```sh
printf 'launcher\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
```

From Rust, `amane::ipc_call("amane", "launcher", &["toggle".into()])` does the same. Shell
Settings (`amane-settings`) listens on `$XDG_RUNTIME_DIR/amane-settings.sock` and takes no
commands of its own; launching it again brings the open window forward.

## Commands

### `launcher`

`launcher [toggle | open | close] [apps | files | actions | clipboard]`
([launcher.md](../shell/launcher.md)). No arguments toggles it.

| Call | Does |
|---|---|
| `toggle` | opens the pill, or closes the launcher |
| `open` | opens the pill; nothing if it's open |
| `close` | closes it |
| `open <mode>` | opens it in that mode, or switches to it; asked for the mode already showing, it goes back to the pill |
| `toggle <mode>` | closes it when that mode shows, else opens it in that mode |

The modes also answer to `applications` (for `apps`) and `home` or `search` (the pill).

### `quick-settings`

`quick-settings [toggle | open [<page>] | close | edit | customize | reset-layout | controls | (show | hide) <control>]`
([quick-settings.md](../shell/quick-settings.md)). No arguments toggles it.

| Call | Does |
|---|---|
| `toggle`, `close` | opens or closes the card |
| `open` | opens it on the grid |
| `open <page>` | opens it on a page: `grid`, `sound`, `wifi`, `vpn`, `bluetooth`, `power-mode`, `power`, `night-light`, `do-not-disturb`, `keyboard`, `session`, `background-apps`, `tailscale`, `keep-awake`. A page the machine can't show answers `page <name> has nothing to show on this machine` |
| `edit`, `customize` | opens the grid in edit mode with the Customize window |
| `reset-layout` | puts back the default layout |
| `controls` | one line a control this machine has: its name, a tab, and `shown` or `hidden` (Shell Settings' Quick Settings page lists these) |
| `show <control>`, `hide <control>` | puts a control in the first free cells at its size (`no room for <control>: hide another control first` when none fit) or takes it off the canvas; the layout is saved |

### `notifications`

`notifications [toggle | clear | dnd [on | off | toggle]]`
([notifications.md](../shell/notifications.md)). No arguments toggles the notification center.

| Call | Does |
|---|---|
| `toggle` | opens or closes the notification center |
| `clear` | clears every notification |
| `dnd`, `dnd toggle` | switches Do Not Disturb |
| `dnd on`, `dnd off` | turns it on (until turned off) or off |

### `osd`

`osd <what> <how> [fine]` ([osd.md](../shell/osd.md)): the media and brightness keys. Each changes
the level or switch and shows the new one in the on-screen display; `fine` steps by 1 in place of 5.
The niri binds fall back to `wpctl` / `brightnessctl` unless the reply is `ok`.

| Call | Does |
|---|---|
| `volume up`, `volume down` | moves the output's volume 5% (up unmutes), clamped to 0–100 |
| `volume mute` | mutes or unmutes the output |
| `mic mute` | mutes or unmutes the microphone |
| `brightness up`, `brightness down` | moves the screen's brightness 5%, never below 1% |
| `kbd-light up`, `kbd-light down`, `kbd-light cycle` | moves the keyboard light a step; `cycle` goes from the brightest back to off |
| `airplane toggle` | blocks or unblocks every radio |
| `profile cycle` | the next power mode the daemon offers |
| `show volume`, `show mic`, `show brightness`, `show kbd-light` | shows the level as it is, changing nothing |

### `widgets`

`widgets list | add <kind> [size] [output] | remove <id> | reset-layout [output] | edit [output] | done`
([widgets.md](../shell/widgets.md)). Without an output, it's the focused workspace's monitor.

| Call | Does |
|---|---|
| `list` | one line a widget: monitor, id, kind, size, and its cell (`col,row`) or `hidden` |
| `add <kind> [size] [output]` | places one in the first free cells; the kinds are `cpu`, `memory`, `temperature`, `network`, `disk`, `system`, `battery`, `clock`, the sizes `small`, `medium`, `large`, `extra-large` (each kind takes some) |
| `remove <id>` | removes one, by the id `list` gives |
| `reset-layout [output]` | removes every widget from a monitor |
| `edit [output]` | opens Edit Widgets on a monitor |
| `done` | leaves Edit Widgets |

Errors start with `error:` (`no room for it`, `no monitor`, `no widget <id>`).

### `status-bar`

`status-bar (show | hide | toggle) <icon>` ([status-bar.md](../shell/status-bar.md#status-icons)):
shows or hides one of the optional status icons, `volume`, `bluetooth`, `network`, `tailscale`,
`battery`, `keep-awake`, `privacy`, `status-icons` or `notifications`, in `status-bar.json`.
`taskbar` is the same command under the bar's old name.

### `night-light`

`night-light [state | on | off | toggle | temperature <K> | schedule <when>]`
([displays.md](../shell/displays.md#night-light)), which Shell Settings' Displays page calls.
No arguments is `state`.

| Call | Does |
|---|---|
| `state` | answers like the state file: `on 4000 manual`, `off 3500 sun 59.92 10.75`, `on 4000 hours 21:00 07:00` |
| `on`, `off`, `toggle` | turns it on or off; under a schedule, on means it follows the schedule |
| `temperature <K>` | how warm, 2500–6000 K (clamped); the night's under a schedule |
| `schedule manual` | all day while on |
| `schedule sun <lat> <lon>` | from sunset to sunrise there |
| `schedule hours <HH:MM> <HH:MM>` | from the first time to the second, overnight when the second is earlier |

Without `wlsunset` it answers `wlsunset isn't installed`.

### Context menus

See [context-menu.md](../shell/context-menu.md#why-arming) for why a menu opened from outside is armed first.

| Call | Does |
|---|---|
| `arm [name]` | the next right-click opens the registered menu `name` (`file` by default) at the cursor |
| `open [name] [x] [y]` | opens it at a screen position (`file` at 240, 180 by default) |
| `close` | closes every open menu |

`file` is a sample menu `main.rs` registers.

### `alert-test`

`alert-test [stacked]` shows a sample alert whose answers do nothing, its buttons side by side or
stacked ([the alert](../shell/quick-settings.md#the-alert)). Release builds answer
`only in debug builds`.

## niri keybinds

A keybind spawns the call through a shell:

```kdl
binds {
    Mod+Space hotkey-overlay-title="Spotlight" {
        spawn "sh" "-c" "printf 'launcher\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
    Mod+Ctrl+V hotkey-overlay-title="Clipboard" {
        spawn "sh" "-c" "printf 'launcher\\ntoggle\\nclipboard\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
    Mod+S { spawn "sh" "-c" "printf 'quick-settings\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock"; }
    Mod+N { spawn "sh" "-c" "printf 'notifications\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock"; }
}
```
