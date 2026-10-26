# Shader Lab

`apps/shader-lab` is a playground for amane shader effects (`Rectangle::effect`, see amane's
`docs/fork/graphics/effects.md`). It shows one effect on a rounded rectangle, pill, circle, text,
an icon (Image, fitted in), a picture (Photo, covering; your wallpaper by default) or the whole
preview, over a dark, light, grey or wallpaper backdrop, with a control for every parameter the
effect lists.

```sh
nix develop -c cargo run -p amane-shader-lab -- --effects apps/shader-lab/effects
```

With `--effects`, the lab reads the source tree, so saving a `.wgsl` file or an `effect.json`
shows the change straight away. Without it, the built-in effects are written to
`$XDG_RUNTIME_DIR/amane-shader-lab/`. Shader compile errors and manifest errors show in red under
the controls. A broken shader keeps its last good version on screen.

`--effect <folder>`, `--subject <n>`, `--backdrop <n>` and `--config <name>` set what it starts
on, and `--open <picker>` (like `color.tint` or `shadow.0`) opens a colour picker, which is handy
for screenshots.

## Saved looks

The top of the panel lists the saved looks. Clicking one loads it.

| Button | Does |
|---|---|
| **Save as…** | names the look and writes `apps/shader-lab/configs/<name>.json` (Enter saves) |
| **Update** (Ctrl+S) | writes the look over the file it came from |
| **Rename** | saves under a new name and removes the old file |
| **Delete** | asks, then removes the file; the look stays in the lab |
| **Copy loader line** | copies `amane_kit::lab::look("<name>", w, h, radius)` for this subject's size |

A saved file has every param and colour written out, the gradient (a preset or the look's own
stops), the subject, its position, the backdrop and the shadows. It's plain JSON for git; see
[kit/lab.md](../kit/lab.md) for the format and the loader. The build embeds every file in
`configs/`, so a saved look reaches the shell with no copying. Without `--effects`, looks save to
`~/.config/amane-shader-lab/configs`, and the panel says the shell won't see them.

## Editing a look

- **Moving:** drag the subject over the backdrop (glass bends what's under it), nudge it with
  the arrow keys (Shift for 10 px), or double-click to put it back in the middle. A click
  without a drag is still a press, for effects that take presses.
- **Colours:** click a swatch to open the picker under it: saturation and brightness square,
  hue strip, alpha strip, hex field, and the colours picked last.
- **Gradient:** the bar shows the effect's gradient with a handle per stop. Drag a handle to
  move its stop; click it again (or **Colour**) to pick its colour; double-click the bar, or **+**,
  to add a stop; **−** or Delete removes the selected one (2 to 8 stops). Editing turns a preset
  into the look's own stops.
- **Shadows** (rectangle, pill and circle): **+ Drop shadow** and **+ Inner shadow**, then each
  shadow's colour, X, Y, blur, spread and opacity, in order (↑ ↓ ✕). They're layers of their
  own, so an effect doesn't take them in, and drop shadows are cut out under the shape, so they
  don't show through glass.
- **Undo and redo:** Ctrl+Z and Ctrl+Shift+Z (or Ctrl+Y), or the buttons. Dragging one
  control is one step.
- **Time:** **Hold time** (or Space) stops every shader's clock, and **At** scrubs it while
  it's held. The corner readout shows frames per second and how long the view, the drawing and
  presenting take, averaged over the last 30 frames.

The sliders don't take the scroll wheel, so scrolling the panel never changes a value.

**Copy as Rust** puts the effect, with the values as they're set, on the clipboard as an
`Effect::new()…` expression to paste into the shell.

## The effects

| Folder | Shows |
|---|---|
| `chrome` | the metallic chrome ported from Figma |
| `chrome-window` | the chrome, reflecting what the window drew behind it (`"backdrop"`); try it over the Wallpaper backdrop |
| `spectral` | the animated spectral background from the Figma Community file "Shader Dash", drawn inside the subject's shape; try the Fill preview subject |
| `grainy-gradient` | the warped, breathing gradient with film grain from the Figma Community file "Gradient Shader Card"; press on the subject for ripples |
| `rings` | the stepped, colour-fringed light rings from the Figma Community file "Copy Shader Animation Component"; Glow only makes its black clear |
| `hatching` | the Figma shader "Hatching": the picture redrawn as lines between two colours, as thick as it's bright; starts on the Photo subject |
| `moving-blobs` | the Figma shader "Moving blobs": liquid blobs that refract, displace, bulge, hue-shift or invert the picture under them, with dispersion and a cast shadow; starts on the Photo subject |
| `glass` | the shell's own glass: `kit/src/glass.wgsl` (linked, so editing it reloads here) over a surface drawn like the shell's panels, with Dark and Light presets from the `sys` tokens; Frost stands in for niri's blur, which the lab can't show |
| `liquid-glass` | glass that refracts: inside a bezel along the edge the backdrop is read from further along the edge's normal, each colour a little apart, under the shell's tint, glow and lit rim; the shell itself can't do this until niri draws it, as a surface can't read what's behind it |
| `chrome-wallpaper` | the chrome, reflecting the wallpaper file, placed by the window's position on the output (Window x/y, Output w/h) |

The reflection reads the surface's slope from the blurred alpha (steepest at the rim) plus the
noise's ripples, and looks up the environment that far away (Warp). Soften spreads a ring of
reads, and Reflect blends the environment, shaped by the gradient's bands, over the plain
chrome. At Reflect 0 the shader returns the plain chrome unchanged. A normal window never learns
where it is on screen, so in the lab the wallpaper variant can't line up with the real
wallpaper. On the desktop layer, a widget knows its own position.

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
| `extends` | a folder whose manifest fills in every field this one leaves out; this one's `params` come first |
| `passes` | each pass's `shader` file and its `inputs`: `"content"`, `"backdrop"`, the number of an earlier pass, or `{ "image": "~/path", "blur": px }` |
| `params` | `key`, `label`, `default`, `min`, `max`, `step`, `unit` and the `slot` it fills, `[row, lane]` of `values` |
| `gradient` | gradient presets: eight color rows from `colors_row`, two position rows from `positions_row`, the stop count in `count_slot` |
| `animate` | the param the Animate switch sweeps from `from` to `to` every `seconds` |
| `presets` | named sets of param values, `{ "name", "values": { key: value } }`, picked from the Preset dropdown |
| `clicks` | presses on the subject, as rows from `rows_from` (x and y from 0 to 1 from its top left, seconds since), at most `max`, each kept `seconds`; their number goes in `count_slot` |
| `params[].options` | names for 0, 1, 2…: the param shows as a dropdown, like a Figma `select` |
| `colors` | `key`, `label`, `default` (`#rrggbb` or `#rrggbbaa`) and the `row` it fills with red, green, blue and alpha; shown as a swatch and a hex field; presets set them under `colors` |
| `suggest` | the `subject` and `backdrop` the lab switches to when the effect is picked (or started with `--effect`) |
| `shader` | a rectangle shader drawn over the subject's fill (`Rectangle::shader`), in place of `passes` |
| `surface` | draws the subject as a translucent surface: the `tint` and `glow` colours, the `frost`, `glow_blur` and `radius` params, its rectangle written to `rect_row` and its radius to `radius_slot` |
| `constants` | values no control changes: `slot` and `value` |
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
- A WebGL (GLSL ES) shader ports the same way: its `gl_FragCoord` is `vec2(uv.x, 1.0 - uv.y) * size`
  (GL counts up from the bottom), its `iResolution` is `size`, and its `iTime` is amane's `time`.
  `spectral/spectral.wgsl` is an example.
- For a loop that should rest when nobody's watching, drive it from the manifest's `animate`
  instead of `time`: a shader whose source says `time` redraws every frame.

The chrome port (`effects/chrome/chrome.wgsl`) has notes on what changed from the original.
