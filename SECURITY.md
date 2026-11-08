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

Out of scope: the niri and amane forks (report to
[emaki-desktop/niri](https://github.com/emaki-desktop/niri/security) and
[emaki-desktop/amane](https://github.com/emaki-desktop/amane/security)), and a bug that needs the
user's own account on the machine to already be compromised.
