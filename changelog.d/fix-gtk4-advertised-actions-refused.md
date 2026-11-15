### Bug Fixes

* Linux: a snapshot no longer lists `Click` on a GTK4 entry or password field whose only click verb is `activate`, or `Select` on a GTK4 list box, its rows, a tab list or its tabs, because headless `click` and `select` refuse them (`POLICY_DENIED` and `selection_index_unproven`); `--headed click` and `--headed select` still reach them through the pointer. On the GTK4 widget zoo the 9 advertised-but-refused rows of the live ref-contract leg now pass. ([#217](https://github.com/gshost1/agent-desktop/pull/217))
