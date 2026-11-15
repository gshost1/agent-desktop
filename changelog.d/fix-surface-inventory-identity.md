### Bug Fixes

* macOS: `wait --event surface-appeared` no longer fires when focus moves between windows of an app with an open sheet or popover; `list-surfaces` now reads every window and names each sheet, popover and alert by its own window id (`w-<n>`), not by its position under the focused window
* macOS: a surface read that could not see some surface kinds is reported to `wait --event` as incomplete, so it fires neither `surface-appeared` nor `surface-dismissed`; in an app too dense for the menu search, the window server's menu-window list settles that no menu is open
