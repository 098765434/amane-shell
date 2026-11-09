# Penpot tooling for Emaki, with Claude Code

This page covers research and ideas, as of 2026-11-09, for making Emaki's Penpot file ("New File 5") easier to build and keep in step with the code. The work happens through Claude Code. What's still to draw is in [penpot-todo.md](penpot-todo.md); token debt is in [penpot-backlog.md](penpot-backlog.md).

## How it works today

Claude Code drives Penpot through the **Penpot MCP server**, configured in `~/.claude.json` as `penpot` at `https://penpot-mcp.tail8c36e9.ts.net/mcp`. The server talks to a plugin open in the Penpot tab. Its main tool is `execute_code`, which runs JavaScript against the [Plugin API](https://penpot-plugins-api-doc.pages.dev/). A `storage` object keeps values between calls ([Penpot MCP docs](https://help.penpot.app/mcp/)).

Each surface took one long session: dock, bar, Quick Settings, Settings, launcher, OSD, notifications, widgets and Messages. Most of the time went to work that repeated across sessions.

### What cost the most time

**1. Rebuilding helpers.** Every session rewrote the same helpers:
- `box`, `flex`, `text`, `tok`, `glyph`, `path`, `makeSet`, `card`, `frost` and `scene`.
- The bubble outline and the clock digits.

`storage` is wiped when the plugin reconnects, and sometimes on a page switch. Functions in `storage` keep their closures, but there is no file to reload them from.

**2. Penpot quirks, found again each time:**

| Quirk | Workaround |
|---|---|
| Text bounds update after the call ends. A width or height read in the same call is stale. | Build text that depends on its own size in phases: create in one call, measure in the next, place in a third. Messages' bubbles needed `phaseA/B/C`. |
| Applying a typography token resets a raw font size or weight set before it, such as the 600 figures or 0.75× rates. | Re-apply the size and weight after a wait. |
| `resize()` on a text sets `growType` to `fixed`. | Set `growType` back afterwards. |
| The text alignment property is `align`, not `textAlign`. | Use `align`. |
| Strokes have no join setting, so corners come out mitred. | Split paths into segments with round caps. |
| `createBoolean` fails on shapes inside a flex layout. | Build the boolean outside the layout, then move it in as an absolute child. |
| Flex children order by layer order, and `setParentIndex` or `sendToBack` behave unexpectedly. | Check the order after any change. |
| Main components can only be edited while their page is active. | Switch to the page first. Instances need a nudge to relayout. |
| `penpot.openPage()` inside a long call can leave new shapes on the wrong page. | The widgets' "All sizes" board landed on the components page. Check the page after creating shapes. |
| Instances don't pick up `backgroundBlur` set on their main component. | Set it on each instance too. |
| The exporter renders neither background blur nor some glyphs, and a light-mode export once dropped some text. | Exports can't fully verify a board. |

**3. Getting data in.** The plugin's `fetch` to a local URL is blocked, and `uploadMediaUrl` refuses private addresses (`ssrf-blocked-target`). So the wallpaper had to be sent as base64 in chunks through tool calls, about 80 KB of tokens.

**4. Keeping tokens in step.** New tokens are typed twice: once in Penpot (`addToken`), once in `tokens.json`, then run through `cargo xtask tokens normalize` and `check`. The two can drift. `widget.*`, for example, was in both, but the backlog said otherwise.

**5. Checking against the code.** Each session re-derived measurements from the Rust source, through an Explore agent or by reading it. Then a token audit, theme toggles and exports, all done by hand.

## What Penpot offers

- **Plugins.** A plugin is a `manifest.json` plus a code file and a UI iframe. It asks for permissions, such as `content:read/write`, `library:read/write`, `page:read` and `file:read`. It is installed by the manifest's URL; `Ctrl+Alt+P` opens the manager. Typings are in `@penpot/plugin-types` ([create a plugin](https://help.penpot.app/plugins/create-a-plugin/), [getting started](https://help.penpot.app/plugins/getting-started/)). It's the same API `execute_code` uses.
- **The RPC API.** It takes `POST <host>/api/main/methods/<command>`. Older versions use `/api/rpc/command/<command>` instead. Commands include `get-file`, `update-file` and `create-file-media-object`, among others. Access needs a personal access token, which the instance must enable with the `access-tokens` flag. Each instance documents its own API at `/api/_doc` ([community example](https://community.penpot.app/t/api-example-for-modifing-a-text-component/8297), [Cloudron note on the path change](https://forum.cloudron.io/topic/9406/penpot-package-updates/50?page=1)).
  - `update-file` takes low-level change lists with revision numbers. Reading is easy; writing is fragile.
  - Uploading media from a script works, because the script sends the bytes itself. That would have avoided the base64 detour.
- **Design tokens.** Tokens can be imported and exported as JSON from the Tokens panel ([guide](https://penpot.app/blog/a-practical-guide-to-the-design-tokens-json-format/)). The export is Tokens Studio's multi-set shape, not strict DTCG ([thread](https://community.penpot.app/t/token-export-is-tokens-studio-format-not-dtcg-2025-10/10544)). That's the shape `tokens.json` already has.
  - GitHub sync is announced but not confirmed shipped ([what's next](https://community.penpot.app/t/what-s-next-for-penpot-design-tokens/8789)).
  - [penpot-export](https://awesome.ecosyste.ms/projects/github.com%2Fpenpot%2Fpenpot-export) is a CLI that pulls tokens from a file.
- **Background blur.** Added in 2.17. It renders in the WebGL canvas ([release notes](https://penpot.app/dev-diaries.html), [news](https://alternativeto.net/news/2026/7/design-tool-penpot-brings-background-blur-better-webgl-rendering-and-design-token-upgrades/)). Exports here don't show it.
- **The MCP server's REPL.** According to a [security advisory](https://advisories.gitlab.com/pkg/npm/@penpot/mcp/), `@penpot/mcp` runs a REPL with an unauthenticated `/execute` endpoint on port 4403. It's a risk if exposed, but it's also a way to run a script from a file without sending it through the conversation.
  - Ours runs on a tailnet host. Whether that port is reachable there, and whether it should be, is still to check. It may need a token or a firewall rule in front of it.

## Ideas, by value for effort

### 1. A Claude Code skill for Penpot work (done)

Done: [`.claude/skills/penpot/`](../../../.claude/skills/penpot/SKILL.md). It contains:
- the workflow;
- the quirk list, in `references/quirks.md`;
- the helpers as a file, `scripts/helpers.js`, sent once per session (the read-it-once fallback of idea 2).

The plan, as written before it was built:

Add `.claude/skills/penpot/SKILL.md` to the repo. Claude Code loads it when a task touches Penpot. It holds what each session had to relearn:
- **The file's conventions:**
  - Pages are named "X — Components" and "X".
  - Boards are 1440×900 scenes with the `wallpaper` component and the Compact Frosted bar.
  - Components follow the `Area / Name` naming with variant properties.
  - Glass is background blur on the surface itself, not a separate layer.
- **Token rules:**
  - Bind colours, sizes and type.
  - Component tokens can't reference other component tokens (`tokens check` rejects it).
  - Add tokens in Penpot first, then copy them into `tokens.json`.
  - Raise the tier ceilings in `lint.rs` to match.
  - Update the docs.
- **The quirk list above, each with its workaround.**
- **How to verify:** measure against the code, run the token audit, check light and compact, put the themes back, and export (knowing what the exporter can't show).
- **What not to do:**
  - Commit only when asked.
  - Never credit MacTahoe.
  - No segmented controls.

A skill costs nothing until it's used, and it stops sessions from starting over. Short `.claude/commands/` such as `/penpot-audit` or `/penpot-check-themes` can wrap the routine checks.

### 2. A helper library in the repo: `tools/penpot/lib.js`

Put the helpers in one versioned file. It would cover:
- token helpers;
- text with the size and weight fix built in;
- `makeSet`, scene and bar;
- phased text measuring;
- a page-safe `onPage(name, fn)`;
- path tools (transforming, splitting into round-capped segments);
- the token audit;
- `measure(boardId)`, which returns a board's layout as JSON.

There are three ways to get it into Penpot, cheapest first:
- **Through the REPL endpoint:** a small script (`tools/penpot/run.mjs <file>`, or `cargo xtask penpot run`) posts the file to the MCP server's `/execute`. One Bash call loads the whole library into `storage`, and its source never enters the conversation. This needs the endpoint to be reachable and safe (see above).
- **As an Emaki plugin** (idea 3) that puts the library on the plugin's global. Once installed, nothing needs loading. But the MCP plugin and ours are separate plugins and may not share globals, which needs checking.
- **As a fallback:** Claude Code reads the file and passes it to `execute_code` once per session. It costs tokens, but far fewer than rewriting it.

### 3. An "Emaki" Penpot plugin (in-repo, served from the devshell)

Put the plugin in `tools/penpot/plugin/` (TypeScript plus `@penpot/plugin-types`, built with Vite), served by `nix develop -c …`. It's useful by hand, and Claude Code can drive the same functions. Its panel would have:
- **Token sync:** show what differs between Penpot's sets and the repo's `tokens.json`, as added, changed and removed per set. Penpot data can only reach the repo by export, so the panel offers "Copy as tokens.json" or "Download". The repo side then runs `normalize` and `check`. The other direction is "Apply from tokens.json" (paste or file), which writes the changes into Penpot.
- **Audit:**
  - unbound fills, strokes and text;
  - raw font sizes;
  - shapes on the wrong page;
  - instances missing their main's background blur;
  - variant sets with gaps;
  - text 1 px tall (the collapsed-text bug).

  It lists each finding with a jump-to.
- **Theme preview:** switch light/dark and comfortable/compact from the panel, and always restore dark / comfortable.
- **Scene tools:** new 1440×900 scene with the wallpaper and the bar; "place on the widget grid"; "put background blur on the selection and its instances".
- **Wallpaper:** pick an image file in the plugin UI and upload it with `uploadMediaData`. The image goes straight from disk, with no base64 through Claude.

### 4. A spec from the code: `cargo xtask penpot spec`

Most checking means re-deriving numbers from Rust. Instead, let the code say what it draws:
- **What the command writes:** `target/penpot-spec.json`, with entries like:
  - the widget card sizes per span;
  - the bubble's `measures_like` outputs;
  - the OSD and calendar heights;
  - the menu row height;
  - the window and sidebar sizes;
  - …

  Where tests already measure layouts (`dialog::measures_like_the_design`, the card height tests), they can share the same functions.
- **The check in Penpot:** `lib.measure()` (or the plugin) compares the named boards against the spec and reports drift, such as "Widget / Card Small is 166, spec 164".
- **The CI side:** `cargo xtask penpot spec --check` fails if the spec changes without the Penpot snapshot being updated. That snapshot is a small `penpot-measures.json` committed after each Penpot change.

It replaces the Explore-agent step and catches drift both ways.

### 5. A read-only RPC client for checks without the browser

Add `cargo xtask penpot pull`, reading the URL and an access token from the environment. It fetches the file with `get-file` and writes:
- the token sets, as `tokens.json` for `normalize` and `check`;
- a page and board index;
- the measures for idea 4.

It needs no open Penpot tab, so it can run in CI or a hook. Keep it read-only: writes through `update-file` should go through the plugin API.

It needs the instance's `access-tokens` flag and a token kept outside the repo (agenix or an env file).

### 6. Claude Code hooks

- **On edits:** a `PostToolUse` hook on edits to `crates/kit/design-tokens/tokens.json` runs `cargo xtask tokens check`. One on `apps/**` runs `penpot spec --check` once idea 4 exists.
- **On stopping:** a `Stop` hook reminds Claude to put the Penpot themes back to dark / comfortable if a session changed them. It could check via the REPL endpoint.

### 7. Comparing with the running shell

Add `tools/penpot/compare.sh <surface>`:
1. Open the surface in the running shell, for example `emaki ipc widgets edit` or `notify-send`.
2. Capture it with `grim`.
3. Export the matching board through the REPL endpoint, or fetch it with the RPC exporter.
4. Write a side-by-side image or diff to `target/compare/`.

Glass won't match, because the exporter has no background blur. Layout, type and colour will.

## Suggested order

1. **The skill** (idea 1), plus the quirk list moved out of this page into it. This takes an hour and pays off at the next session.
2. **`tools/penpot/lib.js`**, loaded the cheapest way available:
   - first, check whether the REPL endpoint can be reached safely;
   - otherwise, the read-it-once fallback.
3. **`penpot spec`** (idea 4), starting with the widgets, Messages and the OSD, where the numbers are densest.
4. **The plugin** (idea 3), starting with the token diff, the audit and the wallpaper upload.
5. **The RPC pull** (idea 5) and the CI check, once access tokens are enabled on the instance.

## Open questions

- Is the MCP server's `/execute` port (4403) reachable on the tailnet host, and can it be limited to this machine?
- Do two plugins (the MCP one and an Emaki one) share globals, or must the library load into the MCP plugin's `storage`?
- Background blur:
  - Does it render on the canvas in the instance's Penpot version (2.17+ with WebGL)?
  - Does the instance's version export it at all?
- Does the instance's token export match `tokens.json` closely enough for a plain diff? It might need `tokens normalize` on both sides first.
