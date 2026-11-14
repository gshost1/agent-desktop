# Windows first contact and differences

The full Windows notes behind the short platform skill: supported sessions, every first-contact note and what differs from the other platforms.

Requires Windows 10 1809+ / Windows Server 2019+ (x64 or ARM64). Run it in the
interactive desktop session of the signed-in user. Session 0 (services and
SYSTEM tasks), Server Core, the secure desktop (UAC prompts, the sign-in
screen), a locked desktop and another user's session are not supported:
observation, actions and capture there fail or return nothing.

## First Contact

- **Quote every ref in PowerShell.** PowerShell reads a bare `@token` as its
  splatting operator and deletes the argument before the binary sees it, so
  `set-value @s8f3k2p9:e1 hi` fails `INVALID_ARGS`. Write
  `set-value '@s8f3k2p9:e1' hi`. cmd.exe and bash need no quoting.
- **A window behind another window is drivable. A minimized one is not.**
  Every element of a minimized window reports `offscreen` and ref actions
  fail. Run `restore --app <image>` and snapshot again. Do not use
  `focus-window` by reflex: it steals the user's foreground.
- **No permission dialog exists.** `permissions` reports `automation` as
  `not_required`. Elevation boundaries are the only access control.
- **Chromium and Electron apps read thin before they settle.** Pass a longer
  `--timeout-ms` to `snapshot` instead of concluding the tree is empty.
- **Blocked shortcuts.** `alt+f4`, `win+l`, `win+d`, `alt+tab`, `win+r`,
  `win+x`, `win+e`, `ctrl+shift+esc` and their modifier supersets are refused
  without `--force`. In combos, `cmd` and `meta` are the Windows key, so
  `meta+c` is a shell shortcut. Write `ctrl+c` to copy.
- **`type` needs `--headed`.** It is keystroke synthesis through `SendInput`,
  so a strict-headless `type` returns `POLICY_DENIED`. Use `set-value`, or pass
  `--headed`. It ends `delivered_unverified` (`verification_scope:
  "unavailable"`) when the field exposes no readable Text selection, and can
  verify when one is readable: confirm with a fresh `get`.
- **`press --app` writes to the foreground queue** with no per-process
  targeting. A headless press to a non-frontmost app fails closed. A
  non-interactive caller (service, scheduled task, CI job) gets
  `delivered_unverified`.
- **`launch` takes an absolute path or a bare name** under System32 or the
  Windows directory, with or without `.exe` (`notepad` or `notepad.exe`), not
  a display name. Use `list-apps` to find a running app. `launch --reopen` returns
  `ACTION_NOT_SUPPORTED`: Windows has no reopen event, and `launch_path` is absent.
- **A launched program that exits before its process shows a window** ends the
  wait on the next poll with `APP_UNRESPONSIVE`, `details.kind: exited` (`crashed`
  for an NTSTATUS crash code), `details.status` and `details.pid`,
  `delivered_unverified`. A System32 stub that starts a packaged app and exits
  (such as `calc` on Windows 11) ends this way, because the window it caused belongs to a
  process the launch cannot attribute: run `list-windows` before you launch
  again. An attach to a running instance reads its windows once and returns
  with no `window` when it shows none, instead of waiting out `--timeout`.
- **Notifications:** `list-notifications` with the Action Center closed needs
  `--headed`. Single-item `dismiss-notification` and `notification-action` need
  `--headed` and `--expected-app` or `--expected-title` for the entry at INDEX.
- **Surfaces:** an open or context menu is the `menu` surface. Windows does
  not advertise a `menubar` surface.
- **Commands that raise shell chrome take the foreground.** Strict headless
  refuses them with `POLICY_DENIED` before anything is raised.
- **Stateful actions run once, then verify by re-reading.** `delivered_verified`
  means the readback matched. `ACTION_FAILED` with
  `details.kind: "post_action_verification"` means the action landed but the
  readback differs: read `details.post_state` and do not retry. A `set-value`
  the app reformats reads as failed. See `references/troubleshooting.md`.
- **Unsigned binary warnings are expected.** See `references/troubleshooting.md`.

## What differs on Windows

- Ref actions are semantic UI Automation calls in headless and `--headed` mode, except `right-click`, `double-click`, `triple-click`, `type` and `focus`, which are physical or take focus and need `--headed`.
- A ref resolves only when it was the only element in the window its snapshot captured with its role, native id, name and description, and exactly one live element still matches. Uniqueness counts the whole window, not the shallow tree a `--skeleton` or `--max-depth` snapshot presents, so a twin below the cut still refuses the ref. Identical twins, such as several identical `Delete` buttons, are refused with `AMBIGUOUS_TARGET`, `details.kind: identity_not_unique`, before anything reaches the app; a live second match is `saved_ref_ambiguous`, and a removed element is `STALE_REF`, `saved_ref_absent`. Take a fresh snapshot after a list changes, and use `--headed mouse-click --xy` when you must pick one of two identical controls.
- The cursor overlay plays its per-action travel only for headless semantic actions. A `--headed` action moves the real cursor instead.
