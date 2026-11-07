# Writing a status app

A status app is a program with its own item in the bar, and Activities while something is
going on. It runs on its own and talks to the bar over `org.amane.StatusBar1`. amane-drop
([drop.md](../shell/drop.md), `apps/drop`) is the worked example this guide points into.

The protocol reference is [status-apps.md](status-apps.md). The client and its types are in amane
([status-items.md](https://github.com/098765434/amane/blob/ocbk/docs/fork/system/status-items.md)).
This page covers how to build one that behaves like the bar's own items.

## Pick the right surface

| You have | Use | Not |
|---|---|---|
| A place people go to do something: a grid, a list, a control | a **status item**, opening your own window under it | a tray icon: the bar can't place your window or forward drops to a tray icon |
| Something going on now that ends: an upload, a timer, a call, a recording | an **activity**, ended when it's done | a notification for each step |
| Something that happened and is over | a notification (`amane::Notify`) | an activity left on screen |
| A program that already speaks StatusNotifierItem | the tray ([status-bar.md](../shell/status-bar.md#apps-the-tray)) | |

An app can have one item and any number of activities. Many apps need only activities: a
recorder can show an activity while it records and have no item at all.

## 1. Put up an item

```rust
use amane::status_item::{Accepts, ItemIcon, ItemSpec, MenuEntry};
use amane::StatusItem;

let mut spec = ItemSpec::new("org.example.shelf", ItemIcon::Glyph("tray".into()));
spec.tooltip = "Shelf".into();
spec.accepts = Accepts { files: true, text: false };
spec.opens_on_drag = true;
spec.menu = vec![
    MenuEntry::new("open", "Open Shelf"),
    MenuEntry::separator(),
    MenuEntry::new("quit", "Quit Shelf"),
];
let item = StatusItem::register(spec);
```

| Field | What it does | Advice |
|---|---|---|
| `id` | unique on the desktop; the bar keeps its place in `order` as `app:<id>` | a reverse-DNS name; never change it, or the user's ordering and hiding are lost |
| `icon` | `Glyph` (a bar symbol by name), `Theme` (an icon-theme name), or `File` (png or svg) | prefer a `Glyph`: it takes the bar's colour, size and attention tint. The names are below |
| `title` | text after the icon, cut at 120 px | leave it empty unless the text *is* the item (a reading, a count) |
| `tooltip` | the hover text | the app's name |
| `menu` | the right-click menu: `id`, `label`, `separator`, `enabled`, `checked`, `destructive`, `icon`, `children` (a submenu) | end it with Quit. With no menu, the right-click comes to you as `Activated { button: Right }` |
| `accepts` | whether files and/or text can be let go on the item | |
| `opens_on_drag` | sends `DragHover` as soon as files come over the item | set it when your window takes drops, so the drag can carry on into it |
| `attention` | shows the icon in the accent | for something that wants the user (a failure), cleared once they look |
| `badge` | a small count | |

Glyph names (`amane_kit::icons::NAMED`): `dot`, `record`, `timer`, `music`, `play`, `pause`,
`skip-next`, `skip-previous`, `screen-share`, `camera`, `mic`, `upload`, `download`,
`cloud-upload`, `tray`, `share`, `send`, `phone`, `folder`, `image`, `mail`, `terminal`,
`update`, `history`, `check`, `check-circle`, `close`, `warning`, `error`, `copy`, `package`,
`settings`. The bar shows `dot` for an unknown name.

To change anything, send the whole spec again with `item.update(spec)`. Drop rebuilds its spec
from its settings each time (`spec()` and `refresh_item()` in `apps/drop/src/main.rs`).

## 2. Answer its events

```rust
item.on_event(|event| match event {
    ItemEvent::Activated { button: ItemButton::Left, x, output, edge } => toggle(x, output, edge),
    ItemEvent::MenuSelected(id) if id == "quit" => quit(),
    ItemEvent::DroppedFiles(files) => keep(files),
    ItemEvent::DragHover { over: true, x, output, edge } => open_for_drag(x, output, edge),
    ItemEvent::DragHover { over: false, .. } => close_soon_unless_held(),
    ItemEvent::Shown(_) => {} // hidden by the user: keep running
    _ => {}
});
```

Events arrive on a thread of their own. Change your state there, then let a window redraw: put
the state in a `Mutex` behind a kit `marker!`, and call `changed()` after each change. Drop does
this in `state.rs` with `state::change(…)`.

## 3. Open a window under the item

`x` is the item's middle on monitor `output`, in logical pixels, and `edge` is the bar's edge.
Drop's window is a clear, full-screen `Layer::Overlay` surface that draws one card at that
place. Its outside takes clicks, which close it:

```rust
LayerWindow::new()
    .width(WindowSize::Full)
    .height(WindowSize::Full)
    .layer(Layer::Overlay)
    .space(Zone::Ignore)
    .namespace("org.example.shelf")         // needs a niri layer rule, below
    .output(Some(output))
    .keyboard(Keyboard::Exclusive)          // Escape closes; none while a drag is on
    .blur_shapes(vec![Material::Menu.shape(left, top, w, h, 18.0, 1.0)])
    .child(/* the card, padded to (left, top) */)
```

- **Where the card goes.** Centre it on `x`, keep it on the screen, and keep it clear of the
  bar: `sys::size::panel::height() + 6` from a top bar, or the same above a bottom one.
  `card_place` in `apps/drop/src/grid.rs` does this, with a test.
- **Frost.** Use a blur shape the size of the card, and a niri `layer-rule` for your namespace.
  For an app in this repo, add the namespace to `apps/shell/src/tokens/layer_rules.rs`, then regenerate the
  rules and splice them into niri's config ([layer-rules.md](layer-rules.md)). For one outside
  it, copy the `amane.drop` rule from `niri-layer-rule.kdl`.
- **Only quads.** Draw with `Rectangle`, `Text`, glyphs and `Arc`, so nothing goes through vello
  ([performance.md](../development/performance.md)). Check with `AMANE_FRAMES=1` and the
  `amane/vello-stats` feature: every frame should say `vello 0`.

## 4. Take drags through the item

With `opens_on_drag`, a drag over the item sends `DragHover { over: true }`. Open the window at
once, and the user can carry the files on into it:

- **Don't take the keyboard** (`Keyboard::None`), and give the surface an `input_region` of just
  the card, so the drag can still reach other programs.
- **Close it in a moment** (Drop waits 700 ms) once the files have left the item and the card,
  unless a question is showing. Every drop target in the card reports with `on_files_over`, so
  you know whether anything still holds the files (`state::hover`, `state::drag_left`).
- **Files let go on the item itself** come as `DroppedFiles`. Drop keeps them on its Drop Bar.
- **A click while a drag-opened window shows** turns it into a normal one that takes the keys.

## 5. Show work as an activity

```rust
let mut content = ActivityContent::new("Shelf", ItemIcon::Glyph("cloud-upload".into()), "");
content.title = "Uploading photo.jpg".into();
content.subtitle = "to S3".into();
content.trailing = Trailing::Progress(0.0);
content.progress = Some(0.0);
content.relevance = 0.8;
content.buttons = vec![ActivityButton { destructive: true, ..ActivityButton::new("cancel", "Cancel") }];

let activity = LiveActivity::start(content.clone());
activity.on_event(move |event| if event == ActivityEvent::Action("cancel".into()) { cancel() });
// … as it goes, no more than a few times a second
activity.update(content.clone(), false);
// done: show the outcome, then go
activity.update(finished, false);
activity.end(Dismiss::After(3));
```

These are the bar's rules for activities:

- **Compact is a glance.** The leading picture says what it is. The trailing part is the one
  number that changes: a short text, a countdown, a count-up or a progress ring. Put the rest in
  `title` and `subtitle`, which only the expanded view shows.
- **Let the bar tick.** For a timer, give `Trailing::Countdown(end_ms)` or `CountUp(start_ms)`,
  times in Unix milliseconds, and send nothing until the next real change.
- **Don't update too often.** Drop sends progress at most every 150 ms (`TELL_EVERY` in
  `tasks.rs`).
- **Alert rarely.** `update(content, true)` opens the expanded view for 4 s (not under Do Not
  Disturb). Keep it for what needs the user now: a call, a finished timer. A normal end isn't
  one.
- **Relevance ranks it** against the others, and only the top ones show compact. For reference:
  Screen Sharing 0.9, Drop's tasks 0.8, the timer 0.7, Now Playing 0.3. Something the user is
  waiting on goes high; background goes low.
- **Up to three buttons**, with the destructive one (Stop, Cancel) marked `destructive`. Their
  presses come back as `ActivityEvent::Action(id)`.
- **End it.** `Dismiss::After(n)` shows the final state (Done, Failed, a link) for `n` seconds.
  Drop uses 3 s, 6 s for a failure, and 8 s when there's a link to open. The bar dims an activity
  past `stale_after` without an update, and ends any after 8 hours. `Dismissed` tells you the
  user ended it.
- **Tint** with a palette name (`orange`, `green`, `red`, `purple`…), so it follows the theme.
  Use one for a state (green when done, red when failed), not for decoration.

## 6. Run as a desktop citizen

- **One copy.** `App::new().app_id("org-example-shelf")` gives an IPC socket, and a second launch
  hands its arguments to the copy already running (`on_activate`). Register the item only when
  no copy answers, as Drop's `main()` does.
- **Commands.** Offer `toggle`, `open` and `close` over IPC, for a keybind. Drop's niri bind is
  `printf 'toggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane-drop.sock`, falling back to starting its
  service.
- **Its own service.** Run it as a systemd user unit of its own, not spawned by the shell: a
  restart of the shell kills whatever is in its cgroup. The client puts your item and
  activities back by itself once the bar returns ([drop.md](../shell/drop.md#installing)).
- **Quitting.** Remove the item, give the worker a moment to send that, then exit 0. A unit with
  `Restart=on-failure` then stays stopped.
- **The user decides.** An item can be hidden (you get `Shown(false)`), and an app's activities
  can be turned off in Shell Settings › Status Bar › Activities. Keep working either way.
- **Secrets.** Keep passwords in the keyring, with `amane_kit::secrets` in this repo or
  `secret-tool`. Never pass one as a program's argument, which `ps` shows: Drop gives curl its
  secrets on standard input (`-K -`).

## From another language

Any D-Bus library works. The payloads are JSON strings, and the program has to stay connected:
its item and activities go away when it leaves the bus. In Python, with `dbus-next`:

```python
import asyncio, json, time
from dbus_next.aio import MessageBus
from dbus_next import Message, MessageType

BAR = dict(destination="org.amane.StatusBar1", path="/org/amane/StatusBar",
           interface="org.amane.StatusBar1")

async def main():
    bus = await MessageBus().connect()

    async def call(member, signature, *body):
        reply = await bus.call(Message(member=member, signature=signature, body=list(body), **BAR))
        if reply.message_type == MessageType.ERROR:
            raise RuntimeError(reply.error_name, reply.body)
        return reply.body

    def heard(message):
        if message.message_type == MessageType.SIGNAL and message.interface == BAR["interface"]:
            # ItemEvent(s id, s json) or ActivityEvent(u handle, s json)
            print(message.member, message.body[0], json.loads(message.body[1]))

    bus.add_message_handler(heard)
    await call("RegisterItem", "s", json.dumps({
        "id": "org.example.tea", "icon": {"glyph": "timer"}, "tooltip": "Tea",
        "menu": [{"id": "quit", "label": "Quit"}]}))
    ends = int(time.time() * 1000) + 3 * 60 * 1000  # three minutes from now, in ms
    [handle] = await call("StartActivity", "s", json.dumps({
        "app": "Tea", "title": "Steeping", "leading": {"glyph": "timer"},
        "trailing": {"countdown": ends}, "tint": "orange", "relevance": 0.7,
        "buttons": [{"id": "stop", "label": "Stop", "destructive": True}]}))
    await asyncio.Event().wait()  # stay on the bus

asyncio.run(main())
```

The events are JSON in serde's external tagging, in snake case:

- `{"activated":{"button":"left","x":1532.0,"output":"eDP-1","edge":"top"}}`
- `{"menu_selected":"quit"}`, `{"dropped_files":["/home/me/a.png"]}`, `{"dropped_text":"…"}`
- `{"drag_hover":{"over":true,…}}`, `{"shown":false}`
- for activities, `{"action":"stop"}` or `"dismissed"`

## Checklist

- [ ] A stable reverse-DNS `id`, and a `Glyph` icon.
- [ ] A menu ending in Quit.
- [ ] A window placed under `x` on `output`, clear of the bar, frosted with a blur shape and a
      niri layer rule.
- [ ] With `opens_on_drag`: no keyboard and an input region of just the card while dragging, and
      a close after a grace period.
- [ ] Activities with a short trailing part, honest relevance, alerts only when they matter, and
      ended with `Dismiss::After`.
- [ ] Only quads (`vello 0`).
- [ ] Its own systemd user unit, and a `toggle` command for a keybind.
- [ ] Secrets in the keyring, never in arguments.
- [ ] Tested beside the bar without touching the user's clipboard, keyring or phone
      ([development/status-apps.md](../development/status-apps.md#testing)).
