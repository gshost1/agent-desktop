---
name: agent-desktop
requirements:
  - agent-desktop
description: >
  Operate desktop apps on macOS, Windows and Linux through native accessibility trees with the
  agent-desktop CLI: read UI state, click, fill forms, pick menu items, press keys, manage
  windows. Use for any desktop GUI task or "computer use" request.
---

# agent-desktop

A CLI that reads and drives desktop apps through the OS accessibility tree. Every command prints one JSON envelope (`ok`, `data` or `error`). You run the loop.

```bash
agent-desktop launch Notes --activate
agent-desktop snapshot --app Notes -i --compact
agent-desktop snapshot --app Notes --surface sheet -i --compact  # or menu, menubar, alert, focused
agent-desktop snapshot --app Notes --skeleton -i --compact  # dense app: shallow map
agent-desktop snapshot --root @s8f3k2p9:e3 -i --compact  # then drill one region
agent-desktop find --app Notes --role button --name "Save" --first
agent-desktop find --app Notes --surface menubar --text "Format"
agent-desktop click @s8f3k2p9:e5
agent-desktop set-value @s8f3k2p9:e2 "hello"
agent-desktop select @s8f3k2p9:e7 "Large"
agent-desktop type @s8f3k2p9:e2 " more"
agent-desktop press cmd+s --app Notes  # ctrl+s off macOS
agent-desktop click @s8f3k2p9:e5 --wait-for "button:Replace"  # act, then wait for role:text
agent-desktop wait --element @s8f3k2p9:e9
agent-desktop wait --window "Save" --app Notes
agent-desktop wait --text "Saved" --app Notes
agent-desktop wait --menu --app Notes
agent-desktop batch '[{"command":"click","args":{"ref_id":"@s8f3k2p9:e5"}},{"command":"press","args":{"combo":"return"}}]' --stop-on-error
agent-desktop list-surfaces --app Notes
agent-desktop list-windows --app Notes
```

## The loop

1. Locate with `find` when you know the role or name, else `snapshot -i --compact`.
2. Act on a ref from that output.
3. Read the result. A menu, sheet or alert it opened is in `data.surfaces`: snapshot that surface next.
4. Wait with `wait` or `--wait-for`, never a fixed sleep.

Chain known steps in one Bash call or one `batch`. Don't re-snapshot to confirm an `ok: true` action; check `data.surfaces`.

## Refs, retry, headless

Refs are qualified (`@s8f3k2p9:e1`); pass them as printed. A ref is valid until the UI changes. `STALE_REF` means snapshot again.

Retry only when `disposition.retry` is `safe`. Otherwise the action may have landed: read the live state first.

Ref actions are headless: no focus, cursor or clipboard. Add the global `--headed` only when the user allows physical input, never to recover from a headless failure.

## Load more only when needed

Most tasks need neither. Run `agent-desktop skills get <name>`.

| Load | When |
|------|------|
| `platform` | `PERM_DENIED`, an empty tree, an OS-specific error |
| `desktop core-rules` | Twins, `AMBIGUOUS_TARGET`, unsafe retry, PowerShell |
| `desktop commands-observation` | Snapshot, find, get, surface details |
| `desktop commands-interaction` | Choosing an action, an action error |
| `desktop commands-system` | Launch, windows, clipboard, wait, batch, sessions |
| `desktop workflows` | Worked patterns: forms, menus, dialogs, drag |
