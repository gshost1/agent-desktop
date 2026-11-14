---
name: agent-desktop
tags: computer-use, desktop-automation, accessibility, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  Operate desktop apps on macOS, Windows and Linux through their accessibility trees with the
  agent-desktop CLI: read UI state, click, type, fill forms, navigate menus, press shortcuts,
  manage apps and windows. Use for any desktop GUI task ("click button", "open app", "computer
  use", "snapshot app", "desktop automation").
---

# agent-desktop

A CLI that reads and drives desktop apps through the OS accessibility tree. Every command prints one JSON envelope: `ok`, then `data` or `error`. You run the observe and act loop. Next, read `agent-desktop skills get platform`: what differs on this OS. Flags: `agent-desktop <command> --help`.

## The loop

```bash
agent-desktop snapshot --app "Font Book" -i --compact --lines     # one node per line: ref, role, "name"
agent-desktop find --app "Font Book" --role button --name "Hide Sidebar"   # ref in .data.matches[0].ref_id
agent-desktop click @s8f3k2p9:e12                                 # act on a ref as printed
agent-desktop type @s8f3k2p9:e4 "hello"                           # set-value replaces, clear empties
agent-desktop press ctrl+s --app "Font Book"                      # key combo to the app
agent-desktop wait --app "Font Book" --text "Saved" --timeout 5000
```

- `--app` takes the name `list-apps` prints. Take the ref from the start of the line that names the control.
- Big tree: `snapshot --skeleton` maps it, `snapshot --root <ref>` drills one region. Filter output with `grep` or `jq`; never cut it with `head`.
- Refs (`@<snapshot_id>:e<N>`) belong to their snapshot. After the UI changes, snapshot again before you use refs inside the changed part.
- An action's result proves its delivery: `data.disposition.delivery` is `delivered_verified` (effect read back) or `delivered_unverified`. Do not snapshot just to confirm it. Snapshot when the next step needs new refs or text; `-w "role:text"` on the action waits for that text and returns the snapshot in the same call.
- A menu, sheet or alert the action opened is listed in `data.surfaces`: read it with `snapshot --surface menu` (or `sheet`, `alert`).
- Chain dependent commands in one shell call with `&&`. Wait with `wait`, never a fixed `sleep`.

## Errors

Branch on `error.code`. Retry or substitute an action only when `disposition.retry` is `safe`; otherwise it may have landed, so read the state first.

- `STALE_REF`, `SNAPSHOT_NOT_FOUND`: snapshot again and use the new ref.
- `AMBIGUOUS_TARGET`: identical controls; act on one that is unique in the window.
- `POLICY_DENIED`, `ACTION_NOT_SUPPORTED`: no headless path. Ref actions are headless (no focus, cursor or keys); the global `--headed` flag allows physical input. Use it only when the user permits that, never as a retry.

## Topics

Load one when you need it: `agent-desktop skills get desktop <topic>`.

| Topic | Open it for |
|-------|-------------|
| `refs-and-retry` | Every ref error kind, delivery and retry rules, headed mode, drilling rules |
| `round-trips` | Shell quoting (zsh, PowerShell) and find, click and wait chains in one call |
| `commands-observation` | `snapshot`, `find`, `get`, `is`, `screenshot`, `list-surfaces`, partial trees |
| `commands-interaction` | Choosing an action, `-w`, keys, mouse, action errors |
| `commands-system` | Launch and close, windows, clipboard, `wait`, `batch`, sessions, tracing |
| `workflows` | Forms, menus, dialogs, new windows, scroll-find, drag, anti-patterns |
