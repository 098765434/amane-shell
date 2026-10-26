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

## Docs

- [Controls](../docs/kit/controls.md): buttons, switch, checkbox, radio, dropdown and control sizes.
- [Components](../docs/kit/components.md): card, list, slider, dialog, scroll view and the other
  building blocks, and the modules under them.
- [App scaffolding](../docs/kit/app.md): `app`, `app_window`, text fields and the sidebar.
- [Motion](../docs/kit/motion.md): the springs, keyed list motion, `Kinetic` and `Pager`.
- [Design tokens](../docs/design-system/tokens.md), [materials](../docs/design-system/materials.md):
  how the tokens are made in Penpot and read, and the glass every surface is made of.

The tokens are generated at build time (`build.rs`) from `../design-tokens/tokens.json`. Controls
read `sys.*` tokens only (manual §10.4), so they fit any surface. Their ids key hover and motion
state across rebuilds, so each must be unique and stable.
