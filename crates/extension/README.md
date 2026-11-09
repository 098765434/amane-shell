# emaki-extension

Moved out of the amane fork in Phase 9; the wire format and bus name are unchanged.

A program's own **item in the status bar**, and its **activities**: something
going on now, like a timer, a song or an upload, shown in the bar while it
lasts. amane is the client side and holds the
protocol types. The bar serves `org.emaki.StatusBar1` on the session bus and
draws both. emaki is that bar. Unlike the tray (StatusNotifierItem), the
bar draws everything here itself, and drops reach the program.

## Usage

```rust
use emaki_extension::{
    Accepts, ActivityContent, Dismiss, ItemEvent, ItemIcon, ItemSpec, MenuEntry, Trailing,
};
use emaki_extension::{LiveActivity, StatusItem};

// the id is unique on the desktop, like an app id
let mut spec = ItemSpec::new("org.emaki.drop", ItemIcon::Glyph("tray".into()));
spec.accepts = Accepts { files: true, text: false };
spec.opens_on_drag = true;
spec.menu = vec![MenuEntry::new("quit", "Quit")];

let item = StatusItem::register(spec);

item.on_event(|event| match event {
    // x is the item's middle on that monitor, edge where the bar is
    ItemEvent::Activated { button, x, output, edge } => {}
    ItemEvent::MenuSelected(id) => {}
    ItemEvent::DroppedFiles(files) => {}
    ItemEvent::DroppedText(text) => {}
    ItemEvent::DragHover { over, x, output, edge } => {}
    // the user took it out of the bar (or put it back); keep running
    ItemEvent::Shown(shown) => {}
});

item.update(new_spec); // the whole spec again
item.remove();

let mut content = ActivityContent::new("Drop", ItemIcon::Glyph("upload".into()), "Uploading");
content.trailing = Trailing::Progress(0.3); // or Text, Countdown(ms), CountUp(ms)

let activity = LiveActivity::start(content.clone());
activity.on_event(|event| { /* Action(id) from a button, or Dismissed */ });
activity.update(content, false); // true alerts: the bar briefly expands it
activity.end(Dismiss::After(3));
```

`crates/extension/examples/status_item.rs` puts up an item and a countdown and prints what
happens. Run it beside a bar.

What an activity shows:

| Field | Compact | Minimal | Expanded |
|---|---|---|---|
| `leading` (an `ItemIcon`) | ✓ | ✓ | ✓ |
| `trailing`: text, a countdown or count-up to a time, a progress ring | ✓ | | ✓ |
| `title`, `subtitle` | | | ✓ |
| `progress` (a bar), `buttons` (up to 3) | | | ✓ |
| `tint` (a palette name, so it follows the theme) | ✓ | ✓ | ✓ |

- **Ranking.** `relevance` (0 to 1) ranks several activities. The bar shows the
  most relevant compact and the rest minimal.
- **Countdowns.** A `Countdown` or `CountUp` is counted by the bar, so it
  needs no update each second.
- **Staleness.** `stale_after` (seconds) is how long without an update before
  the bar shows it as out of date.
- **Ending.** The bar ends any activity after 8 hours.

## Calling it from a script

Every payload is one JSON string, so `busctl` works:

```sh
busctl --user call org.emaki.StatusBar1 /org/emaki/StatusBar org.emaki.StatusBar1 \
    StartActivity s '{"title":"Tea","leading":{"glyph":"timer"},"trailing":{"countdown":1767225600000}}'
# → u 7
busctl --user call org.emaki.StatusBar1 /org/emaki/StatusBar org.emaki.StatusBar1 \
    EndActivity us 7 '"now"'
```

An activity or item ends when the program that made it leaves the bus, so a
script's `busctl` call alone only flashes one by. A script keeps a connection
open, for example with `gdbus` in a loop or a small Python `dbus-next` client.

## The protocol

At `/org/emaki/StatusBar`, interface `org.emaki.StatusBar1`. The methods, signals and errors
are listed once, in emaki's
[status-apps.md](../../docs/developer/integration/status-apps.md#the-interface). What the client relies on:

- **Levels.** Payloads carry `v`, the protocol's level (`LEVEL`, now `1`; left out means the
  client's). A bar serves `OLDEST_LEVEL` to `LEVEL` and refuses others with `Error.Level`. A
  level is served for two shell minor releases after the next one ships.
- **Hello.** `hello(id)` says who the program is and returns `ShellInfo`: the levels, the kinds
  the shell takes, the user's locale and the bar's size. It is optional, and said again to
  every new bar.
- **Newer parts.** Unknown properties are ignored; an unknown icon, trailing kind, tint, menu
  entry or button is dropped with a warning and the rest kept. Text, menus and buttons past
  the [budget](src/budget.rs) are cut.
- **Budgets.** 64 KB a payload, one update a tenth of a second to each thing, 20 calls a second,
  10 updates held; faster updates are coalesced by the bar, and past that it answers
  `Error.Busy`.
- **Signals.** These go only to the program that made the item or activity.
- **Ownership.** Only the program that made an item or activity changes it.
  The bar drops all of a program's items and activities when it leaves the bus.
- **Leaving out fields.** Every field but an item's `id` may be left out.

## How the client works

- **Order of calls.** Calls go out on amane's worker thread in the order they
  were made. A `LiveActivity` exists at once, and its updates wait in line
  behind the `StartActivity` that numbers it.
- **Events.** Events come in on a thread of their own. A handler that should
  change a window writes a `Service` and lets that redraw.
- **Bar restarts.** The client keeps every item and activity. When
  `org.emaki.StatusBar1` gets a new owner (the bar restarted), it says hello again, registers
  them all again and maps the new handles back.

## The old name

This crate was `emaki-status` until the extensions plan's Phase 0. `crates/status` is still
there, re-exporting everything under the old name for one release.

## Related

- emaki's [writing a status app](https://github.com/emaki-desktop/emaki/blob/main/docs/developer/integration/status-apps.md#writing-a-status-app):
  a window under the item, drags through it, what to put in an activity, running as a
  service. emaki-drop is the worked example.
- emaki's [status apps internals](https://github.com/emaki-desktop/emaki/blob/main/docs/developer/integration/status-apps.md#the-pieces-across-repos):
  how the bar serves this, and testing beside a running desktop.
- amane's [tray](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/system/tray.md): the StatusNotifierItem tray, for programs that already speak it.
- amane's [notifications](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/system/notifications.md): `Notify`, for what is over rather than going on.
