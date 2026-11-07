# amane-nixos

Shell Settings' NixOS model and jobs, with no UI. Settings' System pages
(`apps/settings/src/system/`) hold the state and draw it; this crate does what they
ask, off their UI thread, and keeps nothing between calls. The contract with the
repo is in [nixos.md](../../docs/integration/nixos.md).

| Module | What |
|---|---|
| `settings` | the flake's `amaneSettings` output read into `Entry`s, a value checked against its control, and `managed/settings.json`: what differs from HEAD and the commit's words |
| `job` | `Steps`: build as the user (nix's JSON log read into `Progress`), compare with the running system, activate as root through polkit, a fresh `flake.lock` for an update and putting it into the repo. One step at a time, stoppable from another thread |
| `lock` | a `flake.lock`'s root inputs and what a fresh one moves |
| `generations`, `repo` | the system profile's generations, what a system says about itself, a commit as a label shows it |
| `catalog` | the nixpkgs catalog the Apps page searches: built from `nix search --json`, searched a line at a time |
| `xkb` | the keyboard layouts `localectl` lists, for checking what's typed |

Every command goes through `Config` (the repo, the host, and the programs and
system paths: `nix`, `git`, `pkexec`, the activation helper, the profiles). Settings
builds one from `nixos.json` with `Config::new`; the tests put fake programs in their
place and run the pipeline through them: a build's progress and path, a failed and a
cancelled build, an update's lock and commit, activating (and polkit's cancel and
refusal), a rollback to an older generation, and the order of an Apply's steps.
Commands run through [`amane-process`](../process/README.md).
