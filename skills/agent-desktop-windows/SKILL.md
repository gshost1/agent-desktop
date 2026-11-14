---
name: agent-desktop-windows
tags: windows-automation, accessibility, uia, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  Windows platform guide for the agent-desktop CLI: what works and what refuses per command
  group, PowerShell ref quoting, minimized windows, headed-only input, and where the shell
  surface, notification, elevation, Chromium and troubleshooting details are. Use for any
  Windows GUI task ("automate Windows app", "UIA tree", "elevated window input blocked").
---

# agent-desktop-windows

Only what differs on Windows; the core skill has the loop. Run it in the interactive desktop session of the signed-in user (Windows 10 1809+ or Server 2019+); Session 0, the secure desktop and a locked desktop are not supported.

## Capability Table

| Group | Commands | Status on Windows |
|-------|----------|-------------------|
| Observation | `snapshot`, `find`, `get`, `is`, `screenshot` | Works. Surfaces are `window`, `focused`, a Chromium modal as `sheet`, and an open application menu as `menu` |
| Surfaces | `list-surfaces` | Works. Lists `window`, `focused` and `sheet` per process, plus `menu` with `item_count` while a menu is open |
| Shell surfaces | `open-system-surface` | Works for `start-menu`, `taskbar`, `system-tray`, `system-tray-overflow` and `action-center`, and returns the window `snapshot --surface <kind>` reads. `quick-settings` returns `INVALID_ARGS` naming `action-center` |
| Ref interaction | `click`, `right-click`, `type`, `set-value`, `clear`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `scroll`, `scroll-to` | Works, semantic in headless and `--headed` mode. `right-click` and `type` need `--headed` and return `POLICY_DENIED` headless |
| Multi-click and focus | `double-click`, `triple-click`, `focus` | Works with global `--headed` only |
| Keyboard and mouse | `press`, `hover`, `drag`, `mouse-click`, `mouse-move`, `mouse-wheel` | Works. Cursor-moving input needs `--headed`. First Contact covers `press --app` |
| Held input | `key-down`, `key-up`, `mouse-down`, `mouse-up` | Fails closed on every platform with `ACTION_NOT_SUPPORTED`. A headless `mouse-down` or `mouse-up` returns `POLICY_DENIED` first |
| App and window | `launch`, `close-app`, `list-windows`, `list-apps`, `focus-window`, `resize-window`, `move-window`, `minimize`, `maximize`, `restore` | Works. First Contact covers the name `launch` takes |
| Displays | `list-displays` | Works |
| Clipboard | `clipboard-get`, `clipboard-set`, `clipboard-clear` | Works; typed text and image content |
| Wait | `wait` | Works for ms, `--element`, `--window`, `--text`, `--menu`, `--menu-closed`, `--event` and `--notification` |
| Notifications | `list-notifications`, `dismiss-notification`, `dismiss-all-notifications`, `notification-action` | Works over the Action Center. Pass `--headed` when the center is closed |
| Cursor overlay | `cursor-overlay` | Works. `enable` returns `data.rendered: true` only after the renderer acknowledges over its pipe. See `references/shell-and-overlay.md` |
| System | `status`, `permissions`, `version`, `batch`, `skills`, `session`, `trace` | Works |

## First Contact

- **Quote every ref in PowerShell.** A bare `@token` is PowerShell's splatting operator, so `set-value @s8f3k2p9:e1 hi` fails `INVALID_ARGS`. Write `set-value '@s8f3k2p9:e1' hi`.
- **A minimized window is not drivable**; one behind another window is. Run `restore --app <image>` and snapshot again. `focus-window` steals the user's foreground.
- **`type` and `right-click` need `--headed`**; use `set-value` to write a field headless.
- **`press --app` writes to the foreground queue**: a headless press to an app that is not frontmost fails closed.
- **`launch` takes an absolute path or a bare System32 name** (`notepad`), not a display name.
- **Chromium and Electron apps read thin before they settle**: raise `--timeout-ms`.
- **Blocked shortcuts** (`alt+f4`, `win+l`, `alt+tab` and others) refuse without `--force`. `cmd` and `meta` are the Windows key: write `ctrl+c` to copy.
- **Identical controls** refuse with `AMBIGUOUS_TARGET`, `identity_not_unique`; use `--headed mouse-click --xy` to pick one.

## Topics

`agent-desktop skills get windows <topic>`:

| Topic | Open it for |
|-------|-------------|
| `first-contact` | Every first-contact note in full: `type` verification, `launch` exits, notifications, stateful readback |
| `shell-and-overlay` | `open-system-surface`, tray and taskbar, notifications, cursor overlay, menu detection |
| `permissions-and-elevation` | Integrity levels, UIPI, blocked combos, protected processes, the interaction lease |
| `chromium-and-electron` | Settle timing, covered windows, WPF provider trap, unnamed web content |
| `troubleshooting` | Symptom to cause, action verification outcomes, saving headless, install warnings |
