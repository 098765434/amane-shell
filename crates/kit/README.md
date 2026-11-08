# emaki-kit

The look of [emaki](../../README.md) as a library: the design tokens from Penpot, the theme they resolve against (mode, density, accent, text scale), and the controls built from them. The shell uses it, and so can apps that should look like the shell, like its settings app.

```toml
[dependencies]
amane = { git = "https://github.com/emaki-desktop/amane", rev = "…" }
emaki-kit = { git = "https://github.com/emaki-desktop/emaki" }
```

```rust
use emaki_kit::app::{app, app_window, standard_shortcut};
use emaki_kit::buttons::Switch;

fn main() {
    app("emaki-example").normal_window("main", main_window).run();
}

fn main_window() -> amane::Window {
    app_window(Switch::new("example.switch", true).build())
        .title("Example")
        .on_shortcut(standard_shortcut)
}
```

`cargo run -p emaki-kit --example gallery` shows every control in one window.

## Docs

- [Controls](../../docs/developer/kit/controls.md): buttons, switch, checkbox, radio, dropdown, menu, stepper and
  control sizes; `cargo run -p emaki-kit --example controls` shows them all.
- [Components](../../docs/developer/kit/components.md): card, list, slider, dialog, scroll view and the other
  building blocks, and the modules under them.
- [App scaffolding](../../docs/developer/kit/app.md): `app`, `app_window`, text fields and the sidebar.
- [Motion](../../docs/developer/kit/motion.md): the springs, keyed list motion, `Kinetic` and `Pager`.
- [Colour picker](../../docs/developer/kit/color-picker.md): a colour from a square, a hue strip or hex, and
  the HSV helpers.
- [Shader Lab looks](../../docs/developer/kit/lab.md): a saved look from the Shader Lab, put on a rectangle.
- [Design tokens](../../docs/developer/design-system/tokens.md), [materials](../../docs/developer/design-system/materials.md):
  how the tokens are made in Penpot and read, and the glass every surface is made of.

The tokens are generated at build time (`build.rs`) from `../design-tokens/tokens.json`. Controls
read `sys.*` tokens only (manual §10.4), so they fit any surface. Their ids key hover and motion
state across rebuilds, so each must be unique and stable.
