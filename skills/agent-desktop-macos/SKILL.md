---
name: agent-desktop-macos
requirements:
  - agent-desktop
description: >
  macOS platform guide for the agent-desktop CLI: Accessibility and Screen Recording permission,
  which actions need --headed, menus and surfaces, and fixing PERM_DENIED, empty trees,
  ACTION_FAILED, POLICY_DENIED and APP_UNRESPONSIVE on macOS.
---

# agent-desktop on macOS

What differs on macOS from the core skill (`agent-desktop skills get desktop`).

## Permissions

The app that launches agent-desktop (Terminal, iTerm2, Warp, VS Code, Codex) needs Accessibility permission; `screenshot` also needs Screen Recording. Run `agent-desktop permissions`. If one is denied, run `agent-desktop permissions --guide`: it opens System Settings with a panel that shows the user which app to add, and returns at once. A new grant applies from the next call, with no restart.

## Headless and headed

- Ref actions are semantic by default; a headless `right-click` can use `AXShowMenu`. Under `--headed`, `click`, `right-click`, `type`, `clear` and `scroll` go physical first, except a click on a native menu-bar item, which uses `AXPress` without moving the pointer. A headed pointer action on a window's element focuses that window and checks the hit target first. `set-value`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `focus` and `scroll-to` stay semantic.
- Headless `type` inserts through `AXSelectedText`. A field where that is not settable returns `POLICY_DENIED`: use `set-value`, or `--headed` when the user allows it.
- `double-click`, `triple-click`, `hover` and `drag` need `--headed`, and return `POLICY_DENIED` without it.
- `press` sends keys to the app's process without focusing it: `press cmd+s --app "TextEdit"`. Quit combos such as `cmd+q` return `POLICY_DENIED` unless you pass `--force`; quit with `close-app`.
- An AX success code does not prove an effect: `delivered_unverified`, such as an `AXPress` with no postcondition, can be `ok: true` with no change. Check that control with `get`, `is`, `find` or `--root`, not a full snapshot.

## Menus and apps

- An app's File and Edit menus are `--surface menubar`. Status items are `--surface menu-bar-extras`; the Dock is `--surface dock`.
- After a `right-click` that returns `APP_UNRESPONSIVE` with `retry: unsafe`, don't retry: snapshot `--surface menu`.
- For combo boxes and pop-up buttons, use `select @ref "Option"` with no menu already open.
- `launch` takes a display name (`"System Settings"`) or a bundle ID. A menu-bar app with no window is readable with `snapshot --app "Control Center"`.
- A ref keeps the element's native address when the app provides one, so identical twins, such as several `Delete` buttons, each act on their own row.

## References

| Reference | Open it when |
|-----------|--------------|
| `platform-details` | Full permission steps, surfaces, Notification Center, Chromium and Electron, identity, keys, cursor overlay |
| `troubleshooting` | A command fails: find the symptom |

Load one with `agent-desktop skills get macos <reference>`.
