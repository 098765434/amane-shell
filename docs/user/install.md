# Installing emaki

**emaki is built and run through its Nix flake.** The flake carries the Rust toolchain,
the native libraries, the GPU driver wiring and every program in the workspace; nothing else
is needed on the machine but niri.

## Before you start

- **The niri fork**, [098765434/niri](https://github.com/098765434/niri) (branch `ocbk`).
  The shell asks niri which fork features exist and leaves out what stock niri can't do,
  but the frost, the desktop layer the widgets sit on, the dock rising with the overview and
  reordering windows from the dock all need the fork. What it adds is in
  [its docs](https://github.com/098765434/niri/blob/ocbk/docs/fork/README.md).
- **A Vulkan driver.** Everything is drawn on the GPU through wgpu. On NixOS the system's
  drivers are under `/run/opengl-driver`, and the installed programs find them by themselves.
- **Nix with flakes** on.

> **Do not build this shell with the upstream `amane` CLI** (`amane dev`, `amane compile`). It
> compiles against its own bundled copy of the library, not the fork this shell depends on,
> and the shell won't compile. The flake is the one build path.

## Build and run

```sh
nix develop -c cargo build                       # compile
nix develop -c cargo run                         # run, in a niri session
nix develop -c env EMAKI_THEME=light cargo run   # the light theme, for once
nix build                                        # a release build: ./result/bin/emaki
```

`nix develop` is a shell with the toolchain and libraries wired up, so plain `cargo` works in
it. `nix build` leaves every program in `./result/bin`, each wrapped with the runtime libraries
it needs, so they run outside the dev shell.

The `amane` library is a git dependency pinned by revision in `Cargo.toml`. To work on the
shell and the library together, see [CONTRIBUTING.md](../../CONTRIBUTING.md#development).

## Install it on NixOS

The package comes from this repository's flake. There is no nixpkgs overlay, so
`pkgs.emaki` doesn't exist; add the flake as an input and take its package.

### 1. The flake input

```nix
inputs.emaki.url = "github:098765434/amane-shell";
```

Pass `inputs` to your home-manager module (`extraSpecialArgs = { inherit inputs; }`), so the
package is reachable as `inputs.emaki.packages.${pkgs.system}.default`. The same input
builds Emaki Settings, Messages, Phone Screen, Shader Lab, Drop and the assistant's daemon.

### 2. The service

A **systemd user service** is the robust way to run the shell: it restarts after a crash and
the journal keeps its log.

```nix
{ inputs, pkgs, ... }:
let emaki = inputs.emaki.packages.${pkgs.system}.default;
in {
  home.packages = [ emaki ];   # the apps, their desktop entries and icons

  systemd.user.services.emaki = {
    Unit = {
      Description = "emaki";
      PartOf = [ "graphical-session.target" ];
      After = [ "graphical-session.target" ];
    };
    Service = {
      ExecStart = "${emaki}/bin/emaki";
      Restart = "on-failure";
    };
    Install.WantedBy = [ "graphical-session.target" ];
  };
}
```

A niri `spawn-at-startup "emaki"` line works too, without crash recovery.

Run the **status apps** as units of their own, not from the shell: a restart of the shell kills
whatever is in its cgroup, and they put their items back by themselves once the bar is there.
Drop's unit and keybind are in [drop.md](features/drop.md#installing); the assistant's daemon
is socket-activated ([assistant.md](features/assistant.md#the-daemon)).

### 3. niri's layer rules

The shell's glass is frosted by niri, through `layer-rule` blocks generated from the design
tokens. Put the two marker lines once into your niri config, where the rules should go:

```kdl
// >>> emaki layer-rules (generated from shell/niri-layer-rule.kdl — do not edit by hand)
// <<< emaki layer-rules
```

Then fill them from the repository:

```sh
./gen-layer-rules.sh ~/.config/niri/config.kdl   # or the file your config is generated from
niri validate -c ~/.config/niri/config.kdl
niri msg action load-config-file
```

The script is idempotent: run it again after every shell update, since a new surface comes
with a new rule. Without the rules the shell still runs, but its surfaces are plain, not
frosted. The details, and what each rule does, are in
[layer-rules.md](../developer/integration/layer-rules.md).

### 4. Keybinds

The shell takes commands over its socket, so a niri keybind is a one-liner. The ones people
copy are in [keyboard-and-ipc.md](keyboard-and-ipc.md).

## Updating

In your own flake, `nix flake update emaki`, rebuild and switch. A switch that restarts
`emaki.service` restarts the bar; programs started from the dock or the launcher live in
scopes of their own, so they survive it. Back up `~/.config/emaki` and
`~/.local/state/emaki` before a switch that changes a file format
([release.md](../developer/release.md#nixos-config-and-the-switch) has the command).

A niri update takes effect at the next login, not at the switch: niri is the session.

## Where to go next

- [configure.md](configure.md): the settings files, the environment variables, theme and density.
- [keyboard-and-ipc.md](keyboard-and-ipc.md): the keybinds and the socket.
- [troubleshooting.md](troubleshooting.md): when something doesn't show, or shows unfrosted.
