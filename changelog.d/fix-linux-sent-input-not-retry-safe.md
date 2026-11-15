### Bug Fixes

* Linux `select` on a dropdown it had to open (a libadwaita combo row clicked under `--headed`, a GTK4 `DropDown` it expanded) no longer answers a missing or refused choice as `not_delivered` with `retry: safe`; the click or expand already moved focus and changed the control, so the error is `delivered_unverified` with `retry: unsafe` ([#208](https://github.com/gshost1/agent-desktop/pull/208))
