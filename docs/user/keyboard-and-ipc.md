# Keyboard and IPC

**The shell has no keybinds of its own; niri's do the work.** A bind writes a command to the
shell's socket, and the shell answers `ok`. The same socket is what scripts use.

## The socket

The shell listens on `$XDG_RUNTIME_DIR/amane.sock`. A call is the command's name on the first
line and each argument on a line of its own; the reply is `ok`, a usage line, or an error.

```sh
printf 'launcher\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
```

Drop and Shell Settings have sockets of their own, `amane-drop.sock` and
`amane-settings.sock`, beside it.

## The keybinds people copy

In niri's `binds {}` block. Every bind spawns the call through a shell:

```kdl
binds {
    Mod+Space hotkey-overlay-title="Launcher" {
        spawn "sh" "-c" "printf 'launcher\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
    Mod+Ctrl+V hotkey-overlay-title="Clipboard" {
        spawn "sh" "-c" "printf 'launcher\\ntoggle\\nclipboard\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
    Mod+Shift+Space hotkey-overlay-title="Assistant: listen" {
        spawn "sh" "-c" "printf 'assistant\\nlisten\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
    Mod+S hotkey-overlay-title="Quick Settings" {
        spawn "sh" "-c" "printf 'quick-settings\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
    Mod+N hotkey-overlay-title="Notifications" {
        spawn "sh" "-c" "printf 'notifications\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
    Mod+P hotkey-overlay-title="Fill a password" {
        spawn "sh" "-c" "printf 'passwords\\nsuggest\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
    Mod+Shift+D hotkey-overlay-title="Drop" {
        spawn-sh "printf 'toggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane-drop.sock || systemctl --user start amane-drop.service";
    }
}
```

### The media and brightness keys

The shell changes the level and shows it in the on-screen display. Each bind falls back to the
plain command unless the shell answers `ok`, so the keys work before the shell is up:

```kdl
binds {
    XF86AudioRaiseVolume allow-when-locked=true { spawn-sh "[ \"$(printf 'osd\\nvolume\\nup\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock)\" = ok ] || wpctl set-volume @DEFAULT_AUDIO_SINK@ 0.05+ -l 1.0"; }
    XF86AudioLowerVolume allow-when-locked=true { spawn-sh "[ \"$(printf 'osd\\nvolume\\ndown\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock)\" = ok ] || wpctl set-volume @DEFAULT_AUDIO_SINK@ 0.05-"; }
    XF86AudioMute allow-when-locked=true { spawn-sh "[ \"$(printf 'osd\\nvolume\\nmute\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock)\" = ok ] || wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"; }
    XF86AudioMicMute allow-when-locked=true { spawn-sh "[ \"$(printf 'osd\\nmic\\nmute\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock)\" = ok ] || wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle"; }
    XF86MonBrightnessUp allow-when-locked=true { spawn-sh "[ \"$(printf 'osd\\nbrightness\\nup\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock)\" = ok ] || brightnessctl set 5%+"; }
    XF86MonBrightnessDown allow-when-locked=true { spawn-sh "[ \"$(printf 'osd\\nbrightness\\ndown\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock)\" = ok ] || brightnessctl set 5%-"; }
}
```

Add `fine` as a last line (`osd\\nvolume\\nup\\nfine`) on a Shift variant to step by 1 instead
of 5. In HDR the brightness keys move SDR white rather than the backlight
([osd.md](features/osd.md#the-keys)).

## What the socket understands

The commands, in the words of what they do. Every argument and reply is in
[ipc.md](../developer/integration/ipc.md).

| Command | Does |
|---|---|
| `launcher [toggle\|open\|close] [apps\|files\|actions\|clipboard]` | opens the launcher's pill, or one of its modes |
| `assistant listen [start\|stop\|toggle\|cancel]`, `assistant status` | speak a command; what the daemon runs |
| `quick-settings [toggle\|open\|close] [<page>]`, `edit`, `show\|hide <control>` | the Quick Settings card, or one page alone (`sound`, `wifi`, `bluetooth`, `power`, …) |
| `notifications [toggle\|clear\|dnd on\|off\|toggle]` | the notification center and Do Not Disturb |
| `osd <what> <how> [fine]` | volume, mic, brightness, keyboard light, airplane mode, the power profile; `show <what>` shows the level unchanged |
| `widgets list\|add\|remove\|edit\|done\|reset-layout` | the desktop widgets |
| `status-bar show\|hide\|toggle <icon>`, `move`, `reset-order`, `tray …`, `apps …`, `activities …` | the bar's items, the tray, programs' items and Activities |
| `night-light [on\|off\|toggle\|temperature <K>\|schedule …]` | Night Light |
| `passwords suggest\|status\|unlock\|lock\|sync\|…` | the password fill and the vault |

## Inside the shell

Once a surface is open, the keyboard is the shell's:

- **Launcher:** type to search; Tab narrows to an app; arrows move; Return runs; Escape closes
  or goes back to the pill ([launcher.md](features/launcher.md#using-it)).
- **Quick Settings** and the **notification center:** Escape closes; arrows move between
  controls.
- **The password popup:** arrows pick an entry, Return fills it, Escape closes
  ([passwords.md](features/passwords.md)).
- **The dock** takes no keys; it is reached with the pointer or niri's overview.

The keyboard layout in the bar follows niri's; a click lists the layouts and switches
([status-bar.md](features/status-bar.md#keyboard-layout)).
