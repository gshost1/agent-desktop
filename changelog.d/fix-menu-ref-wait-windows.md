### Bug Fixes

* A post-action `--wait-for` after a menu or menu bar item now reads the process's focused window and the windows the command opened, so `click <menu item> --wait-for ":Width"` matches as soon as the effect shows instead of polling the menu bar until the timeout.
* A post-action `--wait-for` that no window satisfies also reads the sheets, alerts and popovers the process has open, so a selector in a sheet the action opened matches; on macOS Preview, Adjust Size… with `--wait-for ":Fit into"` drops from a 10.5 s timeout to under 1 s.
