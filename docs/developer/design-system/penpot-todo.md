# What's not drawn in Penpot yet

What the Penpot file ("New File 5") doesn't have yet, by surface. For tokens still to recreate or
delete, see [penpot-backlog.md](penpot-backlog.md).

Already drawn: the dock, the status bar, Quick Settings (the grid and its states), Emaki
Settings' Appearance page, the launcher, the OSD, notifications, the desktop widgets and Messages
(inbox, thread, New Message, the empty and no-phone states).

## Shell surfaces

### Quick Settings (`apps/shell/src/surfaces/quick_settings/`)
- [ ] Detail pages: Sound, Wi‑Fi, VPN, Bluetooth, Power Mode, Battery, Night Light, Do Not
      Disturb, Keyboard, Session (the power menu), Background Apps, Tailscale, Keep Awake, Phone
- [ ] Edit mode and the customize catalog (`customize/`): dragging a control, the outlines, the
      catalog of controls to add
- [ ] The slider card's other states, if any beyond what "Quick Settings — States" shows

### Dock (`apps/shell/src/surfaces/dock/`)
- [ ] Window and app menus (`window_menu.rs`): focus, fullscreen, float, move to workspace, pin,
      close; a pinned app's open, open on another workspace, unpin
- [ ] Drag states: an icon in hand, the gap opening, dropping off the dock

### Status bar (`apps/shell/src/chrome/`)
- [ ] The workspace menu (`workspace_menu.rs`)
- [ ] Activities in the bar (Drop's work, train-watch's run) and their tooltips
- [ ] Status items from other apps (Drop's item)

### Launcher (`apps/shell/src/surfaces/launcher/`)
- [ ] An app's and a file's menu (`app_menu.rs`)
- [ ] The assistant's command card (`assistant.rs`), e.g. "Turn On Night Light"
- [ ] Phone apps in the results (`phone_apps.rs`)

### Desktop widgets (`apps/shell/src/surfaces/widgets/`)
- [ ] Edit Widgets: the remove badges, resize handles, drop and pull outlines, keyboard focus
      ring, the empty-desktop hint
- [ ] The gallery: sidebar (search, categories), pane (previews, names, size pictures), footer
      (hint, Undo Remove, Remove All, Done)
- [ ] The Show and Graph submenus for kinds other than the clock

### Alerts (`apps/shell/src/surfaces/alert/`)
- [ ] The shell's alert dialog, in thick glass: polkit authentication, Bluetooth pairing, the
      phone's alerts, unlocking the password vault
- [ ] Recreate the dialog's tokens first (penpot-backlog.md, "For the dialog design")

### Password fill (`apps/shell/src/surfaces/fill/`)
- [ ] The popup by a password field, with the app's Bitwarden entries

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
- [ ] Phone Input: the trackpad, the Left/Middle/Right buttons, the keyboard hint
- [x] No phone connected, and the inbox's captions (asking the phone, no answer with Try Again,
      SMS not shared, off the network, no conversations): `Messages / Inbox note`, and the
      "No answer", "SMS not shared" and "No phone" scenes
- [x] An empty thread pane ("Choose a conversation, or start a new one."): the "Empty thread" scene
- [ ] Photos and videos, file chips, link chips
- [ ] Attached-file chips over the reply field, and "may be too big to send"
- [ ] The floating New Message pill, and Load Earlier Messages
- [ ] A message's right-click menu

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
- [ ] Stepper, spinner, tag, pop-up button
- [ ] Text field and multi-line text edit, as kit components (Messages' and Settings' are drawn
      on their pages only)
- [ ] Tooltip and toast containers, status page, app bar
- [ ] Dialog (see Alerts)
- [ ] One shared button set (push, icon, round) in place of the copies under Launcher and
      Notifications

## Across the file
- [ ] Light-mode boards: switching the theme recolours the file, but no board is kept in light
- [ ] Compact density: only checked by switching it, not kept as boards
- [ ] Re-check text that went missing in light-mode exports (the widgets' Temperature and Disk
      headers)
