# On-Screen Display

The capsule that shows a level or a switch as it changes (`src/surfaces/osd/`), as macOS and
GNOME answer the media keys: a glyph and a level, or a glyph and a few words, in small glass above
the bottom edge, gone again after 1.5 s (2.2 s for a few words). Pressing again while it shows
changes it in place.

## What it shows

| Readout | Glyph | Shows | Comes from |
|---|---|---|---|
| Volume | the output's (speaker, headphones …), or off while muted or silent | the level, greyed while muted | the keys, and any change to the default output's level or mute |
| Microphone | the microphone, struck through while muted | its level, greyed while muted | the mute key, and the microphone muted or unmuted anywhere |
| Brightness | low under 50%, high above | the level | the keys only (see below) |
| Keyboard light | the keyboard light | one pip a step (a ThinkPad has two), or a level for more than four | the keys, and the light changing anywhere (a ThinkPad's Fn+Space is the firmware's) |
| Sound output | the new device's | its name ("AirPods") | the default output changing |
| Caps Lock, Num Lock | the lock's | "Caps Lock On" / "Off" | the keyboards' LEDs |
| Airplane mode | the airplane | "Airplane Mode On" / "Off" | every radio blocked or unblocked (`rfkill`) |
| Power mode | the mode's | its name ("Power Saver") | the mode changing (power-profiles-daemon) |

## How it hears of a change

- **The keys** come over IPC ([`osd`](../integration/ipc.md#osd)): the shell changes the level
  itself and shows the value it set, so nothing waits for a service to report it (the screen and
  the keyboard light are polled every half second). Holding a key steps from the last value set,
  not a stale reading.
- **Anything else** shows when the service it changes reports it: a script's `wpctl`, a headset's
  buttons, the bar's volume button, a device connecting.

Nothing shows for:

- the shell starting, or the sound server going away or coming back;
- the services reporting a key's own change a moment later (it already showed);
- what Quick Settings' controls change while it's open (they're their own feedback); the lock keys
  still show;
- the screen's brightness changed by anything but the keys (hypridle dims it on idle with
  `brightnessctl`);
- the microphone's level (apps turn it themselves); only muting shows.

## The keys

The niri config binds them to the socket, with the plain commands as a fallback unless the shell
answers `ok` (it isn't running, or is older than this command):

```kdl
XF86AudioRaiseVolume allow-when-locked=true { spawn-sh "[ \"$(printf 'osd\\nvolume\\nup\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock)\" = ok ] || wpctl set-volume @DEFAULT_AUDIO_SINK@ 0.05+ -l 1.0"; }
```

Volume and brightness step 5%, 1% with Shift. Raising the volume unmutes, as on macOS. The keys
never take the screen to black (1% at least). The airplane key isn't bound: the kernel's rfkill
handler already acts on it, and the display shows the change.

## The surface

One overlay-layer surface (`amane.osd`), as big as the capsule, placed by niri on the focused
output, `osd.offset` above the bottom edge and clear of the bar's zone. It takes neither keys nor
the pointer. It frosts itself with one blur shape in `Material::Control`, light or dark with its
backdrop; its layer rule rounds it (`osd.radius`) and gives it the banner's shadow. Hidden, it's
`visible(false)` and holds nothing on the GPU; it draws only rectangles, text and glyphs, so it
never starts vello.

Tokens: `osd.width`, `osd.height`, `osd.radius`, `osd.pad`, `osd.offset`, `osd.track` and
`osd.label` ([tokens.md](../design-system/tokens.md#7-tier-3--component-tokens)).

## Known limits

- Caps Lock and Num Lock are read from the keyboards' LEDs every 250 ms (niri says nothing of them),
  so the capsule can come up a moment after the key.
- A keyboard light whose key is the firmware's shows up to half a second late (it's polled).
- The keyboard layout isn't shown yet: amane doesn't read niri's layout events.
- No settings: it can't be turned off or moved.
