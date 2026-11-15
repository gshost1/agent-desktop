### Bug Fixes

* A post-action `--wait-for` that matches a sheet, alert or popover on a window other than the first one of the process now returns that window's overlay instead of timing out while it snapshots the first window, and an overlay without the selector on one window no longer hides one with it on another.
* A post-action `--wait-for` reads overlays only on windows of the ref's own process instance, so a second running copy of the same application can no longer satisfy it.
* A post-action `--wait-for` whose window reads fail with `PERM_DENIED` now ends with `PERM_DENIED` and the action's delivery disposition instead of a `TIMEOUT`, and reads that fail as unresponsive after the ref's window closed now reach the 5 s `wait_owner_unresponsive` check.
* A post-action `--wait-for` after a menu bar item in a background application, where no window is focused, now reads the application's front window, and one taken without a window baseline reads the focused window instead of no window.
