# What's not drawn in Penpot yet

What the Penpot file ("New File 5") doesn't have yet, by surface. For tokens still to recreate or
delete, see [penpot-backlog.md](penpot-backlog.md).

Already drawn: the dock, the status bar, Quick Settings (the grid and its states), Emaki
Settings' Appearance page, the launcher, the OSD, notifications, the desktop widgets and Messages
(inbox, thread, New Message, the empty and no-phone states, the message menu).

## Shell surfaces

### Quick Settings (`apps/shell/src/surfaces/quick_settings/`)
- [ ] Detail pages: Sound, Wi‑Fi, VPN, Bluetooth, Power Mode, Battery, Night Light, Do Not
      Disturb, Keyboard, Session (the power menu), Background Apps, Tailscale, Keep Awake, Phone
- [ ] Edit mode and the customize catalog (`customize/`): dragging a control, the outlines, the
      catalog of controls to add
- [ ] The slider card's other states, if any beyond what "Quick Settings — States" shows

### Dock (`apps/shell/src/surfaces/dock/`)
- [x] Window and app menus (`window_menu.rs`): focus, fullscreen, float, move to workspace, pin,
      close; a pinned app's open, open on another workspace, unpin: `Dock / Window menu` (Kind =
      Window, Pinned window, Submenu open, Pin), `Bar / Workspace submenu` (Kind = All), and the "Dock — Window
      menu", "Dock — Workspace submenu" and "Dock — Pinned app menu" boards. Not drawn: "Pin to
      Dock" greyed out for an app with no desktop entry
- [x] Drag states: an icon in hand, the gap opening, dropping off the dock: the "Dock — Dragging"
      and "Dock — Unpinning" boards (the dock is detached in them). Not drawn: files held over the
      dock, with the icon that would open them lit behind

### Status bar (`apps/shell/src/chrome/`)
- [x] The workspace menu (`workspace_menu.rs`): it's the workspace submenu the dock and the
      launcher open, not a menu of the bar's own. Drawn as `Bar / Workspace submenu`: Kind = All
      (`workspace_menu::all`, the dock's) and Focused monitor (`on_focused_output`, the launcher's,
      ending in "New Workspace")
- [x] Activities in the bar (Drop's work, train-watch's run): `Bar / Activity` (Kind = Ring,
      Countdown, Text, Minimal × State = Rest, Active), `Bar / Activity card` (Drop, Timer,
      Training's stop question), and the "Status Bar — Activities", "Activity open" and
      "Activity, stop?" strips. The bar is detached in those strips to hold the chips
- [x] The activities' tooltips ("title · subtitle"): the "Status Bar — Activity tooltip" strip
- [x] Status items from other apps (Drop's item): `Bar / App item` (State = Rest, Hover,
      Attention), `Bar / App item menu`, and the "Status Bar — App item" and "App item menu" strips

### Launcher (`apps/shell/src/surfaces/launcher/`)
- [x] An app's and a file's menu (`app_menu.rs`): `Launcher / App menu` (Kind = App, File, each
      with its submenu open), `Bar / Workspace submenu` (Kind = Focused monitor), `Launcher / Open With submenu`, and the
      "Launcher — App menu" and "Launcher — File menu" scenes
- [x] The assistant's command card (`assistant.rs`), e.g. "Turn On Night Light": it's the answer
      card with "Assistant" over the command and "Return to run" (or "Return to confirm"). Drawn in
      the "Launcher — Assistant" and "Launcher — Assistant confirm" scenes. The confirmation dialog
      itself waits for the alert dialog (see Alerts)
- [x] Phone apps in the results (`phone_apps.rs`): a result row with the phone glyph tile and
      "On Pixel 8", ranked under the computer's own app. Drawn in the "Launcher — Phone apps" scene

### Desktop widgets (`apps/shell/src/surfaces/widgets/`)
- [ ] Edit Widgets: the remove badges, resize handles, drop and pull outlines, keyboard focus
      ring, the empty-desktop hint
- [ ] The gallery: sidebar (search, categories), pane (previews, names, size pictures), footer
      (hint, Undo Remove, Remove All, Done)
- [x] The Show and Graph submenus for kinds other than the clock: `Widget / Reading menu` (Kind =
      CPU, Temperature, Network, Disk, Batteries, and two with a submenu open), `Widget / Setting
      submenu` (Graph, Sensor, Interface, Disk, Device), and the "Temperature menu" and "Network
      menu" scenes. Memory's menu is CPU's

### Alerts (`apps/shell/src/surfaces/alert/`)
- [ ] The shell's alert dialog, in thick glass: polkit authentication, Bluetooth pairing, the
      phone's alerts, unlocking the password vault
- [ ] Recreate the dialog's tokens first (penpot-backlog.md, "For the dialog design")

### Password fill (`apps/shell/src/surfaces/fill/`)
- [x] The popup by a password field, with the app's Bitwarden entries: `Fill / Popup` (Unlocked,
      Locked, Unverified) on "Password Fill — Components", and the "Password Fill — Suggestions"
      scene. Not drawn: the "fill anyway?" confirmation row and a fill error's note row

### Notifications
- [ ] Notification actions' context menu, if any, and a swipe in progress (optional)

## Apps

### Emaki Settings (`apps/settings/`)
Only Appearance is drawn. Still to draw:
- [ ] About
- [ ] Cursor
- [ ] Desktop and Desktop Widgets
- [ ] Displays (arrangement, the trial countdown, night light)
- [ ] Dock
- [ ] Drop
- [ ] Input (keyboard, pointer, key bindings)
- [ ] Launcher
- [ ] Network (connections, IPv4)
- [ ] Notifications
- [ ] Passwords
- [ ] Quick Settings
- [ ] Screenshots
- [ ] Search results in the sidebar
- [ ] Sound
- [ ] Status Bar
- [ ] System (updates, generations, apps, services, storage, power)
- [ ] Windows (the niri settings)

### Messages (`apps/messages/`)
- [x] Phone Input: the trackpad, the Left/Middle/Right buttons, the keyboard hint: the "Phone
      Input" scene. The trackpad's group fill stays raw (`style::group_fill`, a TODO(token)). Not
      drawn: Phone Input with Remote Input off, which is only its title and a caption
- [x] No phone connected, and the inbox's captions (asking the phone, no answer with Try Again,
      SMS not shared, off the network, no conversations): `Messages / Inbox note`, and the
      "No answer", "SMS not shared" and "No phone" scenes
- [x] An empty thread pane ("Choose a conversation, or start a new one."): the "Empty thread" scene
- [x] Photos and videos, file chips, link chips: `Messages / Photo` (Photo, Video, Opening),
      `Messages / File chip` (Rest, Opening), `Messages / Link chip`, and the "Attachments" scene
- [x] Attached-file chips over the reply field, and "may be too big to send": `Messages /
      Attachment chip` (Image, File, Too big), in the "Attachments" scene with "and 1 more"
- [x] The floating New Message pill, and Load Earlier Messages: `Messages / New Message pill`, and
      the "Earlier and New Message" scene
- [x] A message's right-click menu: `Messages / Message menu` (Kind = Code, Link, Failed), and the
      "Message menu" scene

### Drop (`apps/drop/`)
- [ ] The grid of folders, apps and actions, and the Drop Bar
- [ ] Work in progress on a tile

### Phone Screen (`apps/mirror/`)
- [ ] The mirrored window and its controls, the phone's apps, the camera as a webcam

### Developer tools (low priority)
- [ ] Shader Lab (`apps/shader-lab/`)
- [ ] train-watch's alert when a run ends (`apps/train-watch/`)

## Kit controls with no Penpot component yet (`crates/kit/src/`)
- [ ] Color picker (`controls/color_picker`)
- [x] Stepper, spinner, tag, pop-up button: `Kit / Stepper`, `Kit / Spinner`, `Kit / Tag`,
      `Kit / Pop-up button`, on "Settings — Components"
- [x] The pop-up button's open menu (`menu/popup.rs`) and the progress bar in `spinner.rs`:
      `Kit / Pop-up menu` and `Kit / Progress bar`. The menu's panel fill stays raw until
      `sys.color.surface.menu.bg` exists (`style::menu_fill`, a TODO(token))
- [x] Text field (`text_field.rs`, `app::field_box`): `Kit / Text field` (Rest, Filled, Focused,
      Warning, Error), on "Settings — Components". The kit has no multi-line text edit:
      `text_edit.rs` is one line's editing, with no drawing, and Messages' reply field is its own
- [x] Tooltip (`containers/tooltip.rs`): `Kit / Tooltip`, on "Status Bar — Components"
- [x] Toast container, status page, app bar: `Kit / Toast` (Text, Action), `Kit / Status page`
      (Text, Button, Compact), `Kit / App bar` (Back, Actions), on "Settings — Components". The
      toast's fill and edge stay raw (`style::menu_fill` and a white @ 0.1 literal). Two
      mismatches in the code show as drawn: the toast's 30 px button in a 34 px pill, and the status
      page's 28 px button slot under a 30 px button
- [ ] Dialog (see Alerts)
- [x] One shared button set: `Kit / Push button` (Style = Filled, Tonal, Plain, Destructive ×
      Shape = Square, Round × State = Rest, Disabled) and `Kit / Icon button` (Style = Plain, Tonal,
      Filled × Size = Regular, Small × State = Rest, Selected, Disabled). The copies under Launcher
      and Notifications are gone. A capsule's radius stays raw: `Radius::Full` has no token
- [ ] `Widget / Card` is unused: each reading's main is the card itself. Building the readings on
      a card copy would mean moving glass, shadow and fill into a copy across 37 variants
- [ ] A form group (header, card of rows, footer) can't be a component: a copy can't take new
      rows. The rows and separators are components; the card is token-bound

## Scenes are built from slot components
Each surface's repeated frame is a component whose body is a nested variant set, switched per
scene (`h.slot(copy.children[i], 'Kind', …)`):
- `Launcher / Sheet` with `Launcher / Body` (Kind = Results, Phone apps, Answer, Applications,
  Applications list, Actions, Files, Clipboard, Clipboard ask, Scoped, No files). The pill scenes
  hide the divider and body
- `Messages / Window` with `Messages / Sidebar` (Conversations, Note) and `Messages / Pane`
  (Thread, Group thread, Attachments, Earlier, New Message, Empty, Phone Input, No phone, SMS not
  shared)
- `Notifications / Center` with `Notifications / Center body` (List, Group open, Empty)
- `Settings / Window` (Material = Glass, Solid) with `Settings / Page`; a second Settings page
  joins `Settings / Page` as a variant (a one-variant set can't be made)
- `Bar / Bar`'s status row holds a hidden app item and three activities (Drop, Timer, Training)
  that the bar strips show and switch
- `Dock / Dock` has State = Rest, Dragging, Unpinning

## Leftovers to confirm with the user
- A `Glyph item` board (not a component) on "Status Bar — Components"
- A collapsed `Icon` text on "Messages — Components"

## Across the file
- [ ] Light-mode boards: switching the theme recolours the file, but no board is kept in light
- [ ] Compact density: only checked by switching it, not kept as boards
- [ ] Re-check text that went missing in light-mode exports (the widgets' Temperature and Disk
      headers)
