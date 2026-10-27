# App scaffolding

What an app beside the shell needs to look like the shell (`kit/src/app.rs`,
`amane_kit::app`): its window, text fields, a dialog and a sidebar to move between pages. Shell
Settings is built from it ([settings-app.md](../shell/settings-app.md)).

```rust
use amane_kit::app::{app, app_window, standard_shortcut};

fn main() {
    app("amane-example").normal_window("main", main_window).run();
}

fn main_window() -> amane::Window {
    app_window(content())
        .title("Example")
        .size(720.0, 480.0)
        .on_shortcut(standard_shortcut)
}
```

| Item | Does |
|---|---|
| `app(app_id)` | `amane::App` with this app-id: its windows, its IPC socket (`$XDG_RUNTIME_DIR/<app_id>.sock` for an id starting `amane-`, else `amane-<app_id>.sock`) and its settings folder go by it, and only one copy runs: a second launch hands over to the first |
| `app_window(content)` | a normal `Window` in the shell's look: `window_bg()` behind `content`, a `sys.space.inset.l` padding, vibrant labels where they're on. Set the title, size and shortcuts on the `Window` it returns |
| `window_bg()` | the opaque window background, `sys.color.window.bg`, as macOS draws a window's content |
| `standard_shortcut(key)` | the shortcuts every window has: Ctrl+W closes it. Call it from your own `on_shortcut` and handle what it leaves (`false`) |
| `text_field(id, placeholder)` | a one-line `TextInput` in `sys.type.label`, its selection in the accent; its text is kept under `id` |
| `field_box(id, input, width)` | the box a text field sits in: control height, `sys.radius.raised`, the secondary fill, ringed in the accent while it has the keys |
| `Page`, `sidebar(…)` | a settings-style sidebar ([below](#sidebar)) |
| `Dialog` | the kit's alert, re-exported; `.open(parent)` shows it in a modal window over the app's ([components.md](components.md#dialog)) |

The other controls (buttons, switch, radio, dropdown) are in [controls.md](controls.md); the
settings files a shell app changes are `amane_kit::settings` ([settings-app.md](../shell/settings-app.md#the-files)).

## Sidebar

`sidebar(id, &pages, selected, on_select)` lists `Page::new(glyph, name)`s one row each
(`sys.size.row.height`, a glyph and a `sys.type.label` name), the shown one filled
`sys.color.state.selected`, with the hover layer; `on_select` gets the index of a row clicked.
Each row is keyed `<id>.<index>`. A `Page::section(name)` is a caption over the pages after it,
as macOS groups its sidebar; it can't be selected but takes an index like a page, so one array
still names every row.

As in macOS's System Settings, the sidebar is the one part of the window that shows what's behind
it. Where the compositor frosts behind the window, the sidebar cuts itself out of the window's
opaque background (a `Mask` fill) and lays `Material::Sidebar`'s tint over the frost; nothing
between `app_window` and the sidebar may then be a `Rectangle`. Where it doesn't, the sidebar is
an opaque panel (`sys.color.sidebar.bg.active`), paler with greyer labels
(`sys.color.sidebar.bg.inactive`, `sys.color.sidebar.label.inactive`) while another window has the
focus.

amane's side, its normal windows, is [`windows/app-windows.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/windows/app-windows.md).
Whether the window is frosted is read from the config: the shell's generated niri config defines
the window material with the `window-rule` that frosts an amane app's windows
(`layer_rules::APP_IDS`; see [layer-rules.md](../integration/layer-rules.md)). An app that should
be frosted is added there.
