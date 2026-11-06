# macOS / Apple references — full list (2026-11-07)

shell: every tracked file. Forks: only lines the fork added (amane vs upstream/main 72e23eb; niri vs v26.04 8ed0da44). Commit history is not included.

## shell (621 lines)

```
Cargo.toml:6:description = "A macOS-flavoured Wayland desktop bar for the niri compositor, built on the amane shell library."
README.md:3:A macOS-flavoured Wayland desktop shell for the [niri](https://github.com/YaLTeR/niri) compositor, built on the
README.md:22:  (Now Playing, the timer, screen sharing, uploads), over `org.amane.StatusBar1`, as macOS 27.3
README.md:25:- **Drop**: `amane-drop`, a Dropzone-style status app: drop files on folders, apps, your phone
README.md:30:- **Launcher**: macOS Tahoe's Spotlight, with Applications, Files, Actions and a clipboard
README.md:34:- **Quick Settings**: GNOME's quick settings in the macOS Control Center look, with a page per
README.md:107:desktop's widgets and Spotlight. It writes `~/.config/amane-shell/*.json`, which the running shell
README.md:125:    launcher/                   the launcher: Spotlight's search pill, its modes and the app grid
README.md:143:apps/drop/                      Drop (amane-drop), a Dropzone-style status app (docs/shell/drop.md)
apps/assistant/Cargo.toml:6:description = "Spotlight's assistant: system commands, typed or spoken, read on this machine with Cactus (Needle 3, Whistle)."
apps/assistant/src/command.rs:2://! already has (Control Center's switches, volume and brightness, apps,
apps/assistant/src/command.rs:11:    /// One of Control Center's switches, by its control's name
apps/assistant/src/lib.rs:1://! Spotlight's assistant, the part the shell and the daemon share: what a
apps/assistant/src/main.rs:1://! `amane-assistant`: Spotlight's assistant's engine, a user service beside
apps/assistant/src/rules.rs:41:/// Not a word alone, nor the bare name of a switch or an app (Spotlight
apps/assistant/src/rules.rs:511:        assert_eq!(reads("firefox"), None, "a bare name is Spotlight's");
apps/assistant/src/snapshot.rs:23:/// One of Control Center's switches.
apps/assistant/train/gen.py:398:    "start a business", "launch date of the iphone", "fire up the grill recipes",
apps/drop/Cargo.toml:6:description = "A Dropzone-style status bar app: a grid of folders, apps and actions to drop files on, and a Drop Bar to hold them."
apps/drop/amane-drop.desktop:10:Keywords=dropzone;shelf;upload;share;sftp;s3;imgur;
apps/drop/src/grid.rs:1://! The grid, as Dropzone's: a card that hangs from the bar under Drop's
apps/drop/src/main.rs:1://! amane-drop: a status bar app in the manner of Aptonic's Dropzone. Its
apps/drop/src/main.rs:74:/// action on the files, numbered from 1 in the grid showing, as Dropzone's
apps/drop/src/share.rs:1://! Uploads, as Dropzone's sharing actions: 0x0.st, Imgur, an SFTP server
apps/drop/src/shared.rs:1://! Recently Shared, as Dropzone's menu keeps it: the last links uploads
apps/drop/src/tasks.rs:3://! Cancel, as Dropzone's tasks show in its grid and menu bar item. A name
apps/messages/src/bubble.rs:16:use amane_kit::macform::theme as mac;
apps/messages/src/bubble.rs:232:        Look::Received => Box::new(path.fill(mac::bubble_received())),
apps/messages/src/bubble.rs:237:        Look::Received => mac::label_primary(),
apps/messages/src/compose.rs:21:use amane_kit::macform::theme as mac;
apps/messages/src/compose.rs:215:        mac::tint()
apps/messages/src/compose.rs:217:        mac::field_border()
apps/messages/src/compose.rs:224:        .fill(mac::field_fill())
apps/messages/src/compose.rs:303:        .fill(with_alpha(mac::label_primary(), 0.07))
apps/messages/src/compose.rs:316:                    mac::label_primary()
apps/messages/src/input.rs:18:use amane_kit::macform::theme as mac;
apps/messages/src/input.rs:119:        .radius(Radius::Fixed(mac::group_radius()))
apps/messages/src/input.rs:120:        .fill(mac::group_fill())
apps/messages/src/input.rs:121:        .border(1.0, mac::field_border())
apps/messages/src/main.rs:35:use amane_kit::macform::theme as mac;
apps/messages/src/main.rs:327:        .style(mac::page_title_font())
apps/messages/src/main.rs:328:        .color(mac::label_primary())
apps/messages/src/new_message.rs:18:use amane_kit::macform::theme as mac;
apps/messages/src/new_message.rs:275:            .color(mac::label_primary())
apps/messages/src/thread.rs:26:use amane_kit::macform::{self, MenuItem, theme as mac};
apps/messages/src/thread.rs:460:    macform::context_menu("messages.thread.menu", items, move |index| {
apps/messages/src/thread.rs:616:        .fill(state_fill(key, mac::bubble_received(), false))
apps/messages/src/thread.rs:624:                    mac::label_primary(),
apps/messages/src/thread.rs:626:                boxed(Text::new(label).style(style).color(mac::label_primary())),
apps/messages/src/thread.rs:652:                with_alpha(mac::label_primary(), 0.06),
apps/mirror/src/apps.rs:1://! The phone's apps for Spotlight: `amane-mirror --list-apps --serial
apps/mirror/src/main.rs:14://! writes the phone's apps for Spotlight and exits. `--camera front|back`
apps/mirror/src/main.rs:38:use amane_kit::macform::theme as mac;
apps/mirror/src/main.rs:361:            .style(mac::page_title_font())
apps/mirror/src/main.rs:379:            amane_kit::macform::MacPushButton::new("amane.mirror.retry", "Try Again")
apps/settings/src/about.rs:10:use amane_kit::macform::theme as mac;
apps/settings/src/about.rs:31:/// The app's icon, name and version, as macOS's About shows them.
apps/settings/src/about.rs:45:            left: mac::row_pad_h(),
apps/settings/src/about.rs:56:                                .style(mac::page_title_font())
apps/settings/src/about.rs:57:                                .color(mac::label_primary()),
apps/settings/src/about.rs:61:                                .style(mac::subtitle_font())
apps/settings/src/about.rs:62:                                .color(mac::label_secondary()),
apps/settings/src/about.rs:79:        ("Spotlight", Config::<Launcher>::path()),
apps/settings/src/about.rs:92:/// as macOS shortens a path: its start and its file name stay.
apps/settings/src/about.rs:122:    let room = crate::page_width() - 2.0 * mac::row_pad_h() - 150.0;
apps/settings/src/about.rs:123:    let fits = |text: &str| amane_kit::layout::text_width(text, mac::subtitle_font()) <= room;
apps/settings/src/appearance.rs:10:use amane_kit::macform::theme as mac;
apps/settings/src/appearance.rs:11:use amane_kit::macform::{ColorWells, ThumbnailPicker};
apps/settings/src/appearance.rs:17:/// Accents saved before macOS 27's colours, and the entry that shows for
apps/settings/src/appearance.rs:53:    let width = crate::page_width() - 2.0 * mac::row_pad_h();
apps/settings/src/desktop.rs:8:use amane_kit::macform::ThumbnailPicker;
apps/settings/src/desktop.rs:9:use amane_kit::macform::theme as mac;
apps/settings/src/desktop.rs:28:    let width = crate::page_width() - 2.0 * mac::row_pad_h();
apps/settings/src/displays/night.rs:20:use amane_kit::macform::theme as mac;
apps/settings/src/displays/night.rs:318:                    mac::control_height(),
apps/settings/src/displays/night.rs:337:                    mac::control_height(),
apps/settings/src/displays/night.rs:363:                .style(mac::control_font())
apps/settings/src/displays/night.rs:364:                .color(mac::label_secondary())
apps/settings/src/displays/sections.rs:16:use amane_kit::macform::MacLeadingIcon;
apps/settings/src/displays/sections.rs:17:use amane_kit::macform::theme as mac;
apps/settings/src/displays/sections.rs:344:    let inner = page_width() - mac::row_pad_h() * 2.0 - tile - 10.0;
apps/settings/src/displays/sections.rs:739:            mac::control_height(),
apps/settings/src/displays/sections.rs:777:    let items = ids.iter().map(amane_kit::macform::MenuItem::new).collect();
apps/settings/src/displays/sections.rs:779:    amane_kit::macform::MacPopUpButton::new("settings.displays.vrr-open-apps")
apps/settings/src/displays/sections.rs:790:/// The scales offered, as macOS's "Looks like" choices.
apps/settings/src/displays/sections.rs:1127:            mac::control_height(),
apps/settings/src/displays/sections.rs:1156:            mac::control_height(),
apps/settings/src/displays/sections.rs:1179:            mac::control_height(),
apps/settings/src/launcher.rs:1://! Spotlight, the shell's launcher: which sources its search draws on, how
apps/settings/src/launcher.rs:10:use amane_kit::macform::ThumbnailPicker;
apps/settings/src/launcher.rs:11:use amane_kit::macform::theme as mac;
apps/settings/src/launcher.rs:44:    .width(crate::page_width() - 2.0 * mac::row_pad_h())
apps/settings/src/main.rs:34:use amane_kit::macform::MacSegmentedNav;
apps/settings/src/main.rs:35:use amane_kit::macform::theme as mac;
apps/settings/src/main.rs:180:const SPOTLIGHT: usize = 7;
apps/settings/src/main.rs:204:        "spotlight" | "launcher" => SPOTLIGHT,
apps/settings/src/main.rs:299:        // launched again (Spotlight, a notification) while running: show that page
apps/settings/src/main.rs:318:        Page::new(ICON_SEARCH, "Spotlight").accent(Accent::Purple),
apps/settings/src/main.rs:367:    // Ctrl+F searches, as macOS's Command-F does
apps/settings/src/main.rs:452:                SPOTLIGHT => (
apps/settings/src/main.rs:453:                    "Spotlight",
apps/settings/src/main.rs:454:                    "settings.scroll.spotlight",
apps/settings/src/main.rs:634:            .height(Size::Fixed(mac::control_height() + sys::space::gap::s()))
apps/settings/src/main.rs:713:                .style(mac::page_title_font())
apps/settings/src/main.rs:714:                .color(mac::label_primary())
apps/settings/src/main.rs:821:    count as f32 * mac::control_height()
apps/settings/src/passwords.rs:356:            "settings.passwords.spotlight",
apps/settings/src/passwords.rs:357:            "Passwords in Spotlight",
apps/settings/src/passwords.rs:358:            settings.show_in_spotlight,
apps/settings/src/passwords.rs:359:            |on| change_passwords(|passwords| passwords.show_in_spotlight = on),
apps/settings/src/passwords.rs:373:                "fcitx5 or IBus is running, so the shell can't offer fill in apps. Spotlight's \
apps/settings/src/passwords.rs:394:         Spotlight, type pw and a name.",
apps/settings/src/previews.rs:246:/// Spotlight's apps as a grid of icons or a list of rows.
apps/settings/src/previews.rs:342:                        .style(amane_kit::macform::theme::page_title_font())
apps/settings/src/search.rs:1://! Searching Shell Settings, as the field over macOS's sidebar does: what's
apps/settings/src/search.rs:354:        &["control center", "wifi", "bluetooth", "volume", "toggles"],
apps/settings/src/search.rs:362:    // Spotlight
apps/settings/src/search.rs:364:        "spotlight",
apps/settings/src/search.rs:365:        "Spotlight",
apps/settings/src/search.rs:369:        "spotlight",
apps/settings/src/search.rs:375:        "spotlight",
apps/settings/src/search.rs:381:        "spotlight",
apps/settings/src/search.rs:387:    page("drop", "Drop", &["dropzone", "amane-drop", "grid", "shelf", "share"]),
apps/settings/src/search.rs:545:        ("spotlight", "Spotlight"),
apps/settings/src/search.rs:713:            "spotlight",
apps/settings/src/status_bar.rs:12:use amane_kit::macform::ThumbnailPicker;
apps/settings/src/status_bar.rs:13:use amane_kit::macform::theme as mac;
apps/settings/src/status_bar.rs:88:    let width = crate::page_width() - 2.0 * mac::row_pad_h();
apps/settings/src/status_bar.rs:224:    let width = crate::page_width() - 2.0 * mac::row_pad_h();
apps/settings/src/status_bar.rs:515:                "What's going on now, as macOS's menu bar has it: what plays, a timer, screen \
apps/settings/src/status_bar.rs:535:/// macOS's "Allow in the Menu Bar".
apps/settings/src/system/about.rs:195:/// macOS's About This Mac opens.
apps/settings/src/system/about.rs:198:    use amane_kit::macform::theme as mac;
apps/settings/src/system/about.rs:218:        .style(mac::page_title_font())
apps/settings/src/system/about.rs:219:        .color(mac::label_primary()),
apps/settings/src/system/about.rs:224:                .style(mac::subtitle_font())
apps/settings/src/system/about.rs:225:                .color(mac::label_secondary()),
apps/settings/src/system/about.rs:234:            left: mac::row_pad_h(),
apps/settings/src/system/controls.rs:12:use amane_kit::macform::MacStepper;
apps/settings/src/system/controls.rs:13:use amane_kit::macform::theme as mac;
apps/settings/src/system/controls.rs:220:        Some(button) => row.detail(Row::new(vec![boxed(button.build())]), mac::control_height()),
apps/settings/src/system/defaults.rs:23:use amane_kit::macform::MenuItem;
apps/settings/src/system/history.rs:8:use amane_kit::macform::theme as mac;
apps/settings/src/system/history.rs:44:            row = row.detail(actions, mac::control_height());
apps/settings/src/system/services.rs:15:use amane_kit::macform::theme as mac;
apps/settings/src/system/services.rs:156:        _ => mac::label_tertiary(),
apps/settings/src/system/services.rs:367:            row = row.detail(Row::new(vec![boxed(mover.build())]), mac::control_height());
apps/settings/src/system/services.rs:393:                        .detail(Row::new(vec![boxed(mover.build())]), mac::control_height());
apps/shader-lab/effects/liquid-glass/effect.json:2:  "name": "Liquid glass (refracting)",
docs/README.md:3:amane-shell is a macOS-flavoured Wayland desktop shell for niri: a status bar, a dock, Spotlight,
docs/README.md:54:| Drop | `amane-drop`, a Dropzone-style status app: a grid of folders, apps and actions, and a Drop Bar | [shell/drop.md](shell/drop.md) |
docs/README.md:56:| Launcher | macOS Tahoe's Spotlight: search, Applications, Files, Actions, Clipboard | [shell/launcher.md](shell/launcher.md) |
docs/README.md:57:| Passwords | Bitwarden (rbw) fill: a popup by any app's password field, checked by niri; Spotlight's Passwords mode | [shell/passwords.md](shell/passwords.md) |
docs/README.md:58:| Assistant | system commands typed or spoken in Spotlight, read on this machine by Cactus (Needle 3, Whistle) in the `amane-assistant` daemon | [shell/assistant.md](shell/assistant.md) |
docs/README.md:60:| Quick Settings | GNOME's quick settings in the macOS Control Center look, with Edit Controls | [shell/quick-settings.md](shell/quick-settings.md) |
docs/README.md:63:| Alert | macOS's alert in the middle of the screen, for questions like "Restart now?" | [shell/quick-settings.md#the-alert](shell/quick-settings.md#the-alert) |
docs/README.md:64:| Shell Settings | the settings app: appearance, the desktop's widgets, Spotlight, and the System pages that manage the NixOS configuration | [shell/settings-app.md](shell/settings-app.md) |
docs/README.md:117:| [archive/spec-macos27-variables.md](archive/spec-macos27-variables.md): the macOS 27 kit's labels, fills, materials and control sizes | [design-system/materials.md](design-system/materials.md), [kit/…
docs/archive/spec-design-tokens-v2.md:125:| **Font fallback doesn't happen.** Only the first family of a stack is used. amane passes it to fontconfig's `find`, which always returns *some* font, so on a machine without SF…
docs/archive/spec-design-tokens-v2.md:434:The 3.0 threshold keeps white on mid blues, as macOS and libadwaita do (white on #0a84ff is
docs/archive/spec-design-tokens-v2.md:461:user's machine. The token value is the same; only the fallback differs (§12 on SF Pro).
docs/archive/spec-design-tokens-v2.md:713:- **Fonts in Penpot.** "SF Pro Text" needs a team upload (licence permitting), or designers
docs/archive/spec-design-tokens-v2.md:757:5. **Font stacks.** §8 and test 16. Visible only without SF Pro Text installed.
docs/archive/spec-design-tokens-v2.md:789:7. **Fonts (PR 5):** with SF Pro Text absent and Inter installed, the debug log names Inter.
docs/archive/spec-macos27-variables.md:1:# Spec: adopting the macOS 27 kit's variables
docs/archive/spec-macos27-variables.md:24:"macOS 27 (Community)". The file isn't Apple's own, but its values match Apple's published
docs/archive/spec-macos27-variables.md:25:system colours and AppKit control sizes. This spec says which of its variables the shell takes
docs/archive/spec-macos27-variables.md:34:| Kit | one | 18 | the 8 Liquid Glass values only. The rest (`Component Fill`, `Section Stroke`, `Link` and the like) is scaffolding for the file's own designers |
docs/archive/spec-macos27-variables.md:42:### 1.1 Apple's vibrant colours are opaque greys, one blend per appearance
docs/archive/spec-macos27-variables.md:63:### 1.2 Bug: niri's plus-darker is a multiply, not Apple's subtraction
docs/archive/spec-macos27-variables.md:66:a multiply. Apple's formula is `D − a·(1 − c)`, clamped at 0. On a mid-grey backdrop
docs/archive/spec-macos27-variables.md:69:- Apple: `0.5 − 0.5 = 0.0`;
docs/archive/spec-macos27-variables.md:114:(3% and 2%). Apple's API has four label levels and three fill levels, and `material.rs` keeps
docs/archive/spec-macos27-variables.md:124:  in Apple's system, it's for decoration such as placeholders and disabled glyphs, so exempt it
docs/archive/spec-macos27-variables.md:174:- `ref.color.accent.seed` (`#0a84ff`, the older iOS blue) becomes an alias of
docs/archive/spec-macos27-variables.md:226:    which is the Liquid Glass press.
docs/archive/spec-macos27-variables.md:266:- the Settings app's content uses **Medium**, as macOS apps do;
docs/archive/spec-macos27-variables.md:268:  macOS 26's Control Center does;
docs/archive/spec-macos27-variables.md:281:## 7. Liquid Glass parameters
docs/archive/spec-macos27-variables.md:296:Medium and large glass blurs heavily (16). This matches Apple's rule that larger glass is more
docs/design-system/materials.md:3:The shell's glass follows the materials in Apple's Human Interface Guidelines. A piece of glass
docs/design-system/materials.md:16:- **Glass**, Apple's Liquid Glass, is the layer of controls and navigation that floats over
docs/design-system/materials.md:19:  backdrop. That is Apple's 35% dimming layer.
docs/design-system/materials.md:24:  the macOS 27 kit's frost: 6 for small glass, 16 for medium and large. Without `glass-small` in
docs/design-system/materials.md:28:  1.8, as macOS's materials do (Gaussian 30 pt, saturation 1.6 to 2.4).
docs/design-system/materials.md:32:  as 40%. Apple's materials do the same with a darken or lighten layer. Where niri doesn't tone the
docs/design-system/materials.md:39:| `Material` | Apple's | Family | Thickness | Adapts | Used by |
docs/design-system/materials.md:41:| `Control` | Control Center module, tab bar | glass | thin | yes | Quick Settings modules, the dock |
docs/design-system/materials.md:68:  `layer_rules::APP_IDS`. As in macOS's System Settings, only the sidebar shows it: the window
docs/design-system/materials.md:70:  (a `Mask` fill) and lays `sys.color.material.sidebar` over the frost. That is Apple's sidebar
docs/design-system/materials.md:74:  opaque, calmer, as AppKit's inactive windows are. The window itself doesn't fade.
docs/design-system/materials.md:87:- **A panel flips together, and seldom.** Apple: "small elements like navbars and tabbars …
docs/design-system/materials.md:95:  with white labels, as macOS's Control Center is, while dark glass keeps white labels until
docs/design-system/materials.md:104:  lift (tone 0.9), so the sheet shows its backdrop's colour the way Apple's does. A plain
docs/design-system/materials.md:109:  macOS 26 glass shaders do, so a rim continues what lies beside it (no stretched ring). The
docs/design-system/materials.md:110:  light comes from the top (`light-angle 270`) with an opposite fill (`far-rim 0.5`), Apple's
docs/design-system/materials.md:113:  saturated 2.0, at the top of Apple's 1.6–2.4, so the backdrop comes through vivid.
docs/design-system/materials.md:116:  `separator()`. These are translucent, so the backdrop tints them, as Apple's label colours
docs/design-system/materials.md:125:  The values are the macOS 27 kit's vibrant greys as translucent white (dark) and black (light),
docs/design-system/materials.md:141:  materials off, as Apple's Reduce Transparency does.
docs/design-system/materials.md:146:  instead of building up (Apple: avoid animating into and out of blurs).
docs/design-system/materials.md:152:  subtracted from it (Apple's plus-lighter and plus-darker), so they take on its colour, as
docs/design-system/materials.md:153:  Apple's vibrancy does. Solid glass and higher
docs/design-system/penpot-backlog.md:120:    on every material, as Apple's are.
docs/design-system/penpot-backlog.md:135:  Penpot, or the next export puts SF Pro Text back.
docs/design-system/penpot-backlog.md:142:- The macOS 27 kit's values were added to `tokens.json` by hand
docs/design-system/penpot-backlog.md:143:  ([spec-macos27-variables.md](../archive/spec-macos27-variables.md)). Recreate them in Penpot under the same
docs/design-system/penpot-backlog.md:150:  - `sys.color.status.critical.*` now holds macOS 27's red, light `#ff383c`, dark `#ff4245`,
docs/design-system/penpot-backlog.md:152:  - `ref.color.accent.seed` is macOS 27's blue `#0091ff`, with `dim` `#0064b3` and `bright`
docs/design-system/penpot-backlog.md:166:  - macOS's blur, saturation and tone ([materials.md](materials.md)): `material.frost.blur-passes`
docs/design-system/penpot-backlog.md:171:  - Apple's edge and light: `material.glass` `.refraction` 16, `.bevel` 12, `.dispersion` 0.3,
docs/design-system/tokens.md:92:Apple) theme their chrome by *surface roles*, not by widget, which is exactly the semantic
docs/design-system/tokens.md:168:| `ref.color.<hue>.<L>` | `color` | same rule for status hues (none today: the status red is macOS 27's, per mode, in `sys.color.status.*`) | `ref.color.red.58` #ff453a |
docs/design-system/tokens.md:169:| `ref.color.accent.seed` | `color` | the default accent | #0091ff (macOS 27's blue) |
docs/design-system/tokens.md:248:| `sys.size.control.<size>`, `sys.size.switch.<size>.*`, `sys.size.checkbox.<size>`, `sys.size.radio.<size>.dot`, `sys.size.slider.<size>.knob-*`, `sys.radius.control.<size>`, `sys.radius…
docs/integration/ipc.md:60:| `listen start` | opens Spotlight on a fresh search and listens for a command |
docs/integration/ipc.md:189:    Mod+Space hotkey-overlay-title="Spotlight" {
docs/integration/status-apps.md:4:their own items in the bar, like macOS's status items, and Live Activities, like macOS 27.3's.
docs/integration/writing-a-status-app.md:143:These follow Apple's guidance for Live Activities, fitted to the bar:
docs/kit/app.md:26:| `window_bg()` | the opaque window background, `sys.color.window.bg`, as macOS draws a window's content |
docs/kit/app.md:29:| `field_box(id, input, width)` | the box a text field sits in, in the macform look: 24 px tall, radius 6, `fieldFill` in a 1 px `fieldBorder` that takes the accent while it has the keys |
docs/kit/app.md:42:on a `MacLeadingIcon` tile of that colour, as macOS's System Settings draws it), the shown one filled
docs/kit/app.md:45:as macOS groups its sidebar; it can't be selected but takes an index like a page, so one array
docs/kit/app.md:48:As in macOS's System Settings, the sidebar is the one part of the window that shows what's behind
docs/kit/components.md:6:the shell's macOS look. They read `sys.*` tokens only, so any component can use them.
docs/kit/components.md:33:`.blur(true)` frosts what's below with `sys.effect.backdrop-blur`, a macOS departure from M3. It
docs/kit/components.md:110:`form` lays settings out as macOS's System Settings does, and is what Shell Settings' pages are
docs/kit/components.md:141:- `macform::ThumbnailPicker::new(id, width, height)`: choices as small pictures (`.option(widget,
docs/kit/components.md:144:- `macform::ColorWells::new(id, colors, chosen)`: colors as dots, `None` for a multicolor
docs/kit/components.md:150:Quick Settings' glass cards fill it with the panel's `fg`, as macOS does, a documented departure
docs/kit/components.md:164:`.large(glyph)` is the macOS Control Center slider: a track a full control tall with the glyph
docs/kit/components.md:176:hangs under the control's surface and is centred above the pointer, as macOS's are, sliding inward
docs/kit/components.md:231:`kit::dialog::Dialog` is macOS 26's alert (`dialog/dialog-*.css`): a 260-wide card of thick glass
docs/kit/components.md:289:- `material.rs` holds the materials, Apple's: glass by role (`Material::Control`, `Sheet` …), the
docs/kit/components.md:318:  `AMANE_ADAPTIVE_GLASS=1`) turns small glass light or dark with its backdrop, as Apple's small
docs/kit/controls.md:5:in the shell's macOS/iOS look and sized like its other controls. They read `sys.*` tokens
docs/kit/controls.md:74:in the macform look (macOS's): a 16 px mark (times the density) with the label 3 px after it in
docs/kit/controls.md:79:  around a drawn checkmark (`macform::symbol`). The fill crossfades over 300 ms. A setting that
docs/kit/controls.md:88:`dropdown::Dropdown::new(id, options, selected)` is one choice out of a list, drawn like macOS's
docs/kit/controls.md:89:pop-up button (the macform pull-down): a `controlFill` button, radius 6, with the chosen option
docs/kit/controls.md:102:`macform::context_menu(id, items, |index| …)` opens the same menu at the pointer, its top-left
docs/kit/controls.md:108:`control_size` has the macOS 27 kit's five sizes, Mini, Small, Medium, Large and XL. The switch,
docs/kit/controls.md:115:## macform
docs/kit/controls.md:117:`amane_kit::macform` holds the macOS-style controls ported from the old Quickshell `macform` set.
docs/kit/controls.md:120:where it needs them. `cargo run -p amane-kit --example macform` shows them beside the kit's other
docs/kit/controls.md:121:controls (app_id `amane-macform-gallery`, so it runs beside the bar; `MACFORM_PAGE=<n>` opens on
docs/kit/controls.md:131:- `macform::theme` maps the original MacTheme to `sys.*` tokens where the dark values match
docs/kit/controls.md:135:- `macform::symbol` draws the SF Symbols the controls use (chevrons, checkmark, ellipsis,
docs/kit/motion.md:58:- When the fingers lift, it coasts as far as their speed carries it (an iOS-like projection eased
docs/shell/assistant.md:3:Spotlight reads a query as a system command and offers it as the card over the
docs/shell/assistant.md:10:- Control Center's switches: Wi‑Fi, Bluetooth, Focus, Dark Style, Night Light, Airplane
docs/shell/assistant.md:20:Questions and anything else go to Spotlight's usual rows and the web search.
docs/shell/assistant.md:33:   - the bare name of a switch or an app ("night light", "firefox"): Spotlight's rows
docs/shell/assistant.md:50:3. **Spotlight's rows**, as always. The assistant stands aside for:
docs/shell/assistant.md:83:`assistant listen start` opens Spotlight on a fresh search and starts listening:
docs/shell/assistant.md:88:- What was heard is typed into the field, so Spotlight searches it.
docs/shell/assistant.md:92:Escape, or closing Spotlight, cancels.
docs/shell/assistant.md:160:  chat engine (`cactus-engine` 2.x) is ARM and Apple only.
docs/shell/context-menu.md:3:A reusable, macOS-style context menu with arbitrarily nested submenus
docs/shell/dock.md:3:The macOS Big Sur dock, for the windows open on every workspace (`src/surfaces/dock/`). It's one
docs/shell/dock.md:40:Under the pointer (or held files), the icons swell as on macOS: 1.5× for the hovered icon, 1.2×
docs/shell/drop.md:3:A status bar app in the manner of Aptonic's Dropzone (`apps/drop`). Its item in the bar opens a
docs/shell/drop.md:98:| `amane-drop run <tile> <file>…` | what letting the files go on tile `<tile>` (from 1, in the grid showing) does, as Dropzone's `dz run` |
docs/shell/drop.md:130:there even when the system lacks them, and it installs `amane-drop.desktop`, so Spotlight finds
docs/shell/launcher.md:3:macOS Tahoe's Spotlight (`src/surfaces/launcher/`): a glass search pill high on the screen,
docs/shell/launcher.md:5:Applications mode is the Launchpad grid the launcher started as, after KDE's
docs/shell/launcher.md:6:[Tahoe Launcher](https://github.com/EliverLara/TahoeLauncher). It's opened over IPC from a
docs/shell/launcher.md:13:  ("Spotlight Search") and, while there's a query, a button that clears it.
docs/shell/launcher.md:91:What the launcher does without opening an app (`actions.rs`), Spotlight's Actions:
docs/shell/launcher.md:95:| Control Center | each of Quick Settings' toggles and buttons this machine has, as it is now ("Turn Night Light On" while it's off, Screenshot), Set Power Mode (a choice of the modes the daemon…
docs/shell/launcher.md:125:Spotlight's Files (`services/launcher/file_search.rs`). A search runs off the UI thread a moment after
docs/shell/launcher.md:159:Spotlight's clipboard history (`services/launcher/clipboard_history.rs`). It's **off until asked for**:
docs/shell/launcher.md:185:changes (`clipboard.enabled` in `launcher.json`, or Shell Settings' Spotlight page).
docs/shell/launcher.md:302:  Shell Settings' **Spotlight** page changes them.
docs/shell/notifications.md:3:Toasts, a notification center and Do Not Disturb, in the style of macOS and iOS
docs/shell/notifications.md:18:dark with its backdrop, as Apple's banners do (see [materials.md](../design-system/materials.md)). Sliding in, sliding out and swiping
docs/shell/notifications.md:23:Toasts and the center share one card, laid out like an iOS banner in three columns:
docs/shell/notifications.md:78:- **Several** form an iOS **stack**: the newest card in front, and up to
docs/shell/osd.md:3:The capsule that shows a level or a switch as it changes (`src/surfaces/osd/`), as macOS and
docs/shell/osd.md:53:(see [Displays](displays.md#hdr)). Raising the volume unmutes, as on macOS. The keys
docs/shell/passwords.md:3:Bitwarden logins, filled into password fields in any app, the way macOS's Passwords fills them:
docs/shell/passwords.md:27:- **Search vault…** opens Spotlight's Passwords mode.
docs/shell/passwords.md:42:**Spotlight.** Type `pw ` and a name in Spotlight, or open the Passwords mode
docs/shell/passwords.md:45:two-factor code** and **Copy password**. A Spotlight fill goes through the popup's checks
docs/shell/passwords.md:51:password**. A copy is made while Spotlight still has the keyboard, as the compositor only takes
docs/shell/passwords.md:64:| Passwords in Spotlight | on | |
docs/shell/passwords.md:84:| X11 apps (xwayland-satellite) | no input method: Spotlight copies instead |
docs/shell/passwords.md:228:  that one, and fill in apps is off (Settings says so). Spotlight's passwords still work.
docs/shell/passwords.md:242:- **X11 apps** get no popup and no fill, only Spotlight's copy.
docs/shell/phone.md:207:Spotlight finds the phone's apps by name ("On Galaxy S24"; Shell Settings →
docs/shell/phone.md:208:Spotlight → Phone apps). Each one opens alone in a window of its own: a virtual
docs/shell/quick-settings.md:3:GNOME's quick settings menu, drawn as the macOS Control Center
docs/shell/quick-settings.md:4:(`src/surfaces/quick_settings/`). As on macOS Tahoe the card itself has no background: it's a
docs/shell/quick-settings.md:10:It's opened from the bar's Control Center button (the slider glyph, between the battery and the
docs/shell/quick-settings.md:16:Control Center button over such a page brings the whole card. Each of those pages has a **Show in
docs/shell/quick-settings.md:26:In Control Center's order (the default layout; edit mode changes it). Every control shows only
docs/shell/quick-settings.md:46:A control works like a macOS Control Center module. A click on the round disc switches the
docs/shell/quick-settings.md:51:shell's alert (macOS's "Are you sure you want to restart your computer now?"; see
docs/shell/quick-settings.md:59:Now Playing is always there, saying "Not Playing" when nothing is, as on macOS; everything else
docs/shell/quick-settings.md:88:space and `quick-settings edit` over IPC. As on macOS Tahoe, editing opens the **Customize
docs/shell/quick-settings.md:130:  module is dragged out of Control Center, or click its badge. Dragged off, the others close up
docs/shell/quick-settings.md:137:  Control Center's order (as `quick-settings reset-layout` does).
docs/shell/quick-settings.md:144:The grid is iOS Control Center's canvas: `quick.grid.cols` × `quick.grid.rows` (4 × 8) cells of
docs/shell/quick-settings.md:182:the canvas. The default packs Control Center's order onto the canvas for the controls this machine
docs/shell/quick-settings.md:291:- **`tile.rs` (`QuickToggle`, `module`)**: GNOME's QuickToggle in Control Center's shapes
docs/shell/quick-settings.md:323:  for a card with a page. The track's fill is the panel's `fg` (white in dark mode) as on macOS,
docs/shell/quick-settings.md:324:  a documented departure from the one-accent rule (§11.2). (Control Center's microphone slider
docs/shell/quick-settings.md:374:(`src/surfaces/alert/mod.rs`): macOS's alert in the middle of the screen, a kit
docs/shell/settings-app.md:10:page's name, like `displays`) opens that page, cold or in the running copy (`App::on_activate`): Spotlight's
docs/shell/settings-app.md:22:Settings' **Customize…**). The settings are macOS-style grouped rows
docs/shell/settings-app.md:75:| Accent color | dots, the hovered one named | System (multicolor), Blue, Purple, Pink, Red, Orange, Yellow, Green, Graphite (macOS 27's colors) | `accent`: `null` or `#rrggbb` |
docs/shell/settings-app.md:83:An accent saved before macOS 27's colours shows as its new counterpart; picking it again saves
docs/shell/settings-app.md:145:### Spotlight
docs/shell/settings-app.md:191:  - the toggles: fill in apps, username fields, Spotlight, and hiding while sharing;
docs/shell/status-bar.md:5:monitor, as macOS has a menu bar on every display (`App::window_per_monitor`): each shows the
docs/shell/status-bar.md:10:cluster on the right. The bar is one uniform frosted strip, like macOS 27's toolbar: its layer
docs/shell/status-bar.md:86:**Dragging** (`drag.rs`), as macOS moves menu bar items with ⌘-drag: the bar never has the
docs/shell/status-bar.md:95:2. **Privacy** (`chips.rs`), in macOS's colours: a green camera while a camera runs; an orange
docs/shell/status-bar.md:129:The first item, **Apps** (`tray.rs`), is macOS's menu bar extras: the system tray's icons from
docs/shell/status-bar.md:141:**Allow in the Status Bar**, as macOS's: Shell Settings lists every app seen in the tray
docs/shell/status-bar.md:145:**Overflow**, as macOS 27's: when the right-hand items would be wider than the bar leaves them (its
docs/shell/status-bar.md:154:is macOS's status items (`NSStatusItem`), as amane's `StatusItem` client or plain JSON over
docs/shell/status-bar.md:178:What's going on now, as macOS 27.3 puts in the menu bar (`activities.rs`).
docs/shell/status-bar.md:187:  - **Timer**: the launcher's timer counting down, in orange, with Cancel and Open in Spotlight.
docs/shell/status-bar.md:244:| `style` | `frosted`: the layer rule's frost and `Material::Bar`'s tint. `glass`: one blur shape in `Material::Control`'s glass over the strip. `solid`: no blur, black in dark mode and white…
docs/shell/status-bar.md:249:| `flash_separators` | the colons blink each second, as macOS's; a figure space stands in, and the clock keeps the width it has with colons. `Clock` signals each second while on |
docs/shell/status-bar.md:258:| `auto_hide` | `never`, `always`, `desktop` (while the bar's monitor shows a workspace with no windows) or `fullscreen` (while a window on that workspace is fullscreen: `WindowInfo::fullscre…
docs/shell/widgets.md:3:macOS's desktop widgets on the wallpaper (`src/surfaces/widgets/`). Quick Settings' tiles are
docs/shell/widgets.md:39:As on macOS, the cards fade back while a window is open on the monitor's workspace, and come up
docs/shell/widgets.md:75:on a 1920 × 1080 screen that's 20 × 11. The sizes are macOS's on that grid: Small 2 × 2 (164 ×
docs/shell/widgets.md:76:164, near macOS's 170 pt), Medium 4 × 2, Large 4 × 4 and Extra Large 8 × 4.
examples/liquid_glass.rs:1://! Liquid glass — a look test, not shell code.
flake.nix:80:      # Cactus 4.0.0 for amane-assistant (Spotlight's assistant): the prebuilt
kit/examples/lists.rs:38:use amane_kit::macform::theme as mac;
kit/examples/lists.rs:154:                .style(mac::page_title_font())
kit/examples/lists.rs:155:                .color(mac::label_primary()),
kit/examples/lists.rs:672:                .trailing(amane_kit::macform::MacSwitch::new("form.tr.sw.c", true).build()),
kit/examples/lists.rs:838:                    amane_kit::macform::MacSwitch::new(format!("form.sel.{i}.c"), on)
kit/examples/macform.rs:1://! The macform controls next to the kit's own, to compare and try:
kit/examples/macform.rs:2://! `cargo run -p amane-kit --example macform`.
kit/examples/macform.rs:5://! the macform port on the right, sharing the same state, with what it
kit/examples/macform.rs:23:use amane_kit::macform::theme::{self as mac, Accent};
kit/examples/macform.rs:24:use amane_kit::macform::{
kit/examples/macform.rs:54:            // MACFORM_PAGE=5 opens on a page, for screenshots
kit/examples/macform.rs:55:            page: std::env::var("MACFORM_PAGE")
kit/examples/macform.rs:91:    app("amane-macform-gallery")
kit/examples/macform.rs:93:        .normal_window("macform", window)
kit/examples/macform.rs:109:    let nav = sidebar("macform.nav", &pages, state.page, |page| {
kit/examples/macform.rs:125:            .style(mac::page_title_font())
kit/examples/macform.rs:126:            .color(mac::label_primary()),
kit/examples/macform.rs:136:            "macform.page",
kit/examples/macform.rs:147:        .title("macform gallery")
kit/examples/macform.rs:179:        cell(CELL_W, 20.0, caption("MACFORM")),
kit/examples/macform.rs:229:            Radio::new(format!("mac.size.{i}"), s.size == i)
kit/examples/macform.rs:255:                    MacSwitch::new("mac.wifi", s.wifi)
kit/examples/macform.rs:260:                    MacSwitch::new("mac.bt", s.bluetooth)
kit/examples/macform.rs:264:                boxed(MacSwitch::new("mac.off", true).disabled(true).build()),
kit/examples/macform.rs:290:                    Checkbox::new("mac.sounds", s.sounds)
kit/examples/macform.rs:296:                    Checkbox::new("mac.badges", s.badges)
kit/examples/macform.rs:342:            caption("(no macform counterpart)"),
kit/examples/macform.rs:381:    .map(|sym| boxed(symbol(sym, 16.0, mac::label_primary())))
kit/examples/macform.rs:412:                    MacPushButton::new("mac.ok", "OK")
kit/examples/macform.rs:414:                        .on_press(press("macform OK"))
kit/examples/macform.rs:418:                    MacPushButton::new("mac.cancel", "Cancel")
kit/examples/macform.rs:419:                        .on_press(press("macform Cancel"))
kit/examples/macform.rs:423:                    MacPushButton::new("mac.search", "Search")
kit/examples/macform.rs:425:                        .on_press(press("macform Search"))
kit/examples/macform.rs:445:                    MacPushButton::new("mac.dis", "Apply")
kit/examples/macform.rs:450:                    MacPushButton::new("mac.dis2", "Apply")
kit/examples/macform.rs:464:            MacSegmentedNav::new("mac.nav")
kit/examples/macform.rs:477:                "position {} of 3 (segmented button on the kit side, back/forward pill on macform)",
kit/examples/macform.rs:518:            Dropdown::items("mac.accent", accents, Some(s.accent))
kit/examples/macform.rs:530:            Dropdown::items("mac.sort", sorts.clone(), Some(s.sort))
kit/examples/macform.rs:546:                        .style(mac::control_font())
kit/examples/macform.rs:547:                        .color(mac::label_primary()),
kit/examples/macform.rs:550:                    MacPopUpButton::new("mac.popup")
kit/examples/macform.rs:562:            MacComboBox::new("mac.font", fonts)
kit/examples/macform.rs:585:            MacTextField::new("mac.name")
kit/examples/macform.rs:596:            MacTextField::new("mac.port")
kit/examples/macform.rs:611:            MacTextField::new("mac.pw")
kit/examples/macform.rs:624:                    MacStepper::new("mac.port.step", s.port)
kit/examples/macform.rs:631:                    MacStepper::new("mac.scale.step", s.scale)
kit/examples/macform.rs:637:                boxed(MacStepper::new("mac.dis.step", 3.0).disabled(true).build()),
kit/examples/macform.rs:647:/// macOS Settings-style rows, from the kit's `form`.
kit/src/animation/edge_reveal.rs:2://! the pointer touches the edge, as the macOS dock does with hiding on.
kit/src/animation/kinetic.rs:23:/// this before it stops, as iOS's "normal" deceleration does.
kit/src/animation/motion.rs:29:// iOS-like motion. TODO(token): these become `sys.motion.*` tokens once
kit/src/containers/app_bar.rs:5://! leads with M3's back button too, a chevron as on macOS. Unlike M3's it's
kit/src/containers/arrangement.rs:2://! place: macOS's Arrange view in the Displays settings.
kit/src/containers/card.rs:5://! vibrant labels and fills ([`material::label`]). Drawn in the shell's macOS look: `sys.radius.popover` corners,
kit/src/containers/dialog.rs:1://! A question answered with one of its buttons, in macOS 26's alert: a
kit/src/containers/dialog.rs:18://! fit, and stack otherwise, as macOS's do; [`Dialog::stacked`] and
kit/src/containers/form.rs:1://! Form — settings laid out as macOS's System Settings lays them out:
kit/src/containers/form.rs:18://! Sizes are the macform look's (`macform::theme`): rows 42 tall (52 with a
kit/src/containers/form.rs:28:use crate::macform::symbol::{Symbol, symbol};
kit/src/containers/form.rs:29:use crate::macform::theme::{self, px};
kit/src/containers/form.rs:30:use crate::macform::{MacLeadingIcon, hover_fill, on_press};
kit/src/containers/form.rs:122:    /// Set in under the row before it, as macOS sets a setting that only
kit/src/containers/form.rs:390:                crate::macform::interactive(base, self.id, self.disabled, on_press(move || press()))
kit/src/containers/form.rs:398:/// A title and its value over a slider that spans the row, as macOS sets
kit/src/containers/list.rs:23:use crate::macform::MacLeadingIcon;
kit/src/containers/list.rs:108:    /// The leading glyph on a tile of one of the named accents, as macOS's
kit/src/containers/lists/item.rs:21:use crate::macform::MacLeadingIcon;
kit/src/containers/tooltip.rs:7://! and centred above the pointer, as macOS's are.
kit/src/controls/buttons/checkbox.rs:3://! Drawn like macOS's (the macform look): a 16 px box, radius 5.5,
kit/src/controls/buttons/checkbox.rs:16:use crate::macform::symbol::{Symbol, symbol};
kit/src/controls/buttons/checkbox.rs:17:use crate::macform::theme::{self, px};
kit/src/controls/buttons/checkbox.rs:18:use crate::macform::{crossfade, labelled};
kit/src/controls/buttons/mod.rs:3://! shapes, state layers) but is drawn in the shell's macOS-style look:
kit/src/controls/buttons/mod.rs:7://! - [`Switch`] — an iOS-style track and knob for an on/off setting (M3's
kit/src/controls/buttons/mod.rs:82:    /// can't be undone ("Delete", "Log Out"), as in macOS's alerts.
kit/src/controls/buttons/radio.rs:3://! Drawn like macOS's (the macform look): a 16 px circle, `fieldFill` in a
kit/src/controls/buttons/radio.rs:18:use crate::macform::theme::{self, px};
kit/src/controls/buttons/radio.rs:19:use crate::macform::{crossfade, labelled};
kit/src/controls/buttons/segmented.rs:9://! It moves like iOS's segmented control: in single-select one thumb springs
kit/src/controls/buttons/switch.rs:4://! layer, a disabled state), but drawn like iOS's: a capsule track that
kit/src/controls/control_size.rs:1://! How big the kit's standard controls are: the five sizes of the macOS 27
kit/src/controls/control_size.rs:77:    /// A button's corners: capsules from Large up, as in macOS 26.
kit/src/controls/dropdown.rs:3://! Drawn like macOS's pop-up button (the macform look): a control-fill
kit/src/controls/dropdown.rs:8://! it (see [`macform::menu`](crate::macform)).
kit/src/controls/dropdown.rs:20:use crate::macform::menu::{self, MenuItem};
kit/src/controls/dropdown.rs:21:use crate::macform::symbol::{Symbol, symbol};
kit/src/controls/dropdown.rs:22:use crate::macform::theme::{self, px};
kit/src/controls/dropdown.rs:23:use crate::macform::{hover_fill, on_press};
kit/src/controls/macform/color_wells.rs:1://! Color wells — the accent colors as a row of dots, as macOS's
kit/src/controls/macform/menu.rs:321:        return Popup::new("amane.macform.menu")
kit/src/controls/macform/menu.rs:382:    let popup = Popup::new("amane.macform.menu")
kit/src/controls/macform/mod.rs:1://! macform — macOS-style form controls, ported from the Quickshell
kit/src/controls/macform/mod.rs:2://! `components/macform` set for a side-by-side look against the kit's own.
kit/src/controls/macform/mod.rs:5://! amane-kit --example macform` shows each one next to its kit counterpart.
kit/src/controls/macform/mod.rs:62:    let amount = fade(&format!("{id}.mac-hover"), on, motion::HOVER);
kit/src/controls/macform/symbol.rs:1://! SFSymbol: the handful of SF Symbols the macform controls draw, as
kit/src/controls/macform/symbol.rs:12:/// The symbols the controls use, named as in SF Symbols.
kit/src/controls/macform/theme.rs:1://! MacTheme: the colours and sizes the macform controls share.
kit/src/controls/macform/thumbnail_picker.rs:2://! like, as macOS's Appearance setting offers Light, Dark and Auto: the
kit/src/controls/mod.rs:8:pub mod macform;
kit/src/controls/slider.rs:5://! discrete [`STEP`]s for the scroll wheel. Drawn in the macOS look:
kit/src/controls/slider.rs:9://! [`Slider::large`] is the macOS Control Center slider instead: a track a
kit/src/controls/slider.rs:101:    /// The Control Center look: a control-tall track with `glyph` as its knob.
kit/src/controls/spinner.rs:1://! Spinner and progress bar — something under way, as macOS shows it: an
kit/src/controls/spinner.rs:16:use crate::macform::theme;
kit/src/foundation/color.rs:74:    /// The colour, as Apple's dark-mode system colours.
kit/src/foundation/icons.rs:117:/// macOS shape; the fill shows the charge.
kit/src/foundation/icons.rs:280:        "linux" | "windows" | "macos" | "freebsd" | "openbsd" => ICON_COMPUTER,
kit/src/foundation/icons.rs:281:        "ios" | "android" => ICON_PHONE,
kit/src/lib.rs:18://! - [`macform`] — experimental macOS-style controls ported from Quickshell,
kit/src/lib.rs:19://!   shown against the kit's own by the `macform` example.
kit/src/lib.rs:32://! - [`form`] — settings as macOS lays them out: rounded groups of rows,
kit/src/lib.rs:38://! - [`material`] — Apple's materials: glass by role, its thickness and
kit/src/lib.rs:83:    avatar, buttons, color_picker, control_size, dropdown, macform, slider, spinner, tag,
kit/src/look/glass_region.rs:22:/// configured: niri's liquid glass, which every config with materials has.
kit/src/look/glass_region.rs:29:/// solid glass, nor where the system asks for higher contrast (Apple's
kit/src/look/glass_region.rs:79:/// A panel of glass keeps the theme's tint (Apple: "bigger elements …
kit/src/look/glass_region.rs:243:/// as Apple's Control Center does; `members` includes the piece being
kit/src/look/material.rs:1://! Materials, as Apple's HIG has them: a piece of glass asks for the
kit/src/look/material.rs:9://! Two families, after Liquid Glass and Apple's standard materials:
kit/src/look/material.rs:19://! every material, as Apple's are; the contrast floors check each against
kit/src/look/material.rs:33:/// look changes with the settings (Apple: "choose materials by semantic
kit/src/look/material.rs:70:    /// Liquid Glass: controls and navigation, lensed.
kit/src/look/material.rs:76:/// Liquid Glass's variants.
kit/src/look/material.rs:112:    /// Turns light or dark with its backdrop, as Apple's small glass does.
kit/src/look/material.rs:158:/// has the focus: calmer, as AppKit's inactive windows are.
kit/src/look/material.rs:163:/// frosts with the small glass material, as Apple's small glass frosts less.
kit/src/look/material.rs:168:/// bright. Over a bright one it keeps all of it: Apple's 35% dimming layer
kit/src/look/material.rs:252:    /// liquid glass, which every config with materials has.
kit/src/look/material.rs:259:            // thick glass frosts more, as Apple's sheets and sidebars do (30 pt)
kit/src/look/material.rs:276:    /// backdrop less than large glass (the macOS 27 kit: 6 against 16), or
kit/src/look/material.rs:326:                    // Apple's sidebar recipe, drawn by the window over niri's frost
kit/src/look/material.rs:524:/// A label, under higher contrast or not: Apple's increased contrast keeps
kit/src/look/material.rs:719:        // set starts higher (the macOS 27 kit: 90 % against 85 % in light)
kit/src/platform/app.rs:30:use crate::macform::theme as mac;
kit/src/platform/app.rs:31:use crate::macform::{MacLeadingIcon, text_field};
kit/src/platform/app.rs:44:/// (`sys.color.window.bg`), as macOS draws a window's content. Only the
kit/src/platform/app.rs:99:        .size(mac::control_font().size)
kit/src/platform/app.rs:100:        .color(mac::label_primary())
kit/src/platform/app.rs:101:        .selection_color(with_alpha(mac::tint(), 0.4))
kit/src/platform/app.rs:105:/// the keys, as macOS keeps what was typed when you click away. Call it
kit/src/platform/app.rs:127:/// The box a [`text_field`] sits in, drawn like macOS's (the macform
kit/src/platform/app.rs:141:    /// The tile its glyph sits on, as macOS's System Settings draws its
kit/src/platform/app.rs:162:    /// A section's title, as macOS's sidebar groups its pages. It takes an
kit/src/platform/app.rs:343:    // window, as a macOS sidebar is the one behind-window part
kit/src/platform/settings.rs:18:/// The accents to pick from, macOS 27's system colours in their dark
kit/src/platform/settings.rs:115:    /// macOS dims them.
kit/src/platform/settings.rs:311:    pub show_in_spotlight: bool,
kit/src/platform/settings.rs:331:            show_in_spotlight: true,
kit/src/platform/settings.rs:446:    /// No strip at all, as macOS 26's menu bar: the items sit on the
kit/src/platform/settings.rs:451:/// When the status bar hides past its edge, as macOS's "Automatically hide
kit/src/platform/settings.rs:606:    /// as macOS's "Allow in the Menu Bar" leaves them out.
kit/src/platform/settings.rs:609:    /// Now Playing shows with nothing playing too, as macOS's "Always
kit/src/platform/settings.rs:621:    /// The colons blink each second, as macOS's "Flash the time separators".
kit/src/platform/settings.rs:915:    /// after the tray, as macOS adds a menu bar extra at the left.
kit/src/platform/settings.rs:1289:/// with them, as Dropzone's actions.
kit/src/theme.rs:29:/// How see-through the glass is, as Apple's Liquid Glass setting has it.
kit/src/tokens/color.rs:246:    ("aliceblue", 0xf0f8ff), ("antiquewhite", 0xfaebd7), ("aqua", 0x00ffff),
kit/src/tokens/font.rs:105:        let s = stack(&["SF Pro Text", "Inter", "sans-serif"]);
kit/src/tokens/font.rs:107:        assert_eq!(pick(&s, |_| true), "SF Pro Text");
kit/src/tokens/rules.rs:25:/// may carry the contrast: Apple's glyphs "flip from light to dark,
kit/src/tokens/values.rs:591:            (FontFamilies, json!(["SF Pro Text", "Inter"])),
niri-layer-rule.kdl:20:// Status bar: one uniform frosted strip along the bottom, macOS 27's toolbar:
src/components/grid/drag.rs:7://! where nothing fits keeps the last preview that did, as iOS stays where
src/components/panel.rs:33:/// the last one opened, as macOS opens a menu bar's panels on its display.
src/components/search_field.rs:7://! Drawn in the shell's macOS look: a `surface.raised` capsule a control
src/main.rs:119:            MenuItem::label("AirDrop")
src/main.rs:121:                .on_select(|| eprintln!("airdrop")),
src/services/assistant.rs:1://! Spotlight's assistant, the shell's end: reading a query as a command,
src/services/assistant.rs:94:    // a word alone, a bare name or a question isn't a command: Spotlight's
src/services/desktop/activities.rs:1://! Live Activities other programs put in the bar, as macOS 27.3 has them in
src/services/desktop/activities.rs:22:/// The longest an activity lasts, as on macOS.
src/services/desktop/keyboard_layout.rs:2://! menu, as macOS's. Followed from `niri msg --json event-stream`, which
src/services/desktop/keyboard_layout.rs:93:/// A short code for a layout's name, as macOS shows the input source: the
src/services/desktop/weather.rs:1://! Today's weather for a place, as macOS's menu bar Weather: from
src/services/status_apps.rs:2://! bar (macOS's status items) and their Live Activities. Each payload is
src/services/vault/deliver.rs:45:            return tell("Nothing was filled", "No window was behind Spotlight.");
src/services/vault/matching.rs:108:            | "flathub" | "mozilla" | "nixos" | "sr" | "srht" | "launchpad" | "googlecode"
src/surfaces/alert/mod.rs:2://! surface of its own in thick glass, as macOS asks before it ends the
src/surfaces/context_menu/mod.rs:412:/// selected (accent, like macOS); a pointer-focused one only hovered.
src/surfaces/dock/drag.rs:4://! while it's held, as on macOS. A pinned app's icon dragged up off the
src/surfaces/dock/magnify.rs:1://! The macOS magnification, as plain layout: how big each icon of a row is
src/surfaces/dock/mod.rs:1://! Dock — the macOS Big Sur dock, for the windows open on every workspace.
src/surfaces/dock/mod.rs:14://! icons swell under the pointer as on macOS ([`magnify`]), with the name
src/surfaces/dock/motion.rs:102:/// The macOS magnification: the icon under the pointer and its neighbours
src/surfaces/dock/pins.rs:2://! there to open whether or not it's running, as macOS keeps its pinned
src/surfaces/fill.rs:27:use amane_kit::macform::theme::{self, px};
src/surfaces/launcher/actions.rs:1://! The launcher's actions, Spotlight's Actions: things it does without
src/surfaces/launcher/actions.rs:49:    /// Where it's listed: "Control Center", "System", an app's name.
src/surfaces/launcher/actions.rs:127:const CONTROL_CENTER: &str = "Control Center";
src/surfaces/launcher/assistant.rs:1://! The assistant in Spotlight: a command read from the query
src/surfaces/launcher/assistant.rs:4://! ordinary action: the Control Center switch, the volume key (with its
src/surfaces/launcher/assistant.rs:39:/// What the machine has now, for a command to name: Control Center's
src/surfaces/launcher/header.rs:181:/// button: a chevron on a soft square under the pointer, as Spotlight's.
src/surfaces/launcher/mod.rs:1://! Launcher — macOS Tahoe's Spotlight: a glass search pill high on the
src/surfaces/launcher/mod.rs:9://! Applications is the Launchpad grid it grew from: a row of category
src/surfaces/launcher/mod.rs:436:                // Files offers its kinds as chips, as Spotlight's filter bubbles
src/surfaces/launcher/mod.rs:711:/// Opens Spotlight on a fresh search and starts listening, Whistle told
src/surfaces/launcher/mod.rs:723:/// Spotlight searches it as if typed. What a rule reads that doesn't end
src/surfaces/launcher/mode.rs:37:            Self::Home => "Spotlight Search",
src/surfaces/launcher/phone_apps.rs:1://! The phone's apps in Spotlight: found by name, each opens in a window of
src/surfaces/launcher/provider.rs:573:/// kind's name, as Spotlight's.
src/surfaces/launcher/provider.rs:684:            Mode::Home if amane_kit::settings::passwords().show_in_spotlight => {
src/surfaces/launcher/results.rs:113:/// A section's name over its rows, a hairline under it, as Spotlight's.
src/surfaces/launcher/results.rs:755:/// What Clipboard says before its history is turned on, as Spotlight does.
src/surfaces/launcher/results.rs:756:const ASK_NOTE: &str = "Spotlight can keep what you copy, to paste it again from here. \
src/surfaces/notifications/card.rs:3://! Laid out like an iOS/macOS banner, in three columns: the app icon,
src/surfaces/notifications/card.rs:604:/// When it arrived, macOS-style: "now", "5m ago", "2h ago", then the day.
src/surfaces/notifications/card.rs:652:    fn times_read_like_macos() {
src/surfaces/notifications/center.rs:6://! several is an iOS stack: the newest card in front, up to
src/surfaces/notifications/center.rs:157:    // place and clicked into, like macOS's Notification Center
src/surfaces/notifications/mod.rs:1://! Notifications, macOS/iOS-style: toasts that slide in at the top-right,
src/surfaces/osd/keys.rs:106:/// Moves the volume by `by`, unmuting as it goes up (as macOS does).
src/surfaces/osd/mod.rs:4://! mode, the sound moving to another device), then fades out, as macOS and
src/surfaces/quick_settings/controls.rs:53:    /// Control Center's order, then the ones added since (a control's
src/surfaces/quick_settings/customize/catalog.rs:2://! the categories a sidebar names (macOS's Customize Control Center names
src/surfaces/quick_settings/customize/catalog.rs:100:    /// The controls under it, in Control Center's order.
src/surfaces/quick_settings/customize/catalog.rs:217:        // in Control Center's order, not the order given
src/surfaces/quick_settings/customize/mod.rs:1://! The Customize window — macOS's Customize Control Center for Quick
src/surfaces/quick_settings/detail.rs:1://! Detail page — what a tile opens: GNOME's submenu, drawn as a macOS
src/surfaces/quick_settings/detail.rs:2://! Control Center page that slides over the grid.
src/surfaces/quick_settings/edit.rs:1://! Edit mode — iOS's "Edit Controls": the grid's controls go inert and can
src/surfaces/quick_settings/edit.rs:18://! The others make way live, as on iOS, each preview worked out from the
src/surfaces/quick_settings/edit.rs:255:/// as a module dragged out of Control Center is, and one from the
src/surfaces/quick_settings/edit.rs:333:/// Control Center's order again, for the controls this machine has.
src/surfaces/quick_settings/edit.rs:442:/// The faint discs the free cells show in edit mode, as iOS draws the
src/surfaces/quick_settings/grid.rs:1://! The grid — iOS Control Center's canvas: `cols` columns by `rows` rows of
src/surfaces/quick_settings/layout.rs:4://! it's Control Center's order packed onto the canvas. A control the file
src/surfaces/quick_settings/layout.rs:58:    /// Control Center's order packed onto `canvas`, each control at its
src/surfaces/quick_settings/mod.rs:1://! Quick Settings — GNOME's quick settings menu in the macOS Control Center
src/surfaces/quick_settings/mod.rs:2://! look: one frosted card above the bar holding Control Center's modules,
src/surfaces/quick_settings/mod.rs:5://! The card is iOS Control Center's canvas, four cells across and eight
src/surfaces/quick_settings/mod.rs:6://! down (`grid`), laid out as iOS lays it out by default: Wi‑Fi and
src/surfaces/quick_settings/mod.rs:12://! to fill the card, as on iOS. Escape goes back to the grid, then closes.
src/surfaces/quick_settings/mod.rs:14://! As on macOS Tahoe, the card itself has no background: each module is a
src/surfaces/quick_settings/mod.rs:29://! bar's Control Center button, a status icon, or over IPC:
src/surfaces/quick_settings/mod.rs:41://! macOS's Customize Control Center does. amane can't carry a drag from one
src/surfaces/quick_settings/mod.rs:556:        // with it, rather than animating out of the tile's (Apple: no
src/surfaces/quick_settings/slider_card.rs:1://! Slider card — Control Center's Display and Sound modules: a glass card
src/surfaces/quick_settings/system_row.rs:1://! The system controls — GNOME's top row, at the bottom as Control Center
src/surfaces/quick_settings/tile.rs:2://! state, in one of macOS Control Center's shapes ([`Style`]), each a
src/surfaces/quick_settings/tile.rs:6://! the Control Center look: the glyph on a round disc that turns accent when
src/surfaces/quick_settings/tile.rs:13://! As on macOS, the disc toggles and the rest of the tile opens the
src/surfaces/quick_settings/tile.rs:41:/// Which of Control Center's shapes a toggle takes.
src/surfaces/quick_settings/tile.rs:131:/// translucent fill on it instead, as Control Center's catalog shows its
src/surfaces/quick_settings/tile.rs:324:        // (`sys.color.state.on`), as Control Center's do
src/surfaces/quick_settings/tiles/appearance.rs:1://! The screen — Control Center's Display card (GNOME's brightness slider,
src/surfaces/quick_settings/tiles/bluetooth.rs:3://! Bluetooth manager like the macOS one: "My Devices" and "Other Devices",
src/surfaces/quick_settings/tiles/media.rs:2://! play/pause and next, like macOS Control Center's module. (GNOME keeps
src/surfaces/quick_settings/tiles/media.rs:3://! its media controls in the notification list instead.) As on macOS the
src/surfaces/quick_settings/tiles/mod.rs:4://! them, in Control Center's order by default.
src/surfaces/quick_settings/tiles/mod.rs:192:    // in edit mode the places a control can go show through, as on iOS.
src/surfaces/quick_settings/tiles/notifications.rs:1://! Focus — GNOME's Do Not Disturb toggle as Control Center's Focus tile
src/surfaces/quick_settings/tiles/session.rs:3://! The ones that end the session ask first in an [`alert`], as macOS does.
src/surfaces/quick_settings/tiles/sound.rs:1://! Sound — Control Center's Sound card: GNOME's output slider (the glyph
src/surfaces/status_bar/activities.rs:1://! Live Activities, as macOS 27.3 puts them in the menu bar: what's going
src/surfaces/status_bar/activities.rs:248:        ActivityButton::new("open", "Open in Spotlight"),
src/surfaces/status_bar/activities.rs:812:/// first, as `key<TAB>app<TAB>title`; ending one is the user's, as macOS
src/surfaces/status_bar/apps.rs:1://! Programs' own items, as macOS's status items: put up over the bar's bus
src/surfaces/status_bar/chips.rs:1://! The privacy dots in macOS's colours (orange microphone, green camera),
src/surfaces/status_bar/chips.rs:16:/// macOS's privacy colours: the microphone and the camera.
src/surfaces/status_bar/drag.rs:1://! Moving the bar's items by dragging them, as macOS moves menu bar items
src/surfaces/status_bar/indicators.rs:206:    /// Settings and the clock always show, as macOS's Control Center and
src/surfaces/status_bar/mod.rs:123:/// The status bar on `monitor`, one on each as macOS has one on each
src/surfaces/status_bar/mod.rs:214:        // (macOS menu-bar behavior); the panels take keys when clicked into.
src/surfaces/status_bar/mod.rs:650:/// Quick Settings' button, the macOS Control Center's place in the bar.
src/surfaces/status_bar/mod.rs:742:/// The clock opens the notification center, like macOS's; its tooltip is
src/surfaces/status_bar/mod.rs:787:/// A small clock face with its two hands, as macOS's analog menu bar clock.
src/surfaces/status_bar/mod.rs:833:/// The keyboard layout as macOS's input menu shows it, a short code, with
src/surfaces/status_bar/tray.rs:1://! Apps' own items, macOS's menu bar extras: the system tray's icons
src/surfaces/status_bar/tray.rs:7://! that don't fit go behind a » button, as macOS 27 has them.
src/surfaces/widgets/catalog/battery.rs:1://! Batteries — macOS's widget: the machine's battery and each device's
src/surfaces/widgets/catalog/mod.rs:5://! places, as macOS's are.
src/surfaces/widgets/catalog/mod.rs:25:/// The room inside a card's edge, as macOS's widgets keep.
src/surfaces/widgets/edit.rs:1://! Edit Widgets — macOS's widget gallery, on one monitor at a time. The
src/surfaces/widgets/gallery.rs:1://! The widget gallery — macOS's: a frosted sheet at the bottom of the
src/surfaces/widgets/kind.rs:21:/// macOS's widget sizes on the desktop's grid: a cell is Quick Settings'
src/surfaces/widgets/kind.rs:22:/// (74 px, 16 px between), so Small is 164 × 164, near macOS's 170 pt.
src/surfaces/widgets/kind.rs:295:    fn sizes_are_macos_on_the_grid() {
src/surfaces/widgets/mod.rs:1://! Desktop widgets — macOS's widgets on the wallpaper, for watching rather
src/surfaces/widgets/mod.rs:14://! As on macOS, a card is dragged to another place at any time, the others
src/surfaces/widgets/mod.rs:77:/// A card's corners: macOS's widgets'.
src/surfaces/widgets/mod.rs:295:/// over the windows, as macOS clears the desktop to edit it; hidden the
src/tokens/layer_rules.rs:95:                "Status bar: one uniform frosted strip along the bottom, macOS 27's toolbar:\n\
tests/fixtures/tailscale/status.json:23:      "OS": "iOS",
```

## amane (18 lines)

```
docs/fork/graphics/blur.md:95:painted over it. These are Apple's vibrant labels on a material.
docs/fork/system/status-items.md:3:A program's own **item in the status bar**, like macOS's `NSStatusItem`, and
docs/fork/system/status-items.md:4:**Live Activities**, as macOS 27.3 puts in the menu bar: something going on now,
docs/fork/system/tray.md:6:to show macOS-style menu bar extras.
protocols/niri-vibrancy-v1.xml:38:    Apple's plus-darker does (D + c - 1) is drawn as white, 1 - c, at
src/graphics/vibrancy.rs:11: * 1 - c at alpha a, which darkens by a(1 - c), Apple's plus-darker
src/graphics/vibrancy.rs:89:// only flat shapes and letters in translucent white or black are vibrant: Apple's
src/layer_window.rs:243:     * niri-vibrancy: Apple's vibrant labels on a material
src/popup.rs:159:     * niri-vibrancy: Apple's vibrant labels on a material
src/status_item.rs:2: * status bar apps: a program's own item in the bar (macOS's NSStatusItem)
src/status_item.rs:3: * and Live Activities (macOS 27.3's), over the bar's session bus service
src/status_item/client.rs:49: * a program's item in the bar, as macOS's NSStatusItem:
src/status_item/client.rs:121: * a Live Activity in the bar, as macOS 27.3's: started, updated as it goes,
src/status_item/protocol.rs:113: * a program's item in the bar, as macOS's status items: its id is unique on
src/status_item/protocol.rs:287: * a Live Activity, as macOS 27.3 puts in the menu bar: something going on
src/widgets/scroll_area/offsets.rs:14:// how quickly a fling's momentum dies out: it travels its speed times this, as iOS's does
src/widgets/text/style.rs:152:        assert_eq!(Case::Capitalize.apply("iOS stays"), "IOS Stays");
src/window.rs:153:     * niri-vibrancy: Apple's vibrant labels on a material
```

## niri (25 lines)

```
docs/fork/README.md:31:- **Effects:** liquid glass. [Materials](effects/materials.md) are the looks,
docs/fork/effects/blur-shapes.md:10:niri draws each with antialiased corners at its opacity, so a client with many rounded pieces of glass (such as a control center) gets blur exactly in their shape that fades with them.…
docs/fork/effects/blur-shapes.md:24:control centre, cards that swell open) gets square, all-or-nothing blur. This
docs/fork/effects/materials.md:71:With `blur-passes` or `blur-offset` set, the backdrop behind that material's shapes is blurred with them instead of `blur {}`'s, so big glass can frost more than small glass, as Apple's …
docs/fork/effects/materials.md:73:The lens bends the backdrop in from past the shape's edge, as Apple's glass does, so a rim continues what lies beside the shape; the blurred capture behind a surface's shapes reaches `re…
docs/fork/effects/materials.md:75:`tone` below 1 squeezes the backdrop's brightness toward the tint's side before the tint is laid on, keeping its hue: under a dark tint the backdrop keeps 0 to `tone` of its brightness, …
docs/fork/effects/materials.md:87:- `material`: the name of a material to draw the effect with, as liquid glass in the shape of the surface's geometry and `geometry-corner-radius`.
docs/fork/effects/materials.md:119:- The lip bends the backdrop in from past the edge, as the recovered macOS 26
docs/fork/effects/materials.md:124:- A rim hairline lit from `light-angle`, with a far-edge catch (`far-rim`, Apple's
docs/fork/effects/materials.md:128:  tint's side (dark tint: `c·tone`; light tint: `1 − (1 − c)·tone`), as Apple's
docs/fork/effects/vibrancy.md:27:Apple's labels on a material are *vibrant*: they're blended with the frosted backdrop under them
docs/fork/effects/vibrancy.md:56:  Apple's plus-darker, `D + c − 1`. Like plus-lighter it needs only blend state, so it works
docs/fork/effects/vibrancy.md:83:- `ada326f0` makes plus-darker subtract (`FUNC_REVERSE_SUBTRACT`), as Apple's does.
docs/fork/layout/workspace-mat.md:8:wherever there are no windows. The mat is a solid colour by default, or optionally liquid glass.
docs/fork/layout/workspace-mat.md:44:- `material`: draw the mat as liquid glass with this named [material](../effects/materials.md) from `materials {}`. The glass is drawn from the blurred wallpaper, using `blur {}`'s se…
docs/fork/maintaining.md:134:| `ada326f0` | [vibrancy](effects/vibrancy.md) | plus-darker subtracts, as Apple's does |
docs/fork/maintaining.md:138:| `78620a8c` | [materials](effects/materials.md) | `tone`, Apple's luminosity adjustment |
niri-config/src/appearance.rs:1112:/// Named looks for blur shapes (`niri-blur-shapes-v1`): liquid glass drawn by niri over the
niri-config/src/appearance.rs:1134:    /// unless set; 0 leaves only the rim's hairline, as Apple's glass has.
niri-config/src/appearance.rs:1154:    /// all; lower keeps a thin tint readable over any wallpaper, as Apple's materials do.
niri-config/src/appearance.rs:1173:    /// a wider blur. Big glass frosts more than small, as Apple's does.
resources/niri-vibrancy-v1.xml:38:    Apple's plus-darker does (D + c - 1) is drawn as white, 1 - c, at
src/render_helpers/shaders/glass.frag:9:// Liquid glass for blur shapes: the blurred background bent at a curved
src/render_helpers/shaders/glass.frag:91:    // At the lip the glass bends what's beside it in over the edge, as Apple's
src/render_helpers/shaders/glass.frag:118:        // its hue kept, as Apple's darken and lighten layers do.
```

## File and folder names (shell)

- apps/shader-lab/effects/liquid-glass/effect.json
- apps/shader-lab/effects/liquid-glass/liquid.wgsl
- docs/archive/spec-macos27-variables.md
- examples/liquid_glass.rs
- kit/examples/macform.rs
- kit/src/controls/macform/color_wells.rs
- kit/src/controls/macform/combo_box.rs
- kit/src/controls/macform/leading_icon.rs
- kit/src/controls/macform/menu.rs
- kit/src/controls/macform/mod.rs
- kit/src/controls/macform/pop_up_button.rs
- kit/src/controls/macform/push_button.rs
- kit/src/controls/macform/segmented_nav.rs
- kit/src/controls/macform/slider.rs
- kit/src/controls/macform/stepper.rs
- kit/src/controls/macform/switch.rs
- kit/src/controls/macform/symbol.rs
- kit/src/controls/macform/text_field.rs
- kit/src/controls/macform/theme.rs
- kit/src/controls/macform/thumbnail_picker.rs

## nixos-config (outside the three repos)

```
home/home.nix:49:  # macOS-style app icons, each named after an app's Icon= key. ~/.local/share
home/home.nix:181:  # --- Drop: amane-drop (a Dropzone-style status bar app) ---
home/home.nix:200:  # --- Assistant: amane-assistant (Spotlight's commands, typed or spoken) ---
niri/config.kdl:170:// Status bar: one uniform frosted strip along the bottom, macOS 27's toolbar:
niri/config.kdl:651:    // Spotlight's assistant: press, say a command, press again (niri has no
```
