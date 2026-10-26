# Saved Shader Lab looks (`amane_kit::lab`)

The Shader Lab ([development/shader-lab.md](../development/shader-lab.md)) saves each look as
`apps/shader-lab/configs/<name>.json`. kit's build embeds every config and every effect folder,
so the shell puts a saved look on a rectangle with one call:

```rust,ignore
use amane_kit::lab;

let pill = Rectangle::new().width(330.0).height(105.0).radius(52.5);

// None when the look isn't saved or its effect won't load
let styled = match lab::look("dock-glass", 330.0, 105.0, 52.5) {
    Some(look) => look.apply(pill),
    None => Stack::new(vec![boxed(pill)]),
};
```

`Look::apply(rectangle) -> Stack` adds the look's drop shadows behind it, its surface (tint,
frost and glow, for glass), its shader or effect, and its inner shadows over it.
`Look::style(rectangle)` does the same without the shadows. The size and radius you pass are the
ones the effect is laid out for. Glass effects read what the window drew behind the rectangle,
so draw the backdrop in the same window (and inside the same clip, if there's one).
`kit/examples/lab_look.rs` shows a look over the wallpaper:
`cargo run -p amane-kit --example lab_look`.

Shaders are written once to `$XDG_RUNTIME_DIR/amane-shell/lab/effects/`, as amane loads them from
files.

## Also in the module

| Item | Is |
|---|---|
| `lab::saved()` | the saved looks' names |
| `lab::config(name)` | a saved [`Config`] |
| `lab::manifest(effect)` | an effect's manifest, with what it `extends` filled in |
| `lab::files()`, `lab::file(path)` | every embedded file, by its path under `apps/shader-lab` |
| `lab::write_effects(root)` | writes the embedded effect folders under `root/effects` |
| `lab::shadows::{behind, in_front}` | a shadow list as layers, as `apply` and the lab draw them |
| `lab::manifest` | the `effect.json` model: params, colours, gradient, passes, `pack` into `values` |

## A config

```json
{
  "name": "Dock glass",
  "notes": "A refracting glass pill with soft layered shadows.",
  "effect": "liquid-glass",
  "subject": { "kind": "pill", "size": 150.0, "x": 0.0, "y": 120.0 },
  "backdrop": "wallpaper",
  "values": { "refraction": 26.0, "bezel": 22.0 },
  "colors": { "tint": "#ffffff14" },
  "gradient": { "preset": 0, "stops": null },
  "shadows": [
    { "inner": false, "color": "#00000033", "x": 0.0, "y": 12.0, "blur": 28.0, "spread": -4.0, "opacity": 1.0 }
  ]
}
```

- **`values` and `colors`:** keyed by the manifest's param and colour keys. Anything left out
  is at the manifest's default; the lab writes everything out when it saves.
- **`gradient.stops`:** `[position, red, green, blue, alpha]` from 0 to 1, and wins over
  `preset`.
- **`subject.kind`:** `rounded`, `pill`, `circle`, `text`, `image`, `fill` or `photo`.
- **`backdrop`:** `dark`, `light`, `grey` or `wallpaper`.

The subject and backdrop are only for the lab; `look` takes the size you give it.
