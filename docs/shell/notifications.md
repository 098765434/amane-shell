# Notifications

Toasts, a notification center and Do Not Disturb, in the style of macOS and iOS
(`src/components/notifications/`). amane is the notification daemon: its `Notifications`
service owns `org.freedesktop.Notifications`, keeps the list and talks to senders (see
[amane `system/notifications.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/system/notifications.md)). Everything here is presentation: how long a toast shows, how
cards stack, what Do Not Disturb hides.

> Only one notification daemon can run. Stop mako, dunst or swaync before starting the shell.

## Toasts (`toasts.rs`)

A banner slides in at the top-right for each arriving notification, newest on top, at most
`notification.toast.max-visible` at once. Each toast is **its own layer surface** (namespace
`amane.notifications.toast`, one of `SLOTS` windows) sized to its card, so niri's shadow, which
its layer rule gives the surface, falls under exactly the card. The card is glass: the surface
asks for one blur shape the size of the card, a `Material::Banner`: thin glass that turns light or
dark with its backdrop, as Apple's banners do (see [materials.md](../design-system/materials.md)). Sliding in, sliding out and swiping
move the surface itself (its margin), not a widget inside it.

### The card (`card.rs`)

Toasts and the center share one card, laid out like an iOS banner in three columns:

- **Leading:** the app's icon (`card.icon-size`), centered in a `card.icon-frame`-wide frame as
  tall as the content.
- **Middle:** the app's name (`sys.type.caption`, secondary), the summary (`card.title`: 13/bold,
  16px lines, tracked −0.26px) and at most two lines of body (`card.body`: 13/regular, 16px lines,
  tracked −0.1px), elided.
- **Trailing:** the time (`card.time`: 11/regular), right-aligned, with the sender's image under it
  as a `card.thumbnail-size` thumbnail with `card.thumbnail-radius` corners, `card.trailing-gap`
  apart. The column is as wide as the widest time, so the text doesn't move as the time ages. The
  image's room is kept from the moment the card is measured, so the card doesn't grow when the
  image finishes loading.
  The image is the `image-data`/`image-path` hint, else the app icon when the sender sent it as a
  file or `file://` url rather than a theme name: the spec ranks it after the hints, and niri
  sends its screenshots that way. Such an icon then isn't drawn on the left as well.

The insets are `card.padding-block` above and below, `card.padding-start` on the leading side and
the wider `card.padding-end` on the trailing side, where the time sits. Actions run in a row of
equal buttons underneath. The design these follow (`shadow-notification-{bright,dark}.css`, a
Figma export) also sets the banner's glass, edge and label blending. The banner material and
vibrancy already draw those, so the card doesn't.

Which toasts show is computed fresh from the notification list and the clock (`alive`):

| Notification | Toast shows for |
|---|---|
| `Timeout::Default` (the sender left it to the shell) | 5 s |
| `Timeout::After(d)` | `d` |
| `Timeout::Never`, or `Urgency::Critical` | until clicked, dismissed or swiped |

- **Hover** holds every shown toast, and they stay 2 s more after the pointer leaves.
- **Click** runs the default action, or dismisses it when there is none (`Notifications::click`).
  Action buttons run their action. The ✕ that replaces the time while hovered dismisses it.
- **Swipe right** past 30 % of the width to put the toast away. The notification stays in the
  center. A sender replacing it shows it again.
- **Transient** notifications (`transient` hint) leave the list once their toast ends
  (`Notifications::expire`), so they never reach the center.
- Opening the center puts every toast away.

A small `Toasts` service ticks every 100 ms. It only wakes the toasts when the set that shows
changes, and keeps what the list can't say: swiped toasts, hover, grace times.

## Notification center (`center.rs`)

A full-height sidebar of thick glass at the right edge (namespace `amane.notifications.center`),
M3's modal side sheet (`side_sheet()`). The surface frosts itself as one blur shape, a
`Material::Sheet`: thick glass, as it holds the most text of any glass. It
springs in when the **clock** is clicked and closes with Escape, the clock, or the IPC call. It
shares Quick Settings' one-open-at-a-time state (`Panel::Notifications`). Its header shows Do Not
Disturb's state, **Clear All**, and the Do Not Disturb button, which opens the timed menu.

Notifications are grouped by app (`desktop-entry`, else the app name), newest group first.
History lasts the session: a notification stays until it is dismissed or the shell restarts.

- **One notification** is a plain card.
- **Several** form an iOS **stack**: the newest card in front, and up to
  `notification.stack.max-peek` blank plates peeking `notification.stack.peek` below it. Each
  plate is `stack.scale-step` smaller and `stack.fade-step` fainter. Click the stack to spring it
  open into a list under the app's name, with **Show Less** and ✕ (clear the group). Stacks fold
  back up when the center closes.
- New cards scale and fade in. Dismissed cards slide right and fade, and a group whose last card
  went keeps drawing until it has slid out.

Times read "now", "5m ago", "2h ago", then the day. They tick with the bar's `Clock`.

## Do Not Disturb (`dnd.rs`)

While on, toasts are hidden: critical ones still show, and everything still collects in the
center. The bar shows a moon. Choices, from the center's moon button or right-clicking the bar
indicator: **For 1 Hour**, **Until This Evening** (19:00, offered only before then), **Until
Tomorrow** (08:00), **Turn On** / **Turn Off**.

It is kept on disk beside the shell's other state files (the dock's pins, the status icons, the
Quick Settings and widget layouts, night light, the last VPN, the launcher's use and clipboard
history; see [services.md](../integration/services.md#settings-and-state)):
`$XDG_STATE_HOME/amane-shell/dnd` holds `forever` or `until <unix seconds>`. A missing, unreadable
or past file reads as off.

## Bar

- The **clock** opens and closes the center.
- An indicator before the clock shows a **moon** while Do Not Disturb is on, or an accent **bell**
  while something arrived since the center was last opened. It shows nothing otherwise. Left-click
  opens the center; right-click opens the Do Not Disturb menu.

## IPC

`notifications [toggle | clear | dnd [on|off|toggle]]` opens or closes the center, clears it, and
switches Do Not Disturb ([ipc.md](../integration/ipc.md#notifications)).

## Motion (`amane_kit::motion`, `kit/src/motion.rs`)

Both surfaces place cards through one keyed animator, shared with the rest of the shell. The view says where each card should be
(a `Pose`: offset, scale, opacity, content opacity) every time it runs. Each card springs there
under its key, and a card that drops out keeps its last content while it moves to its exit pose.
The timings are the shell's shared springs and fades (`motion::spring`, `sheet`, `fade`), named
constants until `sys.motion.*` tokens exist in Penpot. A card is measured once
(`card::measure`), and the stack places it and draws it from that measurement.

## Tokens

Colours are the `sys.color.surface.notification.*` pair for toasts (`bg`, `fg`, `fg-secondary`,
`stroke`; checked for contrast over black and white wallpaper). The center is a panel [`Card`](../kit/components.md#card) filled with thick glass's tint, with
`raised` cards on it, in the panel's colours, under a `TopAppBar`. Sizes are `notification.*`: `toast.width`, `toast.max-visible`,
`center.width`, `center.radius` (the cards' 20 plus the inset of 10 around them, so the corners
nest), `card.radius`, `card.icon-size`, `card.icon-frame`, `card.thumbnail-size`,
`card.thumbnail-radius`, `card.padding-block`, `card.padding-start`, `card.padding-end`,
`card.trailing-gap`, the `card.title`, `card.body` and `card.time` typographies, and `stack.peek`,
`max-peek`, `scale-step`, `fade-step`. A toast's shadow is softer than other surfaces' (niri
softness 48, the design's `0 8 48`; see [layer-rules.md](../integration/layer-rules.md)).

> These tokens were added to `tokens.json` by hand, before they existed in Penpot. Recreate them
> there under the same names, or the next export drops them.

## Not yet

- Toasts look see-through until the generated layer rules are in niri's config (see
  [layer-rules.md](../integration/layer-rules.md)).
- No sounds, inline reply, per-app settings, or Focus schedules.
