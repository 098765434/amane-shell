# The shell's structure

How `apps/shell/src` is laid out, and the few rules that keep it that way. Tests hold the rules
(`surfaces/mod.rs`, `ipc.rs`, `services/state.rs`), so breaking one fails `cargo test`.

## Folders

| Folder | What goes there |
|---|---|
| `main.rs` | startup: logging, the agents and services that start once, the surfaces registered on the `App` |
| `ipc.rs` | the one table of IPC verbs: each read into the kit's `ShellCommand` and handed to its surface |
| `surfaces/<surface>/` | one surface: the bar, the dock, the launcher, notifications, Quick Settings, widgets, the password fill, the alert, the OSD |
| `chrome/` | what several surfaces share and only the shell has: which panel is open (`panel`), where the bar is (`bar`), per-monitor state (`per_output`), the battery glyph, the workspace submenu |
| `services/` | state only the shell has, and what it does with the system: the phone, Do Not Disturb, the clock, media art, the vault, power, network, display; and the helpers `state` (state files), `child_stream` (a program's output, kept going), `launch` |
| `tokens/` | what the shell generates from the design tokens (niri's layer rules) |

The menu, the shell IPC type, file writing and the rest of what apps share are the kit's
(`crates/kit`).

## A surface

A surface is a folder. Its `mod.rs` is its only face to the others: the surface itself (`surface`,
`bar`, `popup`), the functions other surfaces call (`launcher::open(Mode)`, `osd::step_volume`,
`quick_settings::open_alone`), its namespace, and its IPC entry (`run`). What it keeps and draws is
in files of its own, as it needs them:

| File | What |
|---|---|
| `mod.rs` | the surface, its public functions, its state (thread-locals, types) |
| `view.rs` | how it draws |
| `input.rs`, `keys.rs` | the pointer and the keyboard |
| `ipc.rs` | `run(command)`: its part of `ShellCommand` |
| others | what's particular to it (the launcher's `provider/`, the bar's `items.rs`) |

The rules, each a test in `surfaces/mod.rs`:

- No surface names another's modules (`surfaces::launcher::provider::…`): it uses what that
  surface's `mod.rs` offers.
- No surface calls another's IPC: they call each other's functions.
- `services/` and `chrome/` name no surface. A surface registers with `chrome` (as
  `panel::register`) rather than `chrome` calling it.
- No file under `surfaces/` is over 650 lines. Past that, split it along what it does: the
  functions move to a child module (`use super::*;` sees the parent's private items), the types
  and state stay.

## State, settings and logs

- **Settings** are the kit's `Config<T>` files in `~/.config/amane-shell/`, which Shell Settings
  writes too.
- **State** the shell keeps for itself is a `State` type (`services/state.rs`): JSON with a
  version, written whole, in `$XDG_STATE_HOME/amane-shell/`
  ([services.md](../integration/services.md#settings-and-state)). A file in an older format is
  read by the type's `legacy()` and left as it was.
- **Logs** go through the `log` crate. Each program calls `amane_log::init` once; `AMANE_LOG`
  (`warn`, `debug`, …) sets the level, `info` by default. A fallible call whose result is dropped
  says why in a comment, or logs.
