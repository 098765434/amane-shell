# Status apps and Activities

The bar serves `org.amane.StatusBar1` on the session bus. Through it, other programs put up
their own items in the bar, and Activities: what's going on now, shown in the bar until it ends.
The server is `apps/shell/src/services/status_apps.rs`. What the bar draws is in
[status-bar.md](../shell/status-bar.md#programs-items).

The protocol and its JSON types are one crate, [amane-status](../../crates/status/README.md)
(`crates/status`, with the `StatusItem` and `LiveActivity` clients), so a program and the bar
share one definition.

## From Rust

```rust
use amane_status::{ActivityContent, ItemEvent, ItemIcon, ItemSpec, Trailing};
use amane::{LiveActivity, StatusItem};

let item = StatusItem::register(ItemSpec::new("org.amane.drop", ItemIcon::Glyph("tray".into())));
item.on_event(|event| { /* Activated, MenuSelected, DroppedFiles, DragHover … */ });

let mut content = ActivityContent::new("Drop", ItemIcon::Glyph("upload".into()), "Uploading");
content.trailing = Trailing::Progress(0.4);
let upload = LiveActivity::start(content);
```

The client sends everything again when the bar restarts. `crates/status/examples/status_item.rs` is a
whole program.

## From a script

Every payload is one JSON string:

```sh
bus() { busctl --user call org.amane.StatusBar1 /org/amane/StatusBar org.amane.StatusBar1 "$@"; }
bus RegisterItem s '{"id":"demo","icon":{"glyph":"tray"},"title":"Demo","menu":[{"id":"quit","label":"Quit"}]}'
bus StartActivity s '{"app":"Demo","title":"Tea","leading":{"glyph":"timer"},"trailing":{"countdown":1767225600000}}'
```

An item or activity goes when the program that made it leaves the bus. A lone `busctl` call
leaves straight away, so the thing it put up goes with it. A script that keeps one up holds its
connection open, for example a Python `dbus-next` client.

## The interface

| Method | In | Out | Errors |
|---|---|---|---|
| `RegisterItem` | `s` spec | | `Error.Invalid`, `Error.IdTaken` (another program has the id) |
| `UpdateItem` | `s` spec | | `Error.Invalid`, `Error.IdTaken`, `Error.Unknown` (no such item of the caller's) |
| `RemoveItem` | `s` id | | `Error.Unknown` |
| `StartActivity` | `s` content | `u` handle | `Error.Invalid` |
| `UpdateActivity` | `u` handle, `s` content, `b` alert | | `Error.Invalid`, `Error.Unknown` |
| `EndActivity` | `u` handle, `s` dismiss (`"now"`, `{"after":5}`; empty is now) | | `Error.Invalid`, `Error.Unknown` |

Errors are under `org.amane.StatusBar1.Error`.

The two signals go only to the program concerned (a unicast signal):

- `ItemEvent(s id, s event)`
- `ActivityEvent(u handle, s event)`

`Introspect` answers on `org.freedesktop.DBus.Introspectable`.

## The shell's side

- **Ownership.** Items are kept by id with the caller's unique bus name. Only that program
  changes or removes an item.
- **Leaving the bus.** The bar follows `NameOwnerChanged` for unique names, and drops a
  program's items and activities when it leaves.
- **Signals out.** `status_apps::send(id, event)` sends an item's events off the event loop.
- **Activities.** These go to `services/desktop/activities.rs`. It keeps their handles and
  owners, ends them after 8 hours or when dismissed, and sends `Dismissed` once one has gone.
- **Seen lists.** Shell Settings lists programs once seen: items in
  `~/.cache/amane-shell/status-apps-seen.json`, activities' app names in
  `activities-seen.json`.
- **When another program owns the name.** The shell logs it and leaves status apps to that
  program.

Over IPC: `status-bar apps list | activate <id>` and
`status-bar activities list | end bus.<handle>` ([ipc.md](ipc.md#status-bar)).
