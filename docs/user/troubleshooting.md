# Troubleshooting

**Start with the log.** The shell and its apps log to the journal:

```sh
journalctl --user -u amane-shell -b          # the shell, this boot
journalctl --user -u amane-drop -b           # Drop
journalctl --user -u amane-assistant -b      # the assistant's daemon
```

`AMANE_LOG=debug` in the unit's environment says more
([configure.md](configure.md#environment-variables)). Run from a terminal, the shell logs to
stderr instead.

## The bar and the panels aren't frosted

The shell draws its surfaces; **niri** frosts them, through `layer-rule` blocks the shell
generates. Plain, square, unblurred surfaces mean the rules aren't in niri's config, or are
stale.

1. Check the marker lines are in your niri config and filled:
   `grep -c 'amane layer-rules' ~/.config/niri/config.kdl` should say `2`, with rules between.
2. Run `./gen-layer-rules.sh ~/.config/niri/config.kdl` from the repository, then
   `niri msg action load-config-file`. If your config is generated (home-manager), pass the
   source file instead and rebuild.
3. `niri msg layers | grep amane` lists the shell's surfaces; each should have a rule.

The frost itself needs the niri fork: stock niri has no `materials {}` and no `desktop` layer,
so on it the shell lights its edges itself and goes without blur
([layer-rules.md](../developer/integration/layer-rules.md)).

## A program's item is missing from the bar

Status apps (Drop, train-watch, your own) put their items up over `org.amane.StatusBar1`.

- **Drop started before the shell.** Its item comes back by itself once the bar owns the name;
  nothing to do but wait a second. If it doesn't, `systemctl --user restart amane-drop`.
- **Another program owns the name.** The shell logs `another program serves
  org.amane.StatusBar1; status apps go there` and leaves them to it. `busctl --user status org.amane.StatusBar1` says who.
- **The item is hidden.** Shell Settings › Status Bar lists programs' items with a switch each;
  `status-bar apps list` over the socket says `hidden`.
- **It's an Activity, not an item**, and the app's Activities are off in Shell Settings › Status
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
`$XDG_RUNTIME_DIR/amane-assistant.sock` and stopped after five minutes idle.

- `systemctl --user status amane-assistant.socket amane-assistant.service`: the socket should be
  active (listening) even while the service is stopped.
- `printf 'assistant\nstatus\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock` answers what the daemon
  runs (`cactus 4.0.0 (3)`), or `not connected`.
- The models come from the flake on x86_64-linux only; elsewhere the daemon builds without an
  engine and only the rules answer ([assistant.md](features/assistant.md#the-daemon)).
- Speech needs a microphone PipeWire can see; the bar's microphone indicator shows while it
  listens.

## Passwords don't fill

- `printf 'passwords\nstatus\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock` says whether the vault
  is configured, unlocked and synced; Shell Settings › Passwords does the same.
- Fill needs the niri fork, which tells the shell which window has the field. Browsers are left
  out on purpose: their Bitwarden extension checks the website itself
  ([passwords.md](features/passwords.md)).

## The shell restarted and my apps closed

Programs started from the dock or the launcher run in scopes of their own and survive a shell
restart. A program started *by* the shell's unit (a status app put in `ExecStartPost`, say)
shares its cgroup and goes with it: give it a unit of its own
([install.md](install.md#2-the-service)).

## Settings don't take

The shell follows `~/.config/amane-shell/*.json` as they change. A file that fails to parse
is logged (`couldn't read appearance.json: …`) and the defaults are used until it parses
again. `AMANE_*` variables in the unit's environment win over the file while set
([configure.md](configure.md)).
