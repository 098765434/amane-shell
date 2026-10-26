# amane-kit

The look of [amane-shell](../README.md) as a library: the design tokens from Penpot, the theme they resolve against (mode, density, accent, text scale), and the controls built from them. The shell uses it, and so can apps that should look like the shell, like its settings app.

```toml
[dependencies]
amane = { git = "https://github.com/098765434/amane", rev = "…" }
amane-kit = { git = "https://github.com/098765434/amane-shell" }
```

```rust
use amane_kit::app::{app, app_window, standard_shortcut};
use amane_kit::buttons::Switch;

fn main() {
    app("amane-example").normal_window("main", main_window).run();
}

fn main_window() -> amane::Window {
    app_window(Switch::new("example.switch", true).build())
        .title("Example")
        .on_shortcut(standard_shortcut)
}
```

`cargo run -p amane-kit --example gallery` shows every control in one window.

## What's in it

| Module | Has |
|---|---|
| `app` | `app(app_id)`, `app_window`, `standard_shortcut`, `text_field` and `field_box`, `sidebar`, `Dialog` |
| `buttons` | `PushButton`, `IconButton`, `SegmentedButton`, `Switch`, `Checkbox`, `Radio`, `DrawnButton` |
| `dropdown` | `Dropdown`: one choice out of a list, in a popover menu |
| `slider`, `list`, `card`, `divider`, `app_bar`, `scroll_view`, `flow` | the shell's shared building blocks |
| `tooltip` | hover labels; for now they open over layer windows only |
| `tokens`, `theme` | the design tokens (`sys::*` and the component sets) and what they resolve with |
| `layout`, `icons`, `color`, `motion`, `kinetic`, `pager`, `glass`, `glass_region`, `edge_reveal`, `app_icon` | the pieces under those |

Controls read `sys.*` tokens only (manual §10.4), so they fit any surface. Their ids key hover and motion state across rebuilds, so each must be unique and stable.

The tokens are generated at build time from `../design-tokens/tokens.json`. See [docs/design-system/tokens.md](../docs/design-system/tokens.md) for how they're made and read.

An app window's surface is `app::window_bg()`: the popover colour made solid, until Penpot has a window surface token.
