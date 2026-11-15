### Bug Fixes

* A post-action `--wait-for` that follows a ref now also reads the menu the ref's process has open, once per poll, so a click that opens a menu matches an item in it; macOS menus stay application-owned and are not read this way. ([#214](https://github.com/gshost1/agent-desktop/pull/214))
* Windows: `snapshot --surface sheet` and the followed post-action wait read the named window's own modal sheet, or the modal dialog that window owns, whether or not the app is in the foreground, and answer `ELEMENT_NOT_FOUND` with `details.kind: "surface_absent"` instead of `WINDOW_NOT_FOUND` when the window presents none. ([#214](https://github.com/gshost1/agent-desktop/pull/214))
