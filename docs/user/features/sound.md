# Sound

Emaki Settings' **Sound** page (`apps/settings/src/sound.rs`) covers:
- the output and input devices and their levels;
- the output's balance;
- a level meter for the microphone;
- a level for each app playing;
- the sound a notification plays.

`emaki-settings --page sound` opens it, and so does `--page audio`. Quick Settings' Sound page shows the
same app levels, and its **Sound Settings…** row opens this page. When Emaki Settings isn't installed,
the row opens `pavucontrol` instead.

## The page

- **Output:**
  - The device, with the default selected. Picking another moves what's playing to it.
  - Volume, with a mute button.
  - **Balance**, shown when the device has a left and a right. The slider snaps to Centered near the middle.
  - **Test the balance**: Left and Right play the sound theme's front-left and front-right voices.
- **Input:**
  - The device, and its volume with a mute button.
  - **Input level**, a meter in decibels from −60 dB, so speech reads across the middle.
- **Apps:** one row per app playing or paused. Each row has:
  - the app's icon;
  - a slider, which sets the app's level as a share of the output volume;
  - a mute button that mutes that app alone.

  Event sounds, such as a notification's or a click, aren't listed: they're over before they could be set.
- **Notifications › Sound**, on the Notifications page:
  - **Play a sound for notifications.**
  - The sound, chosen from the freedesktop sound theme. Picking one plays it, and ▶ plays it again.
  - The alert volume.

## Where the settings go

The devices, levels, balance and app levels belong to the sound server. They come through amane's
`Audio` service (libpulse, so PipeWire through `pipewire-pulse`), and a change made anywhere shows
on the page: Quick Settings, the volume keys, the bar or pavucontrol. amane's side is documented in
[`system/audio.md`](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/system/audio.md).

**Balance and the volume keys.** pulse keeps a level for each channel.
- Volume changes scale every channel together, so they keep the balance. This covers the slider, the
  volume keys, the bar's scroll and Quick Settings.
- Before, they set all the channels equal, so a balance set in pavucontrol was lost.
- The volume shown is now the loudest channel, as pavucontrol shows it.

The notification sound is `~/.config/emaki/sound.json` (`emaki_kit::settings::Sound`):

| Field | Meaning | Default |
|---|---|---|
| `notification` | the sound theme's sound, by name; `null` plays nothing | `"message-new-instant"` |
| `volume` | 0 to 100 | 60 |

## The input level

The meter is amane's `AudioLevel` service, a recording stream on the default input. The sound
server measures each peak itself (`PEAK_DETECT`), about 25 a second, so nothing reads the sound
itself.
- It records only while the page shows. A second after the page stops reading it, the stream
  ends, and it starts again when the page opens.
- The stream carries `amane.level-meter = "1"`. The shell's privacy indicator
  (`services/desktop/privacy.rs`) leaves it out, so the open page doesn't light the microphone chip.

## The notification sound

The shell plays it when a notification's banner first shows (`surfaces/notifications/toasts.rs`).
That means it is silent in the same cases the banner is hidden:
- under Do Not Disturb;
- for an app set to List Only or Off on the Notifications page.

A notification its sender replaces stays quiet, and a burst plays one sound a second.

`emaki_kit::sounds::play` plays it with `pw-play --media-role=Notification`, which pulse calls the
event role. If `pw-play` or the sound isn't installed, nothing plays. The sounds are found under
`sounds/freedesktop/stereo/` in the XDG data folders.

## Not yet

- Moving one app's sound to another device.
- Ports and card profiles, such as headset or A2DP.
- Volume above 100%.
- A notification's own `sound-name` and `suppress-sound` hints.
- Apps recording, listed like apps playing.
- Sounds for other events (log in, a device plugged in).
