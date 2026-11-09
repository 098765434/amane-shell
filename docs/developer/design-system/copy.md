# Copy

How emaki's settings and panels word things. Emaki Settings follows it page for page; the
shell's panels should too.

## Case

- **Row titles, group captions and sub-page titles** are sentence case: *Saved networks*,
  *Keyboard shortcuts*, *Settings files*.
- **Buttons** are Title Case: **Show Changes**, **Try Again**, **Move to Settings…**.
- **Names of emaki's own parts** keep their capitals wherever they appear: Status Bar, Quick
  Settings, Night Light, Now Playing, Activities, Emaki Settings. So does a button named in
  text: *Installs on Apply*.
- A row that opens a sub-page and the sub-page's title are the same words.

## Ellipsis

A button ends in **…** exactly when it opens a dialog or another window before anything happens:
**Reset…** asks first, **Reset** doesn't. A button whose action can be undone from a toast
doesn't ask, and so has no ellipsis.

## One verb per action

| Verb | For |
|---|---|
| **Remove** | taking something out of a list: a quick key, a package added in Settings, a VPN |
| **Forget** | a saved secret or a Wi-Fi network: it goes, and comes back by being given again |
| **Uninstall** | a Flatpak |
| **Reset** | handing a setting back to the system's (or niri's) choice |
| **Undo Add**, **Undo Remove** | taking back a change that waits for Apply |

## Plain words

The System pages say what a person would, and keep Nix's words for where they're the point:

| Instead of | Say |
|---|---|
| generation | version (*Version 79*, *Go back to version 12?*) |
| nixpkgs | packages, the package set |
| inputs | components |
| ∅ | none |
| *Applying failed* | *Couldn't apply* (an outcome, not a stage) |

Commit hashes and attribute paths stay on the Configuration folder sub-page and in a package's
details, as property rows.

## States

Each is said one way, through Emaki Settings' helpers in `apps/settings/src/main.rs`:

- **Loading:** `loading_row`, a spinner in place of the value.
- **Not answering:** `unreachable_page` for a whole page, `unreachable_row` inside one, each with
  **Try Again**.
- **Something went wrong:** `problem_row` (a warning tile, what it means, **Details…** for what the
  tool said), or `dismissible_problem_row` with **OK** for a change that didn't take.
- **A field that can't be kept:** the field turns red (`field_box_in` with `FieldState::Error`)
  and its row says why.
- **Nothing yet:** `empty_row` inside a group, `status_page` when the list is the whole page.
- **Done, with nothing on the page to show it:** a toast (`emaki_kit::toast`), with **Undo** when
  it can be taken back.

## Notes

A group's note is one sentence. Anything longer goes in a `details_row` (**Details…**) or the
dialog an info button opens.
