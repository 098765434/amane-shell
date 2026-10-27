# NixOS from Shell Settings

Shell Settings' **System** pages manage the NixOS configuration the machine is built from
(`~/nixos-config`, a flake), the way a normal OS's settings app works: change a setting, press
Apply, done. It never edits the hand-written `.nix` files. The code is `apps/settings/src/system/`;
the pages are in [settings-app.md](../shell/settings-app.md#system).

## The contract with the repo

Three files in the repo, and one flake output, are what Settings reads and writes:

| In the repo | What |
|---|---|
| `managed/settings.json` | the values Settings owns, `{ "timeZone": "Europe/Oslo", "openssh": true, "sshdOnDemand": true, "dimAfter": 180, "packages": ["ripgrep"], "bootEntries": 10, … }`. The one file Settings writes (whole, through a rename), and the one it commits (with `flake.lock` after an update). Hand edits are fine. |
| `managed/registry.nix` | which settings may be there: a list of `{ id, path, page, label, control, tier, … }`. `path` is the NixOS option (`[ "services" "tailscale" "enable" ]`), `page` the Settings page that shows it (`about`, `general`, `services`, `power`, `apps`, `storage`, `update`), `control` how (`switch`; `int` with `min`/`max`; `choice` with `choices = [ { label; value; } … ]`; `timezone`; `text`, with `check` naming what Settings checks it against: `hostname`, `xkbLayout`, `xkbVariant` (`localectl list-x11-keymap-*`); `packages`, a list of attribute paths the Apps page draws), `tier` when it takes effect (`rebuild`, `restart`). A service entry may name its `unit` (`"sshd.service"`), `scope = "user"` for a user unit, and `onDemand`, the id of its "only when needed" switch. |
| `modules/managed.nix` | turns each key in the file into an option definition (`lib.mkIf (settings ? id)`), so a missing key leaves the option to the other modules. Also ships the activation helper and its polkit action. |
| `modules/options.nix` | the repo's own options for what has no option path of its own, so the registry can point at them: `amane.idle.{dimAfter,lockAfter,screenOffAfter}` (seconds, 0 = never) build hypridle's listener list; `amane.services.<unit>.onDemand` empties the unit's `wantedBy` (`mkIf` round `mkForce`, so an unset flag leaves the unit alone); `amane.packages` (dotted nixpkgs attribute paths) adds to `environment.systemPackages`, leaving out (with a NixOS warning) one this nixpkgs doesn't have, or has only as an alias that throws, so a nixpkgs update that drops a package doesn't fail the build; `amane.missingPackages` (read-only) lists those; `amane.updates.check` (`daily`, `weekly`, `never`) is the scheduled check's timer (below). |
| `amaneSettings` (flake output) | `nix eval --json .#amaneSettings`: `{ host, revision, entries, systemPackages }`, each entry the registry's fields plus `value` (effective), `default`, `type`, `description`, `setIn`, the repo files that define the option (`modules/managed.nix` when it's ours; empty when nothing sets it, since an option's default doesn't count), and `setAt`, `[ { file, line } ]` for each other repo file's definition, read off the module with `builtins.unsafeGetAttrPos` (empty when the module's shape hides it). `systemPackages` is what the repo's own files install, `{ name, version, file }` each; `missingPackages` is `amane.missingPackages`. |

A setting another file sets (`setIn` has more than `modules/managed.nix`) shows **locked**, with
the file and line: a value Settings wrote would clash with it or be overridden. `nix eval` sees
only files git knows about, so Settings runs `git add` after every write.

### Move to Settings

The one place Settings touches a `.nix` file. A locked control offers **Move to Settings…** when
`setAt` names exactly one line and that line is a plain `name = value;` (a comment after the
value is fine; `mkForce`, `lib.`, a value that spans lines or an attribute set aren't, and the
note then says to edit by hand). A dialog shows the file, line and text; **Move** replaces the
line with `# name is \`id\` in managed/settings.json (Shell Settings).`, writes the option's
current value under `id` in `settings.json` (so nothing changes), and commits both at once as
`move: Keyboard layout from configuration.nix`, revertable on its own. It refuses a file with
uncommitted changes, or a line that changed since the snapshot. A list option (hypridle's
listeners) is moved by hand: its definitions would merge, not override. A package moves the
same way from Apps' *Installed* list (**Move…**): the one line that lists it alone (`foot  #
terminal`, or `pkgs.foot`) becomes `# foot is in \`packages\` in managed/settings.json (Shell
Settings).`, the attribute joins `packages`, and the commit is `move: foot from
modules/apps.nix`; `amane.packages` installs it the same, so nothing changes. `system/adopt.rs`.

The repo's path and the `nixosConfigurations` attribute come from `nixos.json`
(`repo`, `host`; `~/nixos-config` and `nixos-niri` unless set), editable on About This Computer.

## Apply

```
settings.json (worktree ≠ HEAD)   ← what's waiting; Discard = git checkout -- managed/settings.json
   │ Apply… / Try
nix build --no-link --print-out-paths --log-format internal-json <repo>#nixosConfigurations.<host>.config.system.build.toplevel
   │ as the user; progress from nix's `@nix {…}` lines (the `builds` activity's done/expected)
nix store diff-closures /run/current-system <out>     → Changes; kernel/initrd/modules ≠ booted → "needs a restart"
   │ Apply Now / Apply at Restart / Try
pkexec amane-nixos-activate <switch|boot|test> <out>  → the polkit agent asks for the password
   │ switch or boot (a generation; test records nothing, a restart undoes it)
git commit -m "settings: Time zone → Europe/Oslo" -- managed/settings.json   never pushed
```

`amane-nixos-activate` (from `modules/managed.nix`) takes exactly a mode and a path, resolves
the path, and refuses with exit 2 anything that isn't a built NixOS system
(`/nix/store/<hash>-nixos-system-*` with a `bin/switch-to-configuration`); then `nix-env --set`s
the system profile (not for `test`) and runs `switch-to-configuration <mode>`. Polkit's action
`org.amane.nixos-activate` names its store path (`org.freedesktop.policykit.exec.path`), asks
`auth_admin_keep`, and says "Shell Settings wants to apply system settings". Root never
evaluates the repo. pkexec's exit 126 is a dismissed prompt, 127 not authorised.

A build runs on the app's own threads. Closing the window would end it (nix stops a build
whose client goes away), so the window asks first ("Stop the build?"). Cancel kills nix.

After a `switch` or `boot` of the working tree, Settings commits `managed/settings.json` alone
(a pathspec commit takes nothing else from the index) with a subject from the changes, writes
`$XDG_STATE_HOME/amane-shell/nixos/applied.json` `{ head, toplevel }`, and reloads everything.
A failed commit (a signing prompt, say) shows "Applied, but not committed".

Restore (History) builds nothing: it compares a generation's system with the running one and
activates it the same way. It commits nothing.

## Check for Updates

```
nix flake update [input…] --flake <repo> --output-lock-file $XDG_STATE_HOME/amane-shell/nixos/update.lock
   │ the repo's flake.lock is untouched; root inputs whose commit moved → "nixpkgs 26f4107 → 1a2b3c4"; none → "Already up to date"
nix build … --reference-lock-file <state>/update.lock <repo>#nixosConfigurations.<host>.config.system.build.toplevel
   │ then diff-closures and activation exactly as above (Try leaves the repo alone)
cp update.lock <repo>/flake.lock; git add flake.lock
git commit -m "update: nixpkgs 26f4107 → 1a2b3c4[; settings: …]" -- flake.lock managed/settings.json
```

The lock is written into the repo only after a `switch` or `boot` succeeded, so a failed or
cancelled check leaves no trace but `update.lock`. Settings waiting in `settings.json` go into
the same build and the same commit. A nixpkgs bump rebuilds whatever follows it (the shell,
niri) from source, which is minutes; the meter shows it.

## The scheduled check

`amane-update-check`, from `modules/options.nix`, runs as a user timer (`OnCalendar` daily or
weekly from `amane.updates.check`, persistent, with up to an hour's random delay; **Check
automatically** on Software Update). It relocks into `$XDG_STATE_HOME/amane-shell/nixos/check.lock`
(the repo's `flake.lock` is untouched), writes `check.json` beside it, and never builds:

```json
{ "checked": 1761500000, "changes": [ { "name": "nixpkgs", "from": "26f4107…", "to": "1a2b3c4…" } ] }
{ "checked": 1761500000, "changes": [], "error": "…the last line nix said…" }
```

When something moved and it isn't what was last notified (`notified.json`), it sends a toast
through the shell (`notify-send --action=default=Open "Updates available" "nixpkgs, amane-shell"`)
whose click runs `amane-settings --page update`. Both Settings (the card at the top of Software
Update, with **Update All**) and the shell (the Software Update tile in Quick Settings, from
Add Controls → Utilities, which draws only while something waits) watch `check.json`. Settings
rewrites it with no changes after an update is applied or a manual check finds nothing, so the
card and the tile clear before the next run. `systemctl --user start amane-update-check` runs
one now.

## The polkit agent

The shell is the session's polkit authentication agent (`amane::Polkit::serve_agent()`, from
`amane-shell`'s start), so the password prompt for `pkexec amane-nixos-activate`, `systemctl
start`, Flatpak and every other privileged action is the shell's own alert: the action's
message, a password field, "Sorry, that didn't work" after a wrong one (three tries), Cancel
or Escape to refuse. It registers `/org/amane/PolicyKit1/AuthenticationAgent` for the
`unix-session` logind names as the user's display (the shell runs as a user service, outside
the session's scope), and registers again when polkitd restarts. Polkit 124+ runs its PAM
helper as a system service behind `/run/polkit/agent-helper.socket`; the agent connects, writes
the user and the cookie, relays the helper's `PAM_PROMPT_ECHO_OFF` / `PAM_TEXT_INFO` /
`PAM_ERROR_MSG` lines and the answer, and the helper itself reports to polkitd. The NixOS
configuration keeps `security.polkit.enable` and turns the niri flake's `niri-flake-polkit`
(polkit-kde) user service off. amane's `docs/fork/system/polkit.md` has the agent.

## Now, not after Apply

Some things on the System pages act at once and aren't part of the configuration: **Start** /
**Stop** on Services (`systemctl [--user] start|stop <unit>`; systemd asks through polkit, and
a restart puts the unit back to what the configuration says), Flatpak's **Uninstall…**
(`flatpak uninstall -y --noninteractive <id>`), and the catalog behind **Add App**
(`nix search --json --inputs-from <repo> nixpkgs ^`, once per nixpkgs revision, which reads
the configuration's own nixpkgs and fills nix's evaluation cache the first time, for minutes;
`$XDG_CACHE_HOME/amane-shell/nixpkgs/<rev>.tsv`).

## "Up to date"

About This Computer says whether the running system is the configuration, from the quick
signs to the exact one:

1. `settings.json` differs from HEAD: *n settings changed, not applied*.
2. Other tracked files are dirty (`git status --porcelain`): *… has uncommitted changes*.
3. The running system's `configurationRevision` (`nixos-version --json`; the flake sets
   `system.configurationRevision = self.rev or self.dirtyRev`) isn't HEAD: *n commits not
   applied*. A system built from a dirty tree records `<sha>-dirty`, and the commit Settings
   makes afterwards moves HEAD, so `applied.json` matching HEAD and `/run/current-system` counts
   as up to date.
4. The working tree evaluated (`nix eval --raw …toplevel`, seconds, in the background) isn't
   `/run/current-system`: *the configuration differs from the running system*. This also clears
   3 when a commit changed nothing in the system.

A running system with no revision recorded says so: apply once to know.

## Limits (phases 1–4)

- A new setting is a line in the registry (and **Move to Settings…**, or a line in
  `settings.json` in place of the hand-written one); a new kind of control is an arm in
  `system/controls.rs`.
- The window can't close during a build. Moving the build to a `systemd-run --user` unit is
  later work behind the same `BuildJob` API.
- `switch-to-configuration dry-activate` needs root, so which services restart isn't shown.
- The language list is the registry's own (glibc knows more: edit `settings.json` by hand for
  one it lacks); keyboard layouts and variants are checked but not described.
- The scheduled check needs the network; offline it only records why.
