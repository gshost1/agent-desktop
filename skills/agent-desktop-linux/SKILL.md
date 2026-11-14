---
name: agent-desktop-linux
tags: linux-automation, accessibility, at-spi, x11, wayland, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  Linux platform guide for the agent-desktop CLI: what differs on X11 and Wayland, the AT-SPI
  bus, app names, GTK4 and Qt behavior, and where the lifecycle, capture, clipboard, tray,
  notification and troubleshooting details are. Use for any Linux GUI task ("automate GNOME
  app", "AT-SPI tree", "accessibility bus not available").
---

# agent-desktop-linux

Only what differs on Linux; the core skill has the loop. Run the CLI inside the target apps' desktop session (Ubuntu 24.04+, GNOME measured, X11 or Wayland): the session bus and display come from its environment.

```bash
agent-desktop list-apps                                                  # names --app takes
agent-desktop wait --window "Text Editor" --app gnome-text-editor --timeout 10000
agent-desktop snapshot --app gnome-text-editor -i --compact --lines
```

- **`status` first when nothing shows.** An empty `supported_surfaces` means the accessibility bus did not answer, not an empty desktop: open `bus-and-permissions`.
- **`--app` takes the name `list-apps` prints** (`gnome-text-editor`, `nautilus`) or a desktop-entry name such as `Files`, never a window title.
- **A window appears before its content.** `wait --window`, then `wait --text`, which matches a node's own name, value or description.
- **GTK4 nests deeply.** `-i --compact` still shows nameless `group` nodes: read past them. `--max-depth` and `--skeleton` count semantic levels.
- **A ref must have been unique in its window.** Identical controls (same role, name and description) refuse with `AMBIGUOUS_TARGET`, `identity_not_unique`, before anything is sent. Act on a unique control, or on X11 use `--headed mouse-click --xy`.
- **Headless `type` writes through the text interface**, so it works without focus; a terminal accepts keys only under `--headed`. A GTK4 entry has no headless `click` activation: use `type`.
- **Two `click`s on one GTK4 button within about 250 ms act once.** Wait for the effect between repeated clicks.
- **Pointer input is X11 and `--headed` only**: `double-click`, `triple-click`, `hover`, `drag`, `scroll`, `mouse-*`. Wayland refuses them with `ACTION_NOT_SUPPORTED`. `press` works on X11 and Mutter Wayland; window ops and `focus-window` need X11. Under `--headed`, ref `click`, `right-click` and `type` try physical input first.
- **Keys:** `cmd` and `meta` are Super; X keysym names work. Blocked combos refuse without `--force`.
- **Menus:** after opening one, read `data.surfaces`, then `snapshot --surface menu --app <name> -i`. A list-backed dropdown is part of the window, not a menu.
- **Chromium and Qt apps need a launch flag** to appear on the bus; open `troubleshooting`.
- **The state root must be private.** A group- or world-accessible `~/.agent-desktop` makes `snapshot` fail with `INTERNAL`: `chmod 700` it.

## Topics

`agent-desktop skills get linux <topic>`:

| Topic | Open it for |
|-------|-------------|
| `capabilities` | Per command group, what works on X11 and Wayland; `select`, `focus` and toolkit limits |
| `bus-and-permissions` | Bus states, `status` and `permissions` output, state-root rules |
| `display-servers-and-input` | `press`, `type`, pointer, wheel, drag, blocked combos, key names |
| `windows-and-lifecycle` | `launch`, `close-app`, window ops, waits for a window or close, workspaces |
| `capture-and-clipboard` | `screenshot` and the Wayland consent dialog, clipboard |
| `shell-tray-and-notifications` | Menus and popovers, GNOME shell, tray, notifications, cursor overlay |
| `troubleshooting` | Symptom to cause, Chromium, Electron and Qt flags, error codes |
