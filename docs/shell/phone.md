# Phone (KDE Connect)

The phone through KDE Connect, drawn as part of the shell. `kdeconnectd` runs on
its own, and nothing of KDE's ever shows: no Plasma, no Kirigami, no
`kdeconnect-sms`. There are three surfaces:

- **Quick Settings' Phone page:** pairing, battery, Ring, Send Files…, the
  clipboard, and the phone's remote for this computer.
- **The bar's phone icon:** the phone's charge, beside the battery.
- **Messages** (`amane-messages`): the phone's text messages and Phone Input.

All three read the fork's `KdeConnect` and `PhoneMessages` services
([`system/kdeconnect.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/system/kdeconnect.md)).
They come from the Quickshell version's "KDE Connect, made native".

The phone's **notifications** and **media** need no code. The shell is the
notification server, so they show as toasts and in the center. Now Playing reads
every MPRIS player, and the phone's playback is one of them. Files the phone
shares land in `~/Downloads`, and KDE Connect sends its own notification.

## Setting up

- `kdeconnect` on in Shell Settings → Services (`programs.kdeconnect`) installs
  it and opens its ports, UDP and TCP 1714–1764.
- The daemon starts at login from its own autostart entry.
- On the phone, install KDE Connect (F-Droid or Play), on the same network, and
  allow the plugins you want: SMS, Contacts, Remote Input and so on.

## Quick Settings: Phone

The **Phone** control waits in Edit Controls' Suggestions (it's under
Connectivity). It shows "<phone> · 54%", "Not Connected", "No Phone" or "Off",
and it's lit while a phone is connected. The bar's phone icon opens the page
too.

The page:

- **My Devices:** each paired phone with its charge, or "Not Connected". A click
  or the ⓘ opens its own page.
- **Other Devices:** the phones on the network that could pair; a click asks
  one. While it waits, the row shows the code both screens show, and a click
  stops waiting. Opening the page announces this computer again, so phones
  answer at once.
- **A phone asking to pair** appears under the list with its code and
  **Decline** / **Pair**. When the page isn't showing, it asks in the shell's
  alert instead (`surfaces/alert/phone.rs`).
- Without the daemon, the page says so and offers **Turn On**.

A paired phone's page, as far as the phone offers each:

| Row | Does |
|---|---|
| Battery | the charge, and "charging" |
| Ring | rings the phone, even on silent |
| Send Files… | the portal's file chooser, then each file to the phone |
| Messages, Phone Input | opens `amane-messages` on that phone and view (when it's installed) |
| Clipboard: Share Automatically, Send Clipboard to Phone | auto-sync both ways, or this computer's clipboard now |
| Phone Remote: Control This Computer | adds Lock Screen, Suspend, Lock & Suspend and Screenshot to the phone's Run Command |
| Forget This Phone… | asks, then unpairs |

An action that fails shows why in red for 10 seconds.

## The bar's phone icon

`Indicator::Phone` (`phone` in `status-bar.json` and the IPC). It shows while a
paired phone is connected and has sent its charge. It's accent while charging
and red when low. The tooltip has the name and the charge. A phone running low
warns at 20 % and 10 % like a Bluetooth device does (`services/network/bt_battery.rs`),
but not while it charges.

## Messages

`amane-messages` is an app beside Shell Settings (`apps/messages`), with its
own window rule that frosts behind it.

- **The inbox** (the sidebar): the phone's conversations, newest first. Each has
  the contact's initials in a colour kept for the name (grey for a bare number),
  the name, the latest message ("You: …" for this side) and when it was sent
  (the time today, "Yesterday", a weekday, then a date). An unread one is bold,
  with the accent's dot. ⟳ asks the phone again, since a sleeping phone sends
  nothing until asked.
- **A conversation:** bubbles drawn after Messages' (`apps/messages/src/bubble.rs`).
  - Grey on the left; on the right, a gradient from the accent lightened to the accent.
  - Each bubble is a pill, mirrored between the two sides, sized to its text up to 70 % of the
    width.
  - The last bubble of a run has the curled tail at its bottom corner.
  - The outline is one vector path, made for each size, that amane fills, with the gradient, on
    the quad pipeline.

  Each day is named over its messages, and the time sits under each run (one side, under 10
  minutes apart). It opens on the newest messages and follows new ones down.
  **Load Earlier Messages** asks for 50 more. A reply shows at once, dimmed with
  "Sending…", until the phone sends it back. A failed one is red, "Not
  Delivered".
- **Phone Input:** a trackpad (drag moves the phone's pointer, a tap clicks, a
  right click long-presses, scrolling scrolls) with Left/Middle/Right buttons.
  Keys typed while it shows go to the field focused on the phone; the hint says
  whether one is focused. Ctrl and Super stay this computer's shortcuts.

`amane-messages --device <id> --view messages|input` opens on a phone and a
view. Launched again while it runs, it switches to them. Without `--device`, it
picks the first connected phone that has messages.

Contact names come from the vCards the phone's Contacts plugin syncs. Without
it, conversations show numbers.

## Limits

These come from KDE Connect itself:

- A sleeping phone can't be woken. It answers when it wakes.
- A conversation's history only exists once the phone has sent it.
- Pictures and group MMS only show as an attachment count.
- Starting a new conversation isn't offered: there's only a reply in an
  existing one.
- A reply that shows as sent was taken by the daemon; delivery is the phone's.

## Checking it with a phone

1. Phone page → Other Devices → the phone; accept on the phone with the same
   code. `kdeconnect-cli -l` says paired and reachable.
2. The charge shows on its row, on its page and in the bar.
3. Ring rings it; a notification on the phone shows as a toast; its music shows
   in Now Playing.
4. Send Files… arrives on the phone; a file shared from the phone lands in
   `~/Downloads`.
5. Control This Computer on → the phone's Run Command lists the four; Lock
   Screen locks.
6. Messages: the inbox fills with names. A conversation loads, and a reply
   appears as Sending… and then sent. A text to the phone appears live.
7. Clipboard both ways; Share Automatically off writes `autoShare=false`.
8. Phone Input: the trackpad moves the phone's pointer; with a field focused on
   the phone, typing lands there.
9. Forget This Phone… unpairs it.
