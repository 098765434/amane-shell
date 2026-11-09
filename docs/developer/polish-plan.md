# Polish plan: one copy of everything

## Context

The cleanup roadmap (`docs/archive/ROADMAP.md`, Phases 0–13, finished 2026-11-07) fixed the layers: crates, surface folders, typed IPC, state files, one menu, logging. What it left is duplication *inside* those layers. Three read-only sweeps of the workspace (110k lines) found roughly 2,500–3,000 removable lines. The duplication comes from four sources:

1. **Missing foundation.** About 20 helpers (XDG paths, writing a file whole, "x ago", byte sizes, unix time, poison-safe locks, `--flag` parsing, MIME lookup, string interning) are written 2–12 times each. The cause: `phone`, `assistant` and `pinentry` can't depend on the kit, and nothing below the kit holds them.
2. **Missing primitives.** The kit lacks a few small building blocks, so every surface rebuilds them: a fixed-size frame (~108 sites), a reveal "swell" (6 sites), a press action (18 sites), a pressable row, a scroll pane, edit-mode chrome, and a catalog sheet. Customize and the widget gallery are near-copies of each other.
3. **Hand-mirrored tables.** One fact is written several times and kept in step by hand:
   - IPC: `parse` and `to_lines` mirror each other, ~390 lines.
   - Emaki Settings: the page list is written in 7 places, and `search.rs` copies the titles.
   - Quick Settings: the controls are spread over 7 parallel `match`es.
   - Choice enums: 22 `ALL` arrays.
4. **Missing service helpers.** The same plumbing is repeated across services, and it hides a real bug: listeners call `read()` where they should call `peek()`, which counts as being watched and defeats idle-stop. Affected: weather, background apps, tailscale, Do Not Disturb, night light, keep-awake.

**Outcome:** one copy of each helper, data-driven tables where facts used to be mirrored, the kit as the single place for UI idioms, and lints that keep it this way.

**Decisions:**
- **New crate:** a std-only `emaki-base`.
- **Visible changes:** allowed where copies disagree today. Each one is listed in §Visible changes.
- **Constraints:**
  - Forks stay cheap to rebase, so no new amane API: service helpers are kit extension traits.
  - New UI stays on the quad pipeline.
  - Diagrams in docs are mermaid.
- **Commits:** each step is one commit that builds and passes CI.

## Target shape

```mermaid
graph TD
  base[emaki-base<br/>paths · fs · time · format · sync · args · mime · intern]
  process[emaki-process] --> base
  log[emaki-log]
  phone[emaki-phone] --> base
  assistant[emaki-assistant] --> base
  pinentry[emaki-pinentry] --> base
  nixos[emaki-nixos] --> base
  kit[emaki-kit<br/>re-exports base] --> base
  kit --> amane[amane fork]
  apps[shell + apps] --> kit
```

---

## Phase 0: Baseline (S)

**Status (2026-11-07).** Done at shell `e7a0831`: clippy clean, 712 tests pass.

| Crate | Lines |  | Crate | Lines |
|---|---:|---|---|---:|
| apps/shell | 44,326 | | crates/kit | 25,775 |
| apps/settings | 15,244 | | crates/phone | 4,240 |
| apps/messages | 3,334 | | crates/nixos | 2,582 |
| apps/drop | 3,327 | | crates/assistant | 1,759 |
| apps/shader-lab | 3,054 | | crates/status | 1,091 |
| apps/mirror | 2,834 | | crates/pinentry | 926 |
| apps/assistant | 1,103 | | crates/process | 423 |
| apps/train-watch | 628 | | crates/log | 87 |

110,733 lines of Rust in all; 93 files over 400 lines; 103 poison-tolerant lock spellings; 576
`Size::Fixed(` wrappers.

- Record the metrics: line counts per crate and files over 400 lines. Confirm that `cargo clippy --workspace --all-targets -D warnings` and `cargo test --workspace` are green.
- Take screenshots for later comparison:
  - the bar, Quick Settings (grid, Customize, the Bluetooth and Phone pages), the widget gallery and edit mode, the launcher, and the OSD;
  - the Emaki Settings pages (Appearance, Status Bar, Displays, System → Apps);
  - Drop, Messages and Shader Lab.
- Land this plan as `shell/docs/development/polish-plan.md`. It moves to `docs/archive/` when finished, as the roadmap did.

## Phase 1: `emaki-base`, the one copy of each helper (M)

**New crate `crates/base` (std only, plus `serde`/`serde_json` for `fs::save_json`):**

| Module | Contents | Replaces |
|---|---|---|
| `paths` | moved from `kit/src/foundation/paths.rs`, which the kit now re-exports | `phone/kdeconnect/{config,files,contacts}.rs`, `assistant/lib.rs:49`, `pinentry/peer.rs:16`, `apps/assistant/main.rs:56`, `train-watch/main.rs:74`, `shell/services/vault/rbw.rs:125`, `messages/phone.rs:15` |
| `fs` | `write_whole(path, bytes, Write { fsync, private })` and `save_json(path, &T)` (pretty, logs on failure) | `kit/look/glass.rs:172`, `kit/platform/secrets.rs:170`, `nixos/{settings,catalog}.rs`, `phone/kdeconnect/files.rs:43`, `drop/{shared,state}.rs`, `mirror/apps.rs:71`, `shader-lab/configs.rs:104`, `settings/system/defaults.rs:408`, `settings/displays/kdl.rs:566`, the three save blocks in `kit/platform/settings/seen.rs` |
| `time` | `unix_ms()`, `unix_s()` | 6 copies (`sms.rs:1151`, `activities.rs:228`, `file_search.rs:471`, `usage.rs:130`, `clipboard_history.rs:421`, `phone/apps.rs:70`) |
| `format` | `ago(Duration)`, `span(Duration)` ("2 h 5 min"), `bytes(u64)`, `count(n, one, many)` | the "ago" copies (`nixos/lock.rs:24`, `settings/system/update.rs:126`, `passwords.rs:213`, `launcher/model.rs:333`, `notifications/card/parts.rs:134`); h/min in `tiles/power.rs:161`, `tiles/keep_awake.rs:21`, `catalog/battery.rs:81`, `train-watch/log.rs:188`; bytes in `storage.rs:32`, `catalog/system.rs:22`, `launcher/model.rs:342`; plurals in 4 sites |
| `sync` | `trait LockExt { fn locked(&self) -> MutexGuard<T> }` for `Mutex` and `RwLock` | about 76 `unwrap_or_else(PoisonError::into_inner)` / `\|e\| e.into_inner()` sites, and `assistant.rs:74` `state()` |
| `args` | `flag(&args, name)`, accepting both `--n=v` and `--n v` | 5 copies with 3 behaviours (settings, messages, mirror ×2, shader-lab) |
| `mime` | `mime(path)`, `is_image(path)` | `drop/share.rs:91`, `drop/grid.rs:1399`, `messages/compose.rs:263`, `phone/sms.rs:156`, `launcher/provider/files.rs:97` |
| `percent` | `encode` / `decode` | `media_art.rs:91`, `file_search.rs:423`, `rbw.rs:180`, `launcher/model.rs:263` |
| `intern` | `intern(String) -> &'static str`: one bounded leak, deduped through a set | `shader-lab/panel.rs:1031`, `settings/main.rs:552`, `settings/system/controls.rs:82`, `kit/color_picker/mod.rs:390`, `status_bar/indicators.rs:120`, `widgets/mod.rs:222` |

- **`app_ids`:** move from the kit to base, so that `emaki_log::init(app_ids::SHELL)` works in the crates that don't use the kit. This replaces the 9+ `"emaki"` literals.
- **Hex colours:** keep the kit's `tokens::color` parser as the only one. `color_picker` and `settings/appearance.rs:73` call it.
- **Opening files:** add `kit::open(path_or_uri)` on one launch API, for the 12 `xdg-open` sites.

**Lands as four commits:**
1. The crate and its tests.
2. The kit re-exports base, and kit-internal sites move onto it.
3. The non-kit crates move onto it.
4. The shell and apps move onto it, and every copy is deleted.

**Status (2026-11-07).** Done (shell `a35a1ab`, `0cb5186`, `bf655e4`, `b11708a`).
- **Changes from the plan:**
  - `fs::part_of` names a part file per process (`<file>.part<pid>`), as the glass shader's did, so two programs writing one file never share a part.
  - `paths::private_runtime` is new: the pinentry's socket goes without rather than fall back to the shared temporary folder.
  - `sync` is two traits, `Locked` (`locked()`) and `Shared` (`reading()`, `writing()`). Condvar waits keep `PoisonError::into_inner`: there's no guard to wrap.
  - `format::span` rounds to the nearest minute and shows seconds under one, so keep-awake no longer needs to round up.
  - `kit::launch::open` takes the eight UI-side `xdg-open`s. The rest stay: the service threads run off the event loop, and the phone crate and train-watch don't use the kit.
  - The media-art download (a `curl` into a part file) and the nixpkgs catalog (streamed) still write their own part files.
- **Found along the way:** `emaki-nixos`'s `the_steps_of_an_apply_run_in_order` failed about one run in three ("text file busy": a fake rewritten while another test forked). Its fakes are now links to one stub (`86b759f`).
- **Asked for during the phase:** the last Material 3 traces (component docs written as departures from Material's, the counterpart columns in `docs/kit`) and Messages' copy of the kit's `Avatar` (`16ee963`).

## Phase 2: Kit interaction core (M)

- **`Actions`:**
  - `Actions::press(f)`, plus `on_hold` / `on_release`. This replaces the 18 `Actions { press: Some(Rc::new(..)), ..Default::default() }` sites, `parts::on_press`, and `fill/view.rs:357`.
  - The stepper drops its hand-wired hold (`stepper.rs:200-225`).
- **One hover layer:**
  - `hover::layer(id, fill, Tone)` built on one `fade`; `buttons::state_fill` and `hover::hover_fill` both go.
  - The `.mac-hover` key is retired.
  - One `DISABLED_OPACITY` token replaces 0.38, 0.5 and the literal in `form.rs:398`.
- **One pressable row:** `pressable(rect, id, base_fill, disabled, Option<Actions>)`, used by `containers/list.rs:219`, `containers/form.rs:367`, `platform/app.rs:258` and `menu/popup.rs:405`.
- **Checkbox and radio become one `Mark { Check | Radio }`.**
  - It reads `control_size::current()`, which removes the literal `px(16)`/`5.5`/`4.8` (also in `parts::labelled`).
  - `ControlSize::checkbox()` and `radio_dot()` finally have users.
- **Round glyph buttons:** `pop_up_button`, `nav_buttons` and the stepper halves become an `IconButton` form variant.
- **`look/style.rs`:**
  - Delete the nine pure-alias functions; call sites use `sys::` directly.
  - Delete the dead `knob()` and `error()` (the latter hard-codes `#ff6961`).
- **API consistency:**
  - Every control id takes `impl Into<String>` (`Stepper` didn't).
  - Every control uses `disabled(bool)` (`NavButtons` had `back_enabled`).
  - `Slider` gets an id.
- **Named motion constants:**
  - `popup.rs` drops its `FADE` and uses `motion::FADE`.
  - The 300 ms crossfade and the 250 ms swallow window get names.
  - `held()` and `pressed()` share one timeout, and their docs are fixed to match it.
- **Delete unused public items:** `ListItem::leading_tile_accent`, `Launcher::keys_for`.
- **`pad_x(l, r)`:** a helper beside `layout::pad`, for the ~12 hand-built `Padding {..}`.

**Status (2026-11-07).** Done (shell `1505a3b`, `6366dcd`, and the `pad_x` commit after them).
- **Done as planned:** `Actions::press`, one hover layer and one fade, one `DISABLED_OPACITY` (0.4), the checkbox's and radio's one mark, named timings, the dead items, `Stepper::new(impl AsRef<str>)`, `pad_x`.
- **Kept, on a second look:**
  - **`look/style.rs`'s aliases:** they are the form controls' vocabulary, written once; inlining them would spread `material::label(Level::Primary)` and the like over about 110 call sites.
  - **One `pressable` row for the list, the form row, the app sidebar and the menu:** they share only the last three lines. Each has its own fill, hover colour and radius, so a helper would save little and hide more.
  - **The round glyph buttons as an `IconButton` variant:** the pop-up, nav and stepper buttons are drawn in the form controls' look, which `IconButton` isn't. Folding them would change how Emaki Settings looks.
  - **`NavButtons`' `back_enabled`/`forward_enabled`:** it has two sides, and one `disabled(bool)` can't say which.

## Phase 3: Layout, motion and container primitives (M)

**Layout:**
- `layout::frame(w, h)` (a transparent fixed box) and `layout::place(widget, x, y, w, h)`, promoted from `widgets/catalog/mod.rs:85`.
- A mechanical pass deletes the needless `Size::Fixed(..)` wrappers (254 of them; `From<f32> for Size` exists in `amane/src/placement.rs:154`).

**`Area`:** a kit pixel-rect type with `contains`, `centre`, `scaled`, `clipped`. It replaces the 5 rect structs (`customize/mod.rs:77`, `gallery.rs:48`, `launcher/shape.rs:28`, `quick_settings/mod.rs:300`, and grid `Slot` as a newtype). Alongside it:
- `Rim::scaled`
- `Material::shape_of(&Rim)` (from `tiles/mod.rs:399`)
- the "scale about the centre" copies in `tiles/mod.rs:261,318`, `widgets/view.rs:164` and `customize/drag.rs:96` move onto `Area::scaled`.

**`Reveal`:**
- It gains `visible()` and `swell(from_scale) -> Swell { scale, opacity, frost }`.
- The 6 hand-written swells (alert, launcher, customize, gallery, OSD, Quick Settings) use it.
- Customize, the gallery and the OSD drop their own `Animation` plus `OPENING` copies of `Reveal`.
- The ~10 `0.001`/`0.999` checks become `visible()`.
- `OPEN_SCALE` comes from the token everywhere.

**Motion:** `motion::ease(dur)` and `motion::spring_for(dur, bounce)`, which respect reduced motion. Used by `status_bar/mod.rs:101`, `workspace_shape.rs:267` and `dock/motion.rs:165`.

**Containers:**
- **`ScrollPane { kinetic, bar }`** with `wheel`, `reset`, `opacity`, and one `WHEEL_STEP`. Used by the launcher and Customize.
- **`grid::edit`:** `remove_badge`, `resize_handle`, `drop_outline`, `BAND`, taken from `quick_settings/edit.rs:359-500` and `widgets/edit.rs:267-356`.
- **`CatalogSheet` and `catalog_item(preview, w, h, title)`:** the sidebar with search, the pane, and the footer (hint, Undo Remove, Reset, Done). Customize and the gallery become thin configurations of it, about 250 lines less.
- **`ListItem::chevron()`:** one trailing-chevron size and level.

**Shell chrome:** `panel::hidden(window)` for the 6 `.visible(false).blur_shapes(vec![]).child(blank())` sites.

**Status (2026-11-07).** Done (shell `0ba540c` … the `WHEEL_STEP` commit).
- **Done as planned:**
  - `layout::frame`; 505 `Size::Fixed` wrappers gone (65 are left, each wrapping a bare literal, which wouldn't infer as `f32`); 161 redundant transparent fills.
  - `reveal::Swell` and `reveal::showing` for the six swells and the threshold checks.
  - `motion::eased`, `panel::empty`, `list::chevron`.
  - `chrome::catalog::Sheet` for Customize and the gallery, and `chrome::edit_marks` for both edit modes, both in `chrome` rather than the kit: the kit's grid is pure by design, and only the shell has these sheets.
  - The kit's `area::Area` replaces the surfaces' copies.
- **Smaller than planned:**
  - **No `ScrollPane` type:** the two copies were a line each, so `kinetic::WHEEL_STEP` alone took the duplication.
  - **`Area` reaches the launcher too** (it gained a `Blend`). Quick Settings' `Sheet` keeps its own type: it carries a radius and a tint as well.
  - **Left in place:** the four "scale about the centre" copies. Each is one line of arithmetic on different types.

## Phase 4: One table per fact (L): the architectural core

Each table becomes the single source; parsers, lists, usage text and search are derived from it.

### 4a. Shell IPC
- **`keywords!` macro** for unit enums (`Switch`, `Listen`, `Light`, `LauncherAction`, …). It generates `as_str`, `FromStr` and the `a|b|c` usage text.
- **`trait Words { fn read(&[&str]) -> Option<Self>; fn write(&self, &mut Vec<String>); const USAGE: &str }`** on each sub-command. `ShellCommand::parse` and `to_lines` (`kit/platform/shell_ipc.rs:338`, `:531`) shrink to a dispatch over verbs.
- **`VERBS`** is derived from that dispatch.
- **`apps/shell/src/ipc.rs:19`:** its 13 closures become a loop.
- **Usage text:** `launcher/ipc.rs:7` reuses the generated text.
- **Format:** the existing `LINES` round-trip test pins it unchanged.
- **Size:** about 200 lines less.

### 4b. Choices and settings files
- **`trait Choice: Copy + Eq + 'static { const ALL: &[Self]; fn label(self) -> &'static str }`**, with `index`/`from_index` defaulted. A `choices!` macro generates it for the 22 `ALL` enums.
- **`settings_file!(Dock, app_ids::SHELL, "dock", dock, change_dock)`** for the 11 `impl Settings` + `fn x()` + `fn change_x()` blocks.
- **`SeenFile<T>`** for `seen.rs`'s `remember_status_app` and `remember_tray`.

### 4c. Quick Settings controls
- **`const CONTROLS: [ControlSpec; 22]`**, where each `ControlSpec` holds:
  - `id`, `name`, `title`, `spans`, `default_span`, `hidden`;
  - `available: fn() -> bool`;
  - `kind: Toggle(fn() -> Option<QuickToggle>) | Cell(fn(Span, bool) -> Option<Box<dyn Widget>>)`;
  - `page: Option<Page>`.
- **What it removes:** the 7 `match`es in `quick_settings/controls.rs:82-270` and the third copy of availability in `pages.rs:116`.
- **Tiles:** they stop re-checking `present()` themselves.

### 4d. Emaki Settings pages and search
- **`const PAGES: &[PageDef]`**, where each `PageDef` holds `name`, `aliases`, `title`, `icon`, `accent`, `words`, `body`, `subs`, `toolbar`. Everything else is derived from it:
  - the index constants, `page_named`, `PAGE_NAMES`, `SUBS` and `sub_page`;
  - the sidebar and `content()`'s scroll ids;
  - `toolbar_action`;
  - the `page(..)` entries in search.
- **`RowSpec { id, title, words }` constants:** pages build their rows from them (`FormRow::spec(&ACCENT)`), and search iterates the same constants. The 63 copied rows in `search.rs` and the source-grepping drift test (`search.rs:732`) go. A structural test replaces it: every spec is reachable and every page has its rows.
- **Size:** about 400 lines less, and adding a page touches one place.

**Status (2026-11-07).** Done, in a lighter form than planned where the plan's form would have cost readability.
- **4a, the IPC:**
  - A `words!` macro spells each keyword enum once, both ways, and `handlers![…]` makes the shell's handler table.
  - **Not done: a table generating `parse` and `to_lines` whole.** The grammar has defaults, optional and trailing arguments, and lenient forms (`dnd` alone, `close` only without a mode). A macro that covered them would be harder to read than the two matches, which the `LINES` round-trip test already holds together.
- **4b, choices:**
  - `settings_file!` replaces the eleven `Settings` impls and their accessors.
  - `Dropdown::choice(id, options, now, pick)` takes `(label, value)` pairs, which serves both the enums' `ALL` and the hand-made tables. So no `Choice` trait was needed.
  - `SeenFile<T>` went in Phase 1, as `remember_named`.
- **4c, Quick Settings:** `controls::SPECS`, with `ControlId` in its order. A page's availability is its control's.
- **4d, Emaki Settings:**
  - `pages::SIDEBAR` holds each page once; the search index builds its page entries from it.
  - One visible change: the search shows the shell's About page under its sidebar title, "About", rather than "About Emaki Settings".
  - **Not done: `RowSpec`.** The 63 rows the index copies from the pages still have their own titles there, and the source-grepping test still checks them. Moving every page's rows onto shared constants is a change to each page; it fits best with Phase 6's form helpers.

## Phase 5: Service helpers, and the idle bug (M)

- **`ServiceExt` (in the kit, no amane change):**
  - `S::replace(fresh) -> bool` compares through `peek()` and writes or stays `quiet`.
  - It replaces the hand-written "write if changed" in `weather.rs:68`, `background_apps.rs:81`, `updates.rs:58` and `connections.rs:41`, plus the 10 Settings sites (`system/apps.rs`, `repo.rs`, `installed.rs`, `update.rs`, `services.rs`, `generations.rs`).
  - **This fixes the `read()`-in-listener bug.** The remaining listener reads (`tailscale.rs:334`, `do_not_disturb.rs:43`, `night_light.rs:163`, `keep_awake.rs:118`) become `peek()`.
- **`state_service!(T)`** for the 13+ services with an empty `listen()` (Settings, Messages, Mirror, Shader Lab).
- **`Busy<K>`:** "what's running, plus the last error", with `run(key, job)`. It replaces Settings' `Removal` (`system/apps.rs:367`) and `Actions` (`system/services.rs:208`).
- **`Mirror<T, Marker>`** and **`Wake::follow(&[subs], check)`** for `phone/pairing.rs:36`, `alert/bluetooth.rs:41`, `hdr_white.rs:44`, `bt_battery.rs:24` and `power_refresh.rs:47`.
- **`Debounced<Q, R>`:** the latest query wins. Used by `launcher/file_search.rs:177` and `services/assistant.rs:57`.
- **Shell-side:**
  - `alert::sync_request` for the 4 alert `sync()`s.
  - A `DevicePage` state with `with_sheet` for the Bluetooth and Phone pages; their duplicated `sheet_height` goes.
  - `services::power::batteries()` as the one device list; today three places build it with different coverage.
  - A `PowerProfile` enum instead of strings (`tiles/power.rs:28`).

**Status (2026-11-07).** Done for what paid (shell `d3c23f6`, `e1b589d`).
- **The idle bug:** the weather, background-apps, Tailscale, Do Not Disturb, night-light, keep-awake and adb listeners read themselves through `peek()`.
- **`ServiceExt::replace`:** the listener's "keep it if it changed", in place of eleven copies.
- **`Busy<K>`:** Emaki Settings' uninstalls and service actions.
- **Left, on a second look:**
  - **`state_service!`:** the empty-listener services build their `new()` by hand and don't all derive `Default`, so a macro would save three lines each and impose `Default`.
  - **`alert::sync_request`:** the four alerts each differ in the middle (the Phone page asks first, polkit clears its field on a new request and rebuilds on a new version).
  - **`Mirror`, `Wake::follow` and `Debounced`:** each was two or three copies of a few lines with different types and wake-ups; a generic version reads worse than the copies.
  - **`DevicePage`, `batteries()` and `PowerProfile`:** each still waits for a change that touches them.

## Phase 6: Apps (M)

**Emaki Settings:**
- `form::choice(id, now).on_pick(..)` built on `Choice`. It replaces about 15 eleven-line dropdowns, and the 14 literal widths become one `Width::Menu` token.
- `form::picker_row(..)` replaces the 6 `ThumbnailPicker` sequences.
- `form::bound_field` is promoted from `drop.rs:368` and replaces the four ways of filling a field from a setting. **This fixes the refill-on-clear bug** in `about.rs:216`, `controls.rs:201` and `night.rs:347`.
- `content_width()` replaces the 6 `page_width() - 2·row_pad_h`.
- One `parse_mode`, shared by `kdl.rs:329` and `sections.rs:1223`.
- `refresh_rate` returns an `Option`. The dead `let _ = slowest` and `let _ = commit` go.
- **Colours:** literal colours become `Accent::*` (`services.rs:153`, `about.rs:34`, `system/about.rs`, `previews.rs:262`).
- `"main"` becomes a constant.

**Drop:**
- `grid.rs` (1,444 lines) splits into `geometry`, `tile`, `prompt`, `shelf`, `actions`.
- Its 35 literal sizes become `sys::` tokens, and its local copies (`fixed_size`, `with_alpha`, `clip`) give way to the kit's.
- `ask` gets `dismiss_then` and a prompt builder.
- The identical `run`/`hold` IPC handlers merge into one.

**Shader Lab:**
- `ScrollArea` replaces the `(Column, f32)` height plumbing, if it fits (check first; keep the manual height if it doesn't).
- `PushButton::disabled` replaces the faked opacity.
- `layout::caption` replaces the local `caption` (and Settings' `note()`).

**Phone, Messages and Mirror:**
- **One D-Bus call helper:** `kc::call(device, sub, iface, method, args)`. The 8 raw `try_call`s in `sms.rs` and `get_all` move onto it.
- **Shared phone files and matching:**
  - `emaki_phone::{apps, screen}` own the `phone-apps.json` and `phone-screen.json` formats, which mirror, the shell and Messages each read or write today.
  - `same_number` and `digits` become public, along with a new `thread_named`. Messages' 3 re-implementations and their different threshold go.
- **Shared input and selection:**
  - `PhoneKey::from_key` and `android_keycode` serve both Messages and Mirror.
  - One phone-selection helper replaces three different rules.
- **Getters:** `Value::string(k)` for the 11 property reads. The trivial getters become public fields where nothing guards them.
- **Mirror:** uses `app_window` and its token background.

**`emaki-nixos` and `emaki-status`:**
- `nixos/settings.rs:262-420` uses serde derives (lenient: `Vec<Value>`, then `from_value` per entry).
- `status/protocol.rs` gets a `Payload` trait for its five `decode`s.

**train-watch:** `mod log` is renamed to `run_log`, so `::log::` isn't needed.

**Status (2026-11-07).** Done for the larger items (shell `27d2797` … the caption commit).
- **Emaki Settings:**
  - `app::fill_once` replaces the six ways of filling a field from its setting, and fixes the fields that refilled themselves when emptied.
  - Seven dropdowns are on `Dropdown::choice` (Phase 4b).
  - The literal colours are accents.
  - `note` is the kit's `caption`.
- **Drop:** `grid.rs` is a folder (`mod`, `tiles`, `actions`, `prompt`, `shelf`); its prompt has one `dismiss()`; its copies of `with_alpha` and `fixed_width` are gone.
- **Phone, Messages and Mirror:**
  - `emaki_phone::shared::{apps, screen}` own the two shared files.
  - `same_phone_number` and `digits` are public, and Messages uses them.
- **train-watch:** `run_log`.
- **Not done yet:**
  - Shader Lab's `ScrollArea` change, and its own interning (Phase 1 took the interning).
  - `picker_row`, `content_width` and the one `parse_mode`.
  - `kc::call` and the getters in `crates/phone`.
  - `nixos/settings.rs` on serde derives.
  - The status crate's `Payload` trait.
  - Mirror's `app_window`.
  - `PhoneKey::from_key`.

## Phase 7: Long functions and finish (M)

**Split each function over 120 lines along what it does.** Today's list:
- shell IPC `to_lines` and `parse`: these go in 4a.
- Quick Settings: `surface()` 315, `tiles/mod.rs` `grid()` 258.
- launcher: `panel()` 178, `assistant.rs` `proposal()` 176 (a `Proposal` struct drops its `type_complexity` allow), `passwords::find` 154.
- Quick Settings `media::cell` 149; notification `card` 136. (Widgets `desk_window` and `view` are split, 2026-11-09: `widgets/window.rs` and `view.rs`; `widgets/` has `clippy::too_many_lines` on, at its default 100.)
- Settings: `apply_bar` 210, `appearance::page` 195, `package::view` 171, `services::page` 155, `drop::accounts` 153, `apps::search_section` 143, `night::section` 130, `status_bar::page` 126.
- Drop: `share::request` 138, `grid::ask` 136, `tile_cell` 132.
- Shader Lab `panel` 135.
- kit: `Stepper::build` 141, form row `build` 131, menu `popup` 124.
- dock `build`, with its `too_many_arguments` replaced by a struct.

**Fill popup:**
- It moves from the legacy `theme::`/`px()` styling onto `sys::` tokens.
- Its rows use the kit's menu rows (`controls/menu/rows.rs`).

**Widgets catalog:**
- `reading()` takes a `Reading` struct (no more allow).
- The `truncate(4)` hack goes.
- The one-line style aliases are removed.

**Leftovers:**
- Remove the dangling comments (`quick_settings/mod.rs:114`, `launcher/mod.rs:263`, `customize/mod.rs:110`) and the mid-file `use`s (`tiles/phone.rs:56,159`).
- `dock/motion.rs:96` `off()`: delete it or use it.
- `launcher/input.rs:362`: replace `unreachable_patterns` with explicit arms.
- `QuickToggle::build` moves its fields instead of cloning them.
- `material.rs:375` `shape_tinted` takes a rect, which drops its allow.
- Remaining literal gaps and colours become tokens (`activities/expanded.rs:146`, `chips.rs:18`, `widgets/view.rs:73`, …).
- Duplicate local `const f32`s fold into one (e.g. `PAD`/`MARGIN` = 16).

**Status (2026-11-07).** The leftovers are done (shell `0976444` … `cf943b9`):
- the stale `unreachable_patterns`, `dead_code`, `too_many_arguments` and `type_complexity` allows (`Reading`, `shape_tinted(Area, …)`, `Proposal`);
- the dangling comments, and the Phone page's mid-file `use`s;
- the import groups the earlier scripted moves had disturbed.

**Not done: splitting the long functions.** The list above stands, and wants doing one function at a time, each looked at live: they draw (`surface()`, `grid()`, `panel()`, `card()`, `apply_bar()`), so a mechanical cut would trade a long function for several that pass a dozen values between them. The fill popup's move onto `sys::` tokens and the kit's menu rows is the same kind of change, and also waits.

## Phase 8: Guardrails and docs (S)

**Lints:**
- `clippy::too_many_lines` warns at a threshold of 120 (`too-many-lines-threshold` in `clippy.toml`), so no new long function lands.
- **Workspace hygiene test** (a `#[test]` in xtask, beside the boundary tests). It fails on:
  - `PoisonError::into_inner` outside `emaki-base`;
  - `Size::Fixed(` with a literal;
  - `Box::leak` outside `intern`;
  - `"emaki"` literals outside `app_ids`;
  - a hand-written part-file rename outside `fs`;
  - `Service::read()` inside a `listen`.

**Docs:**
- Update `docs/development/shell-structure.md` (the primitives a surface uses, the tables) and the kit docs (`docs/kit/`).
- Add `emaki-base` to `docs/architecture.md` with the mermaid graph above.
- Archive this plan with a status per phase.

**Status (2026-11-07).** Done, but for the lint on long functions:
- **The hygiene test:** `xtask/src/hygiene.rs` covers poison-tolerant locks, leaked strings, raw XDG lookups, the shell's id typed out, a log named by hand, and transparent fills. It found three more fills and the vault's own XDG lookup.
- **Docs:** `architecture.md` has `emaki-base`, and `shell-structure.md` has "What a surface builds with" and "Tables".
- **Not done: `too_many_lines`.** With CI's `-D warnings` it would fail on the long functions Phase 7 leaves, so it waits for them.

## Phase 9: Where things live (research, S)

After the phases above, with the code in its final shape: should
`apps/shell/src/services` be a crate of its own, and should anything else move?
Weigh what each move buys (build times, tests without a window, reuse by the apps,
a boundary the compiler checks) against what it costs (a public API to keep, `pub`
items that were private, the cycle of a service that names a surface). Write down
the finding and the moves it recommends, with their order.


### Finding (2026-11-07)

**`apps/shell/src/services` stays in the shell; it doesn't become a crate yet.** What was measured:

| Question | What the code says |
|---|---|
| Is it separable? | Yes, cleanly. Its 10.2k lines (23% of the shell) import nothing from `surfaces/`, `chrome/` or `tokens/`, only each other and the shared crates (kit, phone, status, assistant, pinentry, nixos), and the boundary test in `surfaces/mod.rs` already holds that. |
| Would anything else use it? | No. The apps reach the shell's state through its IPC and its files on purpose: Emaki Settings asks `night-light state` and only watches the state file for a change, and its Quick Settings page asks `quick-settings controls`. The one program that would share a service's *data* already shares it through a crate (`emaki_phone::shared`, now `emaki_nixos::lock::Check`). |
| Would it build faster? | Not noticeably. After an edit, the shell rebuilds in 1.9 s for a surface file and 2.0 s for a service file; rustc's incremental build already recompiles only what changed, and a crate split would save a fraction of that. |
| Would it test better? | No. Its tests already run without a window, in the shell's test binary. |
| What would it cost? | About 150 items go from `pub(super)`/`pub(crate)` to `pub`, under `missing_docs`; about 200 `crate::services::` paths change; the crate would be named for the shell and used only by it. |

Splitting it now buys a compiler-checked boundary that a test already checks, and costs a public API with no second user. **Revisit when** a second program needs one of these services in-process: the vault (rbw) for a Passwords page that fills without the shell, or the launcher's file search for another app. That service then moves into a crate of its own, the way the phone and the status items did in Phase 9 of the roadmap, rather than all of `services/` at once.

**What else moved, or should:**

| Candidate | Verdict |
|---|---|
| `nixos/check.json`, parsed by both Emaki Settings and the shell | **Moved** to `emaki_nixos::lock` (shell `d617190`): one format, one path, one reader; each program wraps it in its own service. |
| The phone's two shared files (`phone-apps.json`, `phone-screen.json`) | **Moved** to `emaki_phone::shared` (Phase 6). |
| `services/vault` (2.1k lines: the rbw client, matching, the identity check) | **Stays.** Its only user is the shell; Emaki Settings' Passwords page asks the shell. It's the first candidate for a crate if a second user appears. |
| `services/launcher` (file search, clipboard history, timers) | **Stays.** Only the launcher uses it; it's a service because it runs programs and keeps state, not because it draws. |
| `tokens/layer_rules.rs` | **Stays.** It names the shell's surfaces; `cargo xtask layer-rules` runs it through the shell (roadmap Phase 10). |
| `chrome/catalog.rs`, `chrome/edit_marks.rs` | **Stay in `chrome`.** They are shell-only, and the kit's grid is pure by design. |
| `kit::platform::shell_ipc` | **Stays in the kit.** It's std-only and could sit lower, but every program that sends a shell command already uses the kit. |
| Emaki Settings' `displays/kdl.rs` | **Stays.** It's the one writer of `displays.kdl`, and only Settings writes it. |

The moves that remain are the ones in Phases 6 and 7's "not done yet".

---

## Visible changes (to check live)

- "Ago" reads one way everywhere, `5 min ago`: notification cards change from `5m ago`.
- Battery reads one way everywhere, `Full in 2 h` / `2 h left`: the widget wording changes.
- Disabled controls use one opacity, the token's value, everywhere.
- Trailing chevrons are one size and level across Quick Settings pages.
- Checkbox and radio follow the control size. They're unchanged at Medium.
- Settings text fields no longer refill themselves after being cleared.
- Services that idle-stop now actually stop when nothing shows them: weather, tailscale, night light and the others in Phase 5.

## Verification

**Every commit:** `env -u LD_LIBRARY_PATH nix develop -c sh -c 'cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace'`. Also run `cargo xtask layer-rules` (no diff), `cargo xtask lint-terms`, and `cargo xtask links`.

**Pinned behaviour:**
- IPC: the `LINES` round-trip test (4a).
- Settings search: the structural spec test (4d).
- Quick Settings: a table test checks every `ControlId` has exactly one spec (4c).
- `emaki-base`: unit tests per module, and the `ago`/`span`/`bytes` outputs are pinned.

**Visual check:** after Phases 2, 3 and 6, run the examples and apps beside the running bar, and compare against the Phase 0 screenshots. Only the visible changes listed above should differ.

**Idle check:** re-run `idle-report` after Phase 5. Wakeups and spawns should be no higher than in `docs/development/idle-baseline.md`; services that idle-stop should go quiet after 30 s unwatched.

**Metrics:** compare line counts with Phase 0. The target is −2,500 lines or more, no function over 120 lines, and no file over 650.
