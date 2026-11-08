# Screenshots

The pictures the [README](../../../README.md) shows, one per slot, in both themes:

| File | Shows |
|---|---|
| `bar-dark.png`, `bar-light.png` | the status bar, Regular size, Frosted, with a workspace, the status icons and the clock |
| `dock-dark.png`, `dock-light.png` | the dock risen, with a few windows and pins |
| `launcher-dark.png`, `launcher-light.png` | the launcher with a search typed and results |
| `quick-settings-dark.png`, `quick-settings-light.png` | the Quick Settings grid |
| `settings-dark.png`, `settings-light.png` | Shell Settings on the Appearance page |

## Taking one

- **`grim`**, on the region, at **2× scale**: `grim -s 2 -g "$(slurp)" bar-dark.png`. Don't use
  `niri msg action screenshot-window`: it puts the picture on the clipboard, and layer surfaces
  such as the bar aren't windows.
- **Both themes.** Take the dark one, switch in Shell Settings › Appearance (or set
  `"mode": "light"` in `appearance.json`), and take the light one from the same place.
- **Crop** to the surface and a little wallpaper around it; the full screen shows whatever else
  is open.
- **A quiet desktop:** Do Not Disturb on, a plain wallpaper, no private names in the launcher's
  results or the notification center.
- PNG, under 1 MB each; `oxipng -o 4` if it's over.
