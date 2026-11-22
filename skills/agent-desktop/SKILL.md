---
name: agent-desktop
tags: computer-use, desktop-automation, accessibility, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  Operate desktop apps on macOS, Windows and Linux through their accessibility trees with the
  agent-desktop CLI: read UI state, click, type, fill forms, use menus and shortcuts. Use for any
  desktop GUI task ("click button", "open app", "computer use", "desktop automation").
---

# agent-desktop

Reads and drives desktop apps through the accessibility tree. Every command prints one JSON envelope: `ok`, then `data` or `error`. Also run `agent-desktop skills get platform`: it is short and says what differs on this OS. Flags: `agent-desktop <command> --help`. Write flags out in full, never in a shell variable (`A="--app X"; agent-desktop press $A` does not split in zsh), and never discard stderr (`2>/dev/null`).

## Topics

`agent-desktop skills get desktop <topic>`:

| Topic | Open it for |
|-------|-------------|
| `refs-and-retry` | Every ref error kind, delivery and retry, headed mode, drilling |
| `round-trips` | Shell quoting (zsh, PowerShell), find, click and wait chains in one call |
| `commands-observation` | `snapshot`, `find`, `get`, `is`, `screenshot`, `list-surfaces`, partial trees |
| `commands-interaction` | Choosing an action, `-w`, keys, mouse, action errors |
| `commands-system` | Launch and close, windows, clipboard, `wait`, `batch`, sessions, tracing |
| `workflows` | Forms, menus, dialogs, new windows, scroll-find, drag, anti-patterns |

## The loop

```bash
agent-desktop find --app "Font Book" --role button --name "Hide Sidebar"   # ref in .data.matches[0].ref_id
agent-desktop click @s8f3k2p9:e12                                 # act on a ref as printed
agent-desktop snapshot --app "Font Book" -i --compact --lines     # one node per line: ref, role, "name"
agent-desktop set-value @s8f3k2p9:e4 "hello"                      # replaces the text; type adds keys
agent-desktop press ctrl+s --app "Font Book"                      # key combo to the app
agent-desktop wait --app "Font Book" --text "Saved" --timeout 5000
```

- `--app` takes the name `list-apps` prints.
- A control you can name: `find` it. Snapshot only to learn what is there, and once: a large app (an office suite, a browser) first gets `snapshot --skeleton`, then `snapshot --root <ref>` on one region. `--wait-for` on a skeleton returns the full tree down to a deep match. A line ending `children_count=N` is a cut branch; `--root` on its ref reads it.
- Roles are tokens such as `button`, `textfield` (single and multi-line text), `checkbox`, `switch`, `combobox`, `menuitem`, `tab`, `cell`.
- Refs (`@<snapshot_id>:e<N>`) belong to their snapshot; take one from the start of the line that names the control. After the UI changes, snapshot again before you use refs in the changed part.
- An action's result says what it proved in `data.disposition.delivery`: `delivered_verified` read the effect back; `delivered_unverified` did not read the effect, so when the task depends on it, read it back with `get` or `-w`. Otherwise do not snapshot just to confirm it. Snapshot when the next step needs new refs or text; `-w "role:text"` on the action waits for that text and returns the snapshot in the same call.
- A menu, sheet or alert the action opened is in `data.surfaces`: read it with `snapshot --surface menu` (or `sheet`, `alert`).
- Wait with `wait`, never a fixed `sleep`.
- A command chained immediately after a key that opens a dialog may still see the parent window; re-snapshot or use `wait --window "<dialog title>" --app "<app>" --timeout 5000` before acting on the dialog.

## Reading output in a shell

- Chain steps with `&&`, never `;`: a refused click must stop the save or submit after it.
- When a command fails, agent-desktop also prints one line on stderr, `agent-desktop: <CODE>: <message> -- <suggestion>`, so a pipe into jq, grep or head cannot hide the error.
- A `jq` path over `data` prints nothing useful for an error envelope. Keep the error visible: `jq -r '.data.lines[]? // .error'`.
- Lines are JSON strings, so a name's quotes print as `\"`. `grep -i subscript`, not `grep 'switch "Subscript"'`.
- Filter with `grep` or `jq`; never cut a snapshot with `head`.

## Errors

Branch on `error.code`. Retry or substitute an action only when `disposition.retry` is `safe`; otherwise it may have landed, so read the state first.

- `STALE_REF`, `SNAPSHOT_NOT_FOUND`: snapshot again and use the new ref. Another `details.kind` or `details.reason`, such as `capture_incomplete` (retake with a larger `--timeout-ms`), is in `skills get desktop refs-and-retry`.
- `APP_NOT_FOUND`: if `error.details.candidates` includes the intended app, retry with its exact name from `error.suggestion`. A near match is never used to send input.
- `AMBIGUOUS_TARGET`: identical controls; act on one that is unique in the window.
- `POLICY_DENIED`, `ACTION_NOT_SUPPORTED`: no headless path. Ref actions are headless (no focus, cursor or keys); global `--headed` allows physical input, only when the user permits it, never as a retry.
