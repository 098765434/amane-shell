# Shader Lab

`apps/shader-lab` is a playground for amane shader effects (`Rectangle::effect`, see amane's
`docs/fork/graphics/effects.md`). It shows one effect on a rounded rectangle, pill, circle, text
or image, over a dark, light, grey or wallpaper backdrop, with a control for every parameter the
effect lists.

```sh
nix develop -c cargo run -p amane-shader-lab -- --effects apps/shader-lab/effects
```

With `--effects`, the lab reads the source tree, so saving a `.wgsl` file or an `effect.json`
shows the change straight away. Without it, the built-in effects are written to
`$XDG_RUNTIME_DIR/amane-shader-lab/`. Shader compile errors and manifest errors show in red under
the controls. A broken shader keeps its last good version on screen.

**Copy as Rust** puts the effect, with the values as they're set, on the clipboard as an
`Effect::new()…` expression to paste into the shell.

## An effect folder

```
effects/chrome/
  effect.json    the manifest
  blur_x.wgsl    pass 0: blurs the drawing across
  blur_y.wgsl    pass 1: then down
  chrome.wgsl    pass 2: the chrome, from the drawing and its blur
```

`effect.json` mirrors a Figma shader's `defineProperties`:

| Field | Means |
|---|---|
| `passes` | each pass's `shader` file and its `inputs`: `"content"` or the number of an earlier pass |
| `params` | `key`, `label`, `default`, `min`, `max`, `step`, `unit` and the `slot` it fills, `[row, lane]` of `values` |
| `gradient` | gradient presets: eight color rows from `colors_row`, two position rows from `positions_row`, the stop count in `count_slot` |
| `animate` | the param the Animate switch sweeps from `from` to `to` every `seconds` |
| `margin` | logical pixels the passes reach past the subject |

## Porting a Figma shader

Figma hands `render(device, frame)` its input layer and runs your own WebGPU passes. amane runs
the passes for you, so a port keeps only the WGSL fragment code:

- Each `createRenderPipeline` becomes a pass in `effect.json`. The input layer is `"content"`,
  and an intermediate texture is the number of the pass that wrote it.
- The uniform struct becomes rows of `values`, and `defineProperties` becomes `params`.
- Read inputs at `canvas_uv` (declare `@location(1) canvas_uv: vec2<f32>`). Pixel sizes are
  real pixels, so multiply logical sizes by `scale`.
- Return plain alpha. Figma's examples return premultiplied color.
- Hoist `fwidth` out of loops instead of turning off `derivative_uniformity`.
- Drive animation from the manifest's `animate`, not from `time`. A shader whose source says
  `time` redraws every frame.

The chrome port (`effects/chrome/chrome.wgsl`) has notes on what changed from the original.
