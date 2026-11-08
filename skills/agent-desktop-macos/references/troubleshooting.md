# macOS troubleshooting

Each entry is a symptom, its cause, and the fix. Read `error.code`, `error.suggestion` and `error.disposition` in the JSON envelope first.

## PERM_DENIED

**Symptom.** A command returns `PERM_DENIED`, or `permissions` shows `denied`.

**Cause.** The app that launched agent-desktop has no Accessibility permission (or Screen Recording, for `screenshot` and `session start --screenshots`). The grant belongs to the launching app, not to the shell. A new launcher, such as a different terminal or an IDE, needs its own grant.

**Fix.** Follow "First contact" in the macOS skill: `permissions --guide`, turn on the launching app (and the `agent-desktop` binary if it is listed on its own) in System Settings > Privacy & Security > Accessibility or Screen Recording, then run `agent-desktop permissions` to confirm `granted`. The launching app does not need a restart.

The guide offers the app bundle of the nearest process above agent-desktop. Under a tmux server or an SSH session, no such app exists, so the guide offers the `agent-desktop` binary. In that case, add the terminal app that started tmux by hand. macOS 27 and later name the Accessibility list Device Control and Data Access.

For Automation, open Privacy & Security > Automation and allow the launching app to control System Events.

## Empty or sparse tree

**Symptom.** `snapshot` returns few nodes, or no refs.

**Cause.** Some apps expose little accessibility data: custom-rendered UIs, games, some creative tools, and parts of web views. Chromium and Electron apps need web accessibility enabled in the renderer.

**Fix.**
1. Check permissions first (see PERM_DENIED).
2. Remove `-i` to see non-interactive nodes. Raise `--max-depth`.
3. Run `list-surfaces --app "App"`. The content may sit in a sheet, popover or menu surface.
4. Retry the snapshot once. A renderer needs a short settle time after accessibility is enabled.
5. Use `screenshot` as a visual fallback (needs Screen Recording).
6. For a fresh Chromium app, use `launch "App" --cdp` and a CDP client for the web contents.

## Slow or truncated snapshot

**Symptom.** A snapshot is slow, or returns `"complete": false` with `"truncated": true`.

**Cause.** Large apps (Xcode, Safari with many tabs) have deep trees. A truncated snapshot ran out of its observation budget.

**Fix.** Use `find` when you know the target. Otherwise use `snapshot --skeleton -i --compact`, then `snapshot --root @<snapshot_id>:eN` to drill down. Add `--window-id` to limit the window.

## ACTION_FAILED

**Symptom.** A ref action returns `ACTION_FAILED`.

**Cause.** The element is disabled, the app is busy, or the element does not support the action. The semantic chain tries only the actions the element advertises, then stops.

**Fix.**
1. Run `is @ref --property enabled`. Wait or fix the state if it is false.
2. Run `get @ref --property bounds` and check that the element is on screen.
3. If physical input is right for the task, retry with `--headed`.
4. As a last step, use the keyboard: `focus @ref`, then `press return`.

## POLICY_DENIED

**Symptom.** A command returns `POLICY_DENIED`.

**Cause.** The semantic path could not finish, and the physical path is blocked because the command ran headless. Hover, drag, double-click, triple-click and cursor-moving mouse commands always need `--headed`. Coordinate `right-click` is also blocked headless.

**Fix.** Add the global `--headed` flag only if physical input is what you want. Example: `agent-desktop --headed mouse-click --xy 640,400 --button right`. Take coordinates from `get @ref --property bounds`.

## APP_UNRESPONSIVE after right-click

**Symptom.** `right-click` returns `APP_UNRESPONSIVE` with `delivery_uncertain` and `retry: unsafe`.

**Cause.** `AXShowMenu` can enter modal menu tracking and return `kAXErrorCannotComplete` after the menu opened.

**Fix.** Do not retry. Run `snapshot --app "App" --surface menu -i`. If the menu is there, use it. If not, run `list-surfaces --app "App"`. For a combo box or menu button, use `select @ref "Option"`.

## Saved file has `:` in its name, in the wrong folder

**Symptom.** A save reported success, but the file is missing from the folder you wanted. A file such as `:Users:me:Reports:q3.rtf` appears in the panel's default folder, often iCloud Drive or Documents.

**Cause.** A full path went into the save panel's name field. The panel does not read it as a folder. It keeps its current folder and turns each `/` into `:` in the file name. `set-value` reported `delivered_verified` because the field held exactly that text.

**Fix.** Put only the file name in the name field and choose the folder with Go to Folder, as in [Save into a given folder](#save-into-a-given-folder). Move the misnamed file yourself; agent-desktop does not undo a save.

## Save into a given folder

An Open or Save panel can be served by a separate process and drawn inside the app's sheet. `press --app` sends its keys to the process that owns the focused panel control, so the panel's own shortcuts work. Go to the folder first, then set the name and save:

```bash
agent-desktop press cmd+s --app "TextEdit"                                    # open the Save sheet
agent-desktop press cmd+shift+g --app "TextEdit"                              # Go to Folder
agent-desktop find --app "TextEdit" --surface sheet --role textfield --state focused --first
agent-desktop set-value @s1:e1 "~/Reports"                                     # the folder
agent-desktop press return --app "TextEdit"                                   # go there
agent-desktop find --app "TextEdit" --surface sheet --role textfield --name "Save As:" --first
agent-desktop set-value @s2:e1 "q3-report"                                     # file name only, no slashes
agent-desktop press return --app "TextEdit"                                   # Save
```

Replace `@s1:e1` and `@s2:e1` with the `ref_id` each `find` returns. While a panel control has focus, `press` reports `data.details.keyboard_target` with `"route": "hosted_view"` and the pid that received the keys. `"route": "host"` with an `unproven` reason means agent-desktop could not prove which process owns the panel's focus and sent the keys to the app, where the panel does not see them. A key that changed nothing in the panel fails with `ACTION_FAILED` and `"effect": "none_observed"`; do not repeat it. In both cases use the panel's controls by ref instead: `select @where "<folder>"` on the `Where:` pop-up (role `combobox`) for a folder it lists, `click` on a folder row in the file list to open it, `set-value` on the name field, then `click` on the `Save` button. Check the result yourself: the sheet is gone from `list-surfaces --app "TextEdit"` and the file is in the folder.

## Rename reports success but the old name stays

**Symptom.** After New Folder, or a Rename command, `set-value` on the row's name field reported success, `press return` ran, and the item still has its old name, such as `untitled folder`.

**Cause.** While a row is renamed, a separate editor floats over it and holds the text that Return commits. A write to the row's field underneath changes what that field reports, not what the editor commits.

**Fix.** Write to the editor. Run `snapshot --app <App> --surface focused -i`: while the edit is open, the root is the editor, a `textfield` holding the current name. `set-value` its ref with the new name; the result says `application_commit: "pending"`. Then `press return --app <App>` and confirm the new name with an independent read, such as a listing of the folder. A `set-value` on the row's own field also reaches the editor when it can prove the editor belongs to that field.

## Return on a sheet reports none_observed

**Symptom.** `press return --app <App>` on a sheet, such as Finder's Go to Folder after `set-value` on its path field, fails with `ACTION_FAILED` and `details.modal_key.effect: "none_observed"`, and the sheet is still open.

**Cause.** `press` sent a real key and then the field's confirm action, and the sheet's window stayed on screen. A sheet in an app that is not in front can ignore keys sent to it, and the confirm action only ends editing in the field.

**Fix.** Do not repeat the same press; the disposition is `retry: unsafe`. Run `snapshot --app <App> --surface sheet -i`. Click the default button by ref when `default_button` is `true`. Otherwise send the key once more with `--headed`, which brings the window forward before the real key. To dismiss the sheet, click its Close or Cancel button.

## select does not find the menu

**Symptom.** `select` fails to confirm the choice.

**Cause.** `select` proves success by a closed-to-open menu change. An already open menu hides that change.

**Fix.** Dismiss the open menu with `press escape`, then run `select` again.

## APP_NOT_FOUND or WINDOW_NOT_FOUND

**Fix.** Run `agent-desktop launch "App Name"` (display name or bundle ID). Then run `list-windows --app "App"`. A full-screen app still appears in `list-windows`, but its bounds equal the screen and its state can lag behind animations, so use `wait`.

## Notification Center

| Symptom | Cause | Fix |
|---|---|---|
| `POLICY_DENIED` when listing: the center is closed | The command is headless and cannot open the center | Add `--headed`, or open the center yourself |
| `dismiss-notification` returns `ACTION_FAILED` | No dismiss path worked on that item | See the chain below, then list again |
| `NOTIFICATION_NOT_FOUND` | The index moved after a dismiss | Run `list-notifications` again |
| Banner gone before you list it | Banners are transient | Use `wait --notification --app "App"` |
| A dismissed item is replaced by another | Notifications from one app are stacked | Dismiss again. `dismiss-all-notifications` may need more than one round |
| Calendar widget stays | It is a system widget, not a notification | Expected |

The dismiss chain runs in order: the `AXDismiss` and `AXRemoveFromParent` actions, then a close button named close, clear or dismiss, then hover to reveal a hidden close button and press it. `AXPress` is never used, because it opens the source app and does not dismiss.

## Keyboard shortcut does nothing

**Cause.** The shortcut used `ctrl` where macOS uses `cmd`, or the keys went to the wrong app.

**Fix.** Use `cmd+...`. To send keys to one app without focusing it, use `press cmd+s --app "App"`.
