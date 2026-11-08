# Troubleshooting

**Start with the log.** The shell and its apps log to the journal:

```sh
journalctl --user -u emaki -b          # the shell, this boot
journalctl --user -u emaki-drop -b           # Drop
journalctl --user -u emaki-assistant -b      # the assistant's daemon
```

`EMAKI_LOG=debug` in the unit's environment says more
([configure.md](configure.md#environment-variables)). Run from a terminal, the shell logs to
stderr instead.

## The bar and the panels aren't frosted

The shell draws its surfaces; **niri** frosts them, through `layer-rule` blocks the shell
generates. Plain, square, unblurred surfaces mean the rules aren't in niri's config, or are
stale.

1. Check the marker lines are in your niri config and filled:
   `grep -c 'emaki layer-rules' ~/.config/niri/config.kdl` should say `2`, with rules between.
2. Run `./gen-layer-rules.sh ~/.config/niri/config.kdl` from the repository, then
   `niri msg action load-config-file`. If your config is generated (home-manager), pass the
   source file instead and rebuild.
3. `niri msg layers | grep emaki` lists the shell's surfaces; each should have a rule.

The frost itself needs the niri fork: stock niri has no `materials {}` and no `desktop` layer,
so on it the shell lights its edges itself and goes without blur
([layer-rules.md](../developer/integration/layer-rules.md)).

## A program's item is missing from the bar

Status apps (Drop, train-watch, your own) put their items up over `org.emaki.StatusBar1`.

- **Drop started before the shell.** Its item comes back by itself once the bar owns the name;
  nothing to do but wait a second. If it doesn't, `systemctl --user restart emaki-drop`.
- **Another program owns the name.** The shell logs `another program serves
  org.emaki.StatusBar1; status apps go there` and leaves them to it. `busctl --user status org.emaki.StatusBar1` says who.
- **The item is hidden.** Emaki Settings › Status Bar lists programs' items with a switch each;
  `status-bar apps list` over the socket says `hidden`.
- **It's an Activity, not an item**, and the app's Activities are off in Emaki Settings › Status
  Bar › Activities.

Testing a status app beside the running desktop is in
[status-apps.md](../developer/integration/status-apps.md#testing).

## Nothing draws, or the log names Vulkan

Everything is drawn through wgpu on Vulkan. The installed programs look for the system's
drivers under `/run/opengl-driver/share/vulkan/icd.d`, and name the Radeon driver alone when
there is one (the others cost memory).

- `vulkaninfo --summary` should list your GPU. If it lists only `llvmpipe`, the driver isn't
  installed or isn't found; on NixOS, `hardware.graphics.enable = true` and the driver package.
- To point the loader somewhere else, set `VK_ICD_FILENAMES` in the unit's environment; the
  wrapper leaves a value you set alone.
- `WGPU_BACKEND=vulkan` is set by the wrapper so wgpu doesn't probe GL; a stack that has only
  GL isn't supported.
- A second GPU with no screen on it can be the one picked; `VK_ICD_FILENAMES` naming the one
  driver settles it.

## The assistant doesn't answer

The launcher's assistant is a daemon beside the shell, started by a **socket unit** on
`$XDG_RUNTIME_DIR/emaki-assistant.sock` and stopped after five minutes idle.

- `systemctl --user status emaki-assistant.socket emaki-assistant.service`: the socket should be
  active (listening) even while the service is stopped.
- `printf 'assistant\nstatus\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock` answers what the daemon
  runs (`cactus 4.0.0 (3)`), or `not connected`.
- The models come from the flake on x86_64-linux only; elsewhere the daemon builds without an
  engine and only the rules answer ([assistant.md](features/assistant.md#the-daemon)).
- Speech needs a microphone PipeWire can see; the bar's microphone indicator shows while it
  listens.

## Passwords don't fill

- `printf 'passwords\nstatus\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock` says whether the vault
  is configured, unlocked and synced; Emaki Settings › Passwords does the same.
- Fill needs the niri fork, which tells the shell which window has the field. Browsers are left
  out on purpose: their Bitwarden extension checks the website itself
  ([passwords.md](features/passwords.md)).

## The shell restarted and my apps closed

Programs started from the dock or the launcher run in scopes of their own and survive a shell
restart. A program started *by* the shell's unit (a status app put in `ExecStartPost`, say)
shares its cgroup and goes with it: give it a unit of its own
([install.md](install.md#2-the-service)).

## Settings don't take

The shell follows `~/.config/emaki/*.json` as they change. A file that fails to parse
is logged (`couldn't read appearance.json: …`) and the defaults are used until it parses
again. `EMAKI_*` variables in the unit's environment win over the file while set
([configure.md](configure.md)).

## Migrating from amane-shell

Until 2026-11 the shell was amane-shell and its programs `amane-*`; they are emaki and
`emaki-*` now (amane, the UI library underneath, keeps its name). Every program moves its own
folders on its first start, once:

| Before | Now |
|---|---|
| `~/.config/amane-shell`, `~/.local/state/amane-shell`, `~/.cache/amane-shell` | `~/.config/emaki`, `~/.local/state/emaki`, `~/.cache/emaki` |
| `~/.config/amane-drop`, `~/.local/state/amane-drop`, the other `amane-*` folders | `emaki-drop` and the other `emaki-*` |
| `~/.cache/amane/messages` (the phone's pictures) | `~/.cache/emaki/phone/messages` |
| `~/.local/share/amane-assistant/models` | `~/.local/share/emaki-assistant/models` |

When both a folder and its new name are there, what the new one lacks moves in and the rest
stays where it was, named in the log (`journalctl --user -u emaki | grep migrate`). The ids the
settings keep are renamed with them (`org.amane.drop` in the bar's order, `amane-settings.desktop`
in the dock's pins), and `~/.config/emaki/.migrated-from-amane` marks it done. Passwords kept in
the keyring under the old names are still found.

`scripts/migrate-from-amane` does the same moves by hand, saying what it would do unless given
`--apply`. Given the niri config, it also renames what no program edits there:

```sh
scripts/migrate-from-amane --apply ~/.config/niri/config.kdl
./gen-layer-rules.sh ~/.config/niri/config.kdl
niri msg action load-config-file
```

- the layer-rules markers, `// >>> amane layer-rules` and `// <<< amane layer-rules`, are
  `emaki layer-rules`; `gen-layer-rules.sh` then writes the rules for the new namespaces
  (`emaki.dock`, `emaki.status-bar`, …) and app ids (`emaki-settings`)
- the sockets in binds and scripts, `amane.sock` and `amane-drop.sock`, are `emaki.sock` and
  `emaki-drop.sock`
- the `AMANE_*` variables the shell reads are `EMAKI_*` (`EMAKI_THEME`, `EMAKI_LOG`, …); the
  library's own, like `AMANE_DEBUG`, keep their names ([configure.md](configure.md))
- the bar's D-Bus name is `org.emaki.StatusBar1`, with no alias: a status app of your own
  changes its destination

On NixOS, nixos-config renames along with it: the flake input `amane-shell` is `emaki`, its
package `inputs.emaki.packages.${system}.default`, the `amane.*` options are `emaki.*`, and the
units `amane-shell.service`, `amane-drop.service` and `amane-update-check` are `emaki.service`,
`emaki-drop.service` and `emaki-update-check` ([install.md](install.md)).
