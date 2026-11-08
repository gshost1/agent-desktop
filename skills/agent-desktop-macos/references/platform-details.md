# macOS Platform Details

The detail behind the macOS skill's short rules: requirements, first contact with permissions, and every way macOS differs from the core skill. The macOS skill's lines still apply; this reference adds what they leave out. When a command fails, find the symptom in the `troubleshooting` reference.

## Requirements

- macOS 13 or later, Intel or Apple Silicon.
- Accessibility permission for the app that launches agent-desktop (Terminal, iTerm2, Warp, VS Code, Codex). macOS 27 and later name this list Device Control and Data Access. If System Settings lists the `agent-desktop` binary separately, grant it too.
- Screen Recording permission, only for `screenshot` and for `session start --screenshots` artifacts.
- Automation permission for System Events, only where a command reports it missing.

## First contact

1. Run `agent-desktop permissions`.
2. If a permission is denied, run `agent-desktop permissions --guide`. It opens the right System Settings page with a panel that shows the user which app to drag into the list. The user confirms with Touch ID or a password, and the panel moves to the next missing permission by itself. The command returns at once with `"guide": "started"`, `"already_running"` or `"not_needed"`. `agent-desktop permissions --request` shows the system prompts instead.
3. If the guide cannot run, open System Settings > Privacy & Security > Accessibility (Device Control and Data Access on macOS 27 and later). Add the launching app and turn it on.
4. Run `agent-desktop permissions` to confirm `granted`. The next agent-desktop call sees a new grant, so the launching app does not need a restart. Each command reads permissions in its own process, so a permission granted during a `batch` applies from the next CLI call, not to later items in that batch.
5. Run `agent-desktop snapshot -i` to confirm. It reads the focused app. `snapshot --app Finder -i` returns `WINDOW_NOT_FOUND` when Finder has no open window.

`permissions` does not prompt. `permissions --request` can prompt, including for Automation. `permissions --guide` does not prompt and covers Accessibility and Screen Recording only. Automation reports `unknown` when macOS would need to prompt.

## What differs on macOS

**Headed actions.** The menu-bar exception covers every item in the application's native menu bar hierarchy, and a headless click on those items calls the same `AXPress`. A ref captured through a process rather than a window has no window to focus, so a headed pointer action on it skips the focus step. Before physical pointer input, a guard refuses it with `ACTION_FAILED` when the hit test is occluded or unknown, and with `STALE_REF` when the element changed, both before any input is sent. `AXShowMenu` applies only when the target provides it. Raw `--xy` input never changes focus.

**Headless type.** The `AXSelectedText` path works only on a text field, text area, secure text field or combo box whose selected text is settable. The `POLICY_DENIED` elsewhere comes from the keyboard fallback, which needs focus. A headed `type` posts keystrokes to the owning process.

**Judge delivery by observation.** Finder, for example, returns success from an `AXConfirm` that does nothing. Read the control back as the macOS skill says; take a full snapshot only when the change can land outside it.

**Surfaces.** `menubar` also holds an app's View and other top-level menus. With no `--app`, `menu-bar-extras` reads system items such as Wi-Fi, Battery, and Clock; with `--app` it reads that app's own item. These shell surfaces are available headless through `open-system-surface`. Reading a windowless background app through its process works for `window`, `menubar`, and `menu-bar-extras`, not `focused`; a regular app that has not shown its window returns `WINDOW_NOT_FOUND`. While a menu, sheet, alert, or popover is open, refs from the window tree behind it are the wrong target.

**Menus.** The `APP_UNRESPONSIVE` case after a headless `right-click` means the menu can be open even though the app did not answer in time. `menu` requires an open menu: if `snapshot --surface menu` returns `surface_absent` with `complete: true`, snapshot `menubar` when it exists, click its item without `--headed`, then snapshot `--surface menu`. An absent `menubar` can mean the app exposes no menu bar. A missing `complete`, `complete: false`, timeout or incomplete read is unknown, not absent. `select` on a combo box or menu button needs a closed-to-open menu transition, which is why the menu must start closed.

**Notification Center.** The notification commands drive Notification Center through accessibility, with the rules in the core `commands-system` reference. When agent-desktop opens the center, it closes it afterwards and restores the previous app focus. Pass both `--expected-app` and `--expected-title` on a single dismiss or action, because the stack shifts. Use `wait --notification --app "App"` for banners that vanish before you list them.

**Chromium and Electron.** agent-desktop switches on the web accessibility of a Chromium renderer automatically when it needs it.

**Identity.** The bundle ID for `launch` looks like `com.apple.systempreferences`. Other commands take the display name through `--app`, or the bundle ID or executable name (`ControlCenter`) when no running app has that display name. Window IDs such as `w-4521` stay valid for the life of the window. Find them with `list-windows`.

**Refs use native addresses when available.** A ref turns back into the exact element it was captured from. If the element was removed or the app restarted, the ref is `STALE_REF`, `details.kind: retained_object_stale`, and it never falls back to a search for a look-alike. When an app yields no address for an element, that ref follows the cross-platform rule instead and must have been the only element in its window with its role, native id, name and description. Take a fresh snapshot after a list changes.

**Keys.** The primary modifier is `cmd`: `press cmd+c`, `press cmd+s`. The quit shortcuts behind the `--force` rule are `cmd+q`, `cmd+shift+q`, `cmd+alt+esc`, `ctrl+cmd+q` and `cmd+shift+delete`. Punctuation keys work in combos, as the symbol (`,` `.` `/` `;` `'` `[` `]` `\` `-` `=` `` ` ``) or by name (`comma`, `period`, `slash`, `semicolon`, `quote`, `leftbracket`, `rightbracket`, `backslash`, `minus`, `equal`, `grave`). Each one is sent with the key that types it in the active keyboard layout, so `press cmd+,` opens Settings on ANSI and JIS keyboards alike.

**Cursor overlay.** `cursor-overlay enable` draws the agent cursor in a click-through window. It never moves the OS pointer or activates an app. The macOS motion and image limits are in the core `commands-system` reference.
