# Security

## Supported versions

Only `main` is supported. Fixes land there; there are no maintenance branches, and a tagged
release gets a fix by a new tag from `main`.

## Reporting a vulnerability

Report it privately through GitHub's vulnerability reporting on this repository: the
**Security** tab, then **Report a vulnerability**. Do not open a public issue or pull request
for it.

Say what the bug lets someone do, how to reproduce it, and which revision you found it on
(the commit, or the `emaki` input's `rev` in your `flake.lock`).

You will hear back within 7 days, with whether it is taken on and what happens next. Once a
fix is on `main`, the advisory is published with credit to you, unless you ask otherwise.

## Scope

The shell runs with the session's rights and holds a few of its keys, so bugs in these are
in scope, whichever program of this repository has them:

- **Privilege.** The shell is the session's **polkit agent**: its dialog authorizes other
  programs' privileged actions, and Emaki Settings' System pages build and activate the
  machine's NixOS configuration through polkit. Anything that lets an action through without
  the user's answer, or with another program's, counts.
- **Credentials.** The shell fills passwords from **rbw** (Bitwarden) into other apps'
  fields, and `emaki-pinentry` passes the vault's master password. A secret shown to the
  wrong window, kept where another user or program can read it, passed on a command line,
  or written to a log counts.
- **Other programs' input.** Anything a program on the session bus (`org.emaki.StatusBar1`),
  the IPC socket (`$XDG_RUNTIME_DIR/emaki.sock`), a notification or a dropped file can make
  the shell do that it shouldn't.
- **The apps' secrets.** Drop's upload accounts, Messages' and Phone Screen's pairing with
  the phone, and the assistant's microphone.

- **amane's part in these.** The polkit agent and the PAM conversation the shell runs come from
  the amane fork, and a bug there that reaches any of the above counts here too (or report it
  to [emaki-desktop/amane](https://github.com/emaki-desktop/amane/security)).

Out of scope: the niri fork (report to
[emaki-desktop/niri](https://github.com/emaki-desktop/niri/security)), amane's code that none of
the above reaches, and a bug that needs the user's own account on the machine to already be
compromised. A program running as the user is not by itself a compromised account: one should
not be able to get root through Emaki Settings' helper, read the vault's master password, or
take the assistant's microphone.

## What's trusted

- **The Nix store, by exact path.** The shell, emaki-pinentry and the assistant's daemon trust
  each other only from the same build, and rbw-agent only from the rbw the shell was built with,
  by store path. Anyone can put a program of any name in the store, so being in it proves
  nothing.
- **Cactus.** The assistant links Cactus's prebuilt `libcactus.a` (pinned by hash in
  `flake.nix`), which isn't built from source. The daemon that runs it has no network, takes
  connections only from the shell, and confines its file access (Landlock).
- **The system's tools.** nix, git, systemctl and the rest are run from the system or user
  profile, never from `PATH`; the tools that handle secrets or untrusted data (curl,
  secret-tool, wl-copy) are pinned by store path (`crates/base/src/tools.rs`).

## How the code holds up

The defences a report may find a way around, so a reporter knows what was meant to stop it:

- **Root.** The activation helper (`nix/nixos-module.nix`) only activates a valid NixOS system
  path, asks for the password every time (`auth_admin`), and the shell's polkit dialog shows the
  command line, store path included.
- **Secrets.** Kept in memory that's wiped when dropped, never in an argument or a log; no core
  dumps of the programs holding them; a plain-text fallback for Drop's passwords only when the
  user agrees.
- **Other programs' input.** The IPC socket, the status bar's D-Bus API, notifications, tray
  icons, images, the clipboard, drops, Phone Screen's stream and album art from the web each
  have a size cap, and a peer check where one is possible; a dropped file's name is never read
  by a shell; a new Bitwarden server is confirmed in the shell's own dialog.
- **Sandboxing.** The assistant's unit (`nix/home-module.nix`) and the threads that decode
  another device's data are confined; the shell and Drop are not, as the apps they open would
  inherit it.
- **Checks.** `cargo deny check` (advisories, licences, sources) and fuzzing of the parsers in
  `fuzz/` run in CI.
