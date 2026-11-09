# Passwords (Bitwarden fill)

Bitwarden logins, filled into password fields in any app:
a small popup by the field lists that app's entries, and a click fills one in. The vault is
[rbw](https://github.com/doy/rbw), which talks to bitwarden.com or a self-hosted Vaultwarden. The
shell is the seat's input method (`zwp_input_method_v2`), so it sees which field has the keyboard
in any app. niri's fork checks every fill before it lets the text in.

## Usage

**Setting up.** Open Emaki Settings › Passwords.
1. Give the **server** (`https://…`; plain `http` only to `localhost`) and the **email**, then
   **Save**. This also points rbw's `pinentry` at `emaki-pinentry`. A new server is confirmed in
   the shell's own dialog (**Use This Bitwarden Server?**, naming its host), since it's where the
   next unlock sends the master password; Return doesn't answer it, so **Use Server** is a click.
2. **Sign In…** opens the shell's dialog for the master password, and a two-factor code if the
   account has one.
3. bitwarden.com may ask to register the device. **Register…** then appears and asks for the
   account's API key. Vaultwarden usually doesn't ask.

**Filling.** Focus a password field in an app, and the popup shows below it (or above, near the
bottom of the screen):
- **Header:** whose field it is. "Passwords for Telegram" for an app the shell checked, or
  "Unverified app · why" for one it couldn't (see [Who owns a field](#who-owns-a-field)).
- **Entry rows:** up to five for the app. A click fills the password. On a username field it
  fills the username, and on a code field (Digits/Pin) the two-factor code.
- **Unlock Bitwarden…** shows instead while the vault is locked, and **Sign in…** while
  there's no account.
- **Search vault…** opens the launcher's Passwords mode.
- **Remember the choice for <app>:** on by default. A filled entry is then linked to the app
  and comes first next time.
- **×** closes it for this field.

A field that already holds text asks once ("fill anyway?"). The text goes in at the cursor.

| Field | The popup shows |
|---|---|
| Password, or hidden sensitive text | always |
| Username, email, other text | only with **Suggest on username fields** on, and an entry remembered or named for the app |
| Digits, PIN (empty) | with an entry remembered or named for the app |
| Terminal | only on `passwords suggest` |
| The shell's own fields, browsers, apps set aside | never |

**The launcher.** Type `pw ` and a name in the launcher, or open the Passwords mode
(`launcher open passwords`). **Enter** fills the password into the window behind the launcher,
once that window has the keyboard back. The best match also offers **Fill username**, **Fill
two-factor code** and **Copy password**. A fill from the launcher goes through the popup's checks
too:
- the app must be verified, and not a browser or one set aside;
- a password goes only into a password field.

When it can't fill, a notification says why, and nothing is copied unless you pick **Copy
password**. A copy is made while the launcher still has the keyboard, as the compositor only takes
a copy from the focused app. It's marked `x-kde-passwordManagerHint`, which wl-clipboard turns
into `CLIPBOARD_STATE=sensitive`, so cliphist and the shell's own history leave it out. It's
cleared after 30 seconds.

**Settings** (Emaki Settings › Passwords):

| Setting | Default | |
|---|---|---|
| Lock after | 15 min | rbw's `lock_timeout` |
| Lock with the screen | on | also locks before sleep |
| Fill passwords in apps | on | takes effect when the shell starts |
| Suggest on username fields | off | |
| Passwords in the Launcher | on | |
| Hide while the screen is shared | on | |

The page also lists the entries remembered for apps, each with ×, and the apps never offered
fill. Those are browsers by default, as their Bitwarden extension checks the website itself.
The settings are `~/.config/emaki/passwords.json`, mode 0600.

**IPC.** `printf 'passwords\nsuggest\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock` shows the
popup for the field with the keyboard, even a username or terminal field. Bind it in niri for
a keyboard way in. Also: `status`, `unlock`, `sign-in`, `register`, `sync`, `lock`,
`configure <server> <email> [forget]` and `lock-timeout <seconds>`. `status` prints `key=value`
lines and never a secret. Any program of yours can write to the socket, so `configure` with a
server other than the current one answers `confirm` and asks in the shell's dialog instead.

### Apps that work

| App | Fill |
|---|---|
| GTK 3/4, Qt 6, emaki apps | yes |
| Firefox | yes (text-input-v3), but set aside by default |
| Chromium, Electron (Signal, Element, VS Code…) | with `--enable-wayland-ime --wayland-text-input-version=3` |
| X11 apps (xwayland-satellite) | no input method: the launcher copies instead |
| Sandboxed (Flatpak) | unverified: Search vault… only |

## Threat model

**Kept from:**
- **The wrong app or window.** niri fills only the same text input of the same surface that was
  checked, in one step (see [How a fill is checked](#how-a-fill-is-checked)).
- **An app pretending to be another.** It can claim any app id, but its program is checked (see
  [Who owns a field](#who-owns-a-field)).
- **Web pages and sandboxed apps.** Browsers are left out, and a sandboxed app is never
  verified. Window titles are never matched.
- **Leaks:**
  - *Logs and arguments:* no secret in a log or in any program's arguments.
  - *D-Bus:* no secret on the session bus, which any program of yours can watch.
  - *Clipboard:* passwords are copied only on request, marked, and cleared.
  - *Core dumps:* the shell sets `RLIMIT_CORE=0` for itself and for rbw (and so for the agent
    it starts), and its unit adds `LimitCORE=0`.
  - *Swap:* a password lives in the shell only between `rbw get` and the fill, sized once and
    wiped after. Nothing is `mlock`ed, as the copies past the shell (the Wayland message, niri,
    the app) can't be. This machine's swap is zram, in memory, never on disk.
  - *Undo history:* none for password fields.
  - *Other input methods:* amane's password fields send them no surrounding text.
- **Phishing with the shell's own dialog.** It comes up only for an rbw command the shell
  started, asked by `emaki-pinentry` under that user's `rbw-agent` (see
  [The master password](#the-master-password)).
- **Screen sharing.** Passwords are never drawn, and the popup hides while the screen is shared.

**Not kept from: programs running as you.** While the vault is unlocked, any of them can ask
`rbw-agent` for any password over its socket, as with ssh-agent and gpg-agent. They could also
change rbw's config file. Bitwarden's desktop app and KeePassXC have the same limit. It's
narrowed by locking: after 15 minutes unused, with the screen, and before sleep. The agent also
stops with the shell, as it runs in the shell's unit.

## Design / internals

### How a fill is checked

1. The shell is the input method only to *learn* what kind of field has the keyboard (its
   content purpose and hints), and to place the popup (`zwp_input_popup_surface_v2`). It
   never types through it: `commit_string` would go to whichever text input is focused when
   the request arrives, so a password meant for one app could reach another.
2. For each new field, amane takes a **target** from the niri fork's `niri-secure-fill-v1`.
   That's niri's snapshot of the field and its owner.
3. On a click, the password is read from rbw (`rbw get -- <uuid>`) into a buffer that wipes
   itself, and sent to that target.
4. niri commits it only if the same text input of the same surface still has the keyboard,
   the session isn't locked, and the target is fresh and unused. Otherwise it drops the text.
   The popup then says why ("The field changed, so nothing was filled").

The popup never takes the keyboard. There's no grab and no virtual keyboard, so the app keeps
every key.

Code:
- amane: `src/wayland/input_method_v2.rs`, `src/wayland/secure_fill.rs`,
  `src/services/focused_field.rs` (amane's `docs/fork/input/input-method.md`);
- niri: `src/protocols/secure_fill.rs` (niri's `docs/fork/input/secure-fill.md`);
- shell: `apps/shell/src/surfaces/fill/`.

### Who owns a field

`apps/shell/src/services/vault/identity.rs`. niri reports two things about the field's owner:
- the process on its socket (SO_PEERCRED) and what that process runs (`/proc/<pid>/exe`);
- the app id its window claims.

The app id is believed only if all of these hold:
- a desktop entry `<app id>.desktop` (or one with that `StartupWMClass`) is found, first in
  the data folders, as for launching;
- the entry isn't in the home folder, as its path is found, not where a link leads (the user's
  programs can write there), and it lives in the Nix store;
- the program is that entry's own store path, or its `<name>-unwrapped`/`<name>-bin` twin that
  the package refers to directly (`nix-store --query --references`). Any other part of its
  closure is refused: glibc's loader (`ld-linux.so ./evil`), a shared python or electron are in
  many;
- the program isn't a loader, an interpreter or a shell, which run whatever they're handed;
- niri saw the process in its own mount and user namespaces, with its root at `/`. Otherwise
  it's a sandbox, and its path says nothing.

A program from the home folder, an app id nothing installed provides, an entry shadowing the
system's from the home folder, a shared runtime (Electron apps on the system electron), or a
sandbox: each is unverified. An unverified app gets no suggestions and nothing remembered. An
entry with `Categories=WebBrowser`, or one that opens http links, is verified *as a browser*,
and fill stays out of it.

A check is kept for 10 minutes. A failed `nix-store` query isn't kept, and at most 256 owners
are.

### Matching

`apps/shell/src/services/vault/matching.rs`, for verified apps only, best first:
1. **Entries remembered for the app.** They're keyed by its desktop id *and* package, so
   another package claiming the same id gets none.
2. **Entries naming the app.** That's `app://<desktop id>`, or `androidapp://<app id>` as
   Bitwarden's phone apps save.
3. **The same site.** A reverse-DNS desktop id is read as a domain (org.telegram.desktop is
   telegram.org), and matched against an entry's website on that domain. These are shown,
   never first.

rbw's list carries no match types, so websites match by domain. Most entries in a vault have no
address at all, so remembering does most of the work over time.

### The master password

rbw asks a `pinentry` program over the Assuan protocol. `emaki-pinentry` (`crates/pinentry`) is
that program.
1. It connects to `$XDG_RUNTIME_DIR/emaki/pinentry.sock` (folder 0700, socket 0600) and
   checks the other end: the same user, running `emaki` from the same Nix store package as this
   pinentry (the same build).
2. The shell (`apps/shell/src/services/vault/pinentry.rs`) answers a caller only if all of these hold:
   - the caller is `emaki-pinentry` from the shell's own store package;
   - its parent is `rbw-agent` from the store package of the rbw the shell runs (`EMAKI_RBW`);
   - that agent has the shell's own `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`,
     `XDG_RUNTIME_DIR` and `RBW_PROFILE`, so it signs in to the account the page shows;
   - an rbw command the shell started, that can ask, is still running (for at most two
     minutes; the window closes as the command ends);
   - the caller's pidfd (SO_PEERPIDFD) still points at it once all that was read.
3. The dialog (`apps/shell/src/surfaces/alert/vault_unlock.rs`) has a title the shell sets, with rbw's
   words under it as plain text, and names the server's host, so a changed server can't pass
   unseen. Its field is taken out (`TextInput::take_secret`) and left
   empty. The answer goes back over the socket and is wiped.
4. If the shell isn't there or says no, the pinentry runs pinentry-gnome3 in its place,
   before it has answered rbw at all. The store path of pinentry-gnome3 is fixed when it's
   built. After a rebuild, until the shell restarts, the profile's pinentry is a newer build than
   the running shell, so the two don't trust each other and unlocking uses pinentry-gnome3.
5. Being in the Nix store proves nothing by itself (anyone can `nix build` a program of any
   name there), so only the exact package counts. A debug build from a checkout, which has no
   package, also takes its own `target` folder and the store.
6. Once the shell has agreed, the pinentry confines its file access (Landlock) to nothing: it
   only passes the pin between rbw and the socket, which are open already. It turns its core
   dumps off; it stays readable to the shell, which checks it through `/proc`.

D-Bus isn't used, as any program of yours can watch the session bus.

### rbw

`apps/shell/src/services/vault/rbw.rs`:
- **Commands.** rbw runs by its store path (`EMAKI_RBW`, set by the flake), with no secret in
  an argument, and an entry's id checked to be a UUID so it can't pass for an option.
- **Errors.** What rbw prints on failure is matched to an error and dropped, never logged.
- **State, from disk:**
  - the config is `~/.config/rbw/config.json`;
  - a copy of the vault is `~/.cache/rbw/<server>:<email>.json`;
  - whether it's unlocked comes from `rbw unlocked`, looked at after every command, when a
    fill or the launcher's passwords want it, and once the agent's lock timeout could have
    locked it. rbw announces nothing, so there's no poll: a `rbw lock` in a terminal shows at
    the next look.
- **Entries** (`rbw list --raw`: name, user, folder, websites) are kept in memory while
  unlocked, and dropped when it locks.

## Known limits

- **Programs running as you can use the unlocked agent**, as above. Keep the auto-lock short.
- **A field that already had the keyboard when the shell started** has to be focused again: a
  text input only tells an input method once it is enabled with one there.
- **One input method a seat.** With fcitx5 or IBus running, the compositor gives the seat to
  that one, and fill in apps is off (Settings says so). The launcher's passwords still work.
- **One app's fields aren't told apart by niri.** Many toolkits (GTK, Chromium) use one text
  input for every field of a window. If the app moves the keyboard to another of its own fields
  in the instant between the click and the fill, that field gets the text. It stays in the same
  app and surface, but could be, say, a chat box. Telling them apart needs smithay to expose a
  per-enable serial. Until then the shell checks the kind of field on its side (the content
  purpose), which narrows but doesn't close the gap.
- **Another client could use niri-secure-fill.** Any unrestricted client can bind it: read the
  focused field's owner (as niri IPC already shows), or put text into the focused field (as a
  virtual keyboard already can). It never hands out a secret.
- **A fill into a field that has text** goes in at the cursor. Password fields never say whether
  they hold text, so the "fill anyway?" question only comes for other fields.
- **Copies in other processes.** The text passes through niri's and the app's memory, and over
  the Wayland socket.
- **X11 apps** get no popup and no fill, only the launcher's copy.
