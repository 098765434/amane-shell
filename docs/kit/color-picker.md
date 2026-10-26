# Colour picker

`amane_kit::color_picker::ColorPicker` picks a colour by dragging, by hex, or from the colours
picked last.

```rust,ignore
use amane_kit::color_picker::ColorPicker;

ColorPicker::new("lab.tint", [0.11, 0.11, 0.12, 0.5])
    .width(260.0)
    .on_change(|rgba| set_tint(rgba))
    .build()
```

- **Square:** saturation across, brightness up, over the current hue.
- **Hue strip** and **alpha strip** (over a checkerboard).
- **Hex field:** `#rrggbb`, or `#rrggbbaa` when partly clear; Enter applies it.
- **Recent row:** the last 8 colours picked; clicking one picks it again.

Colours are `[red, green, blue, alpha]` from 0 to 1, not premultiplied. `on_change` runs on every
change, during drags too. The `id` must be unique and stable: it keys the hex field and the hue
the picker remembers, so dragging through grey or black doesn't lose the hue.

Each square and strip measures its own drag and stays still while its handle moves, as amane's
drag measurement needs (see `shell/src/components/quick_settings/edit.rs` for the same pattern).

The module also has `parse_hex`, `to_hex`, `to_color`, `rgb_to_hsv` and `hsv_to_rgb`.
