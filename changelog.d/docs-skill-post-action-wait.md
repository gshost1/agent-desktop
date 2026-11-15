### Documentation

* The interaction reference describes the post-action `--wait-for` changes from [#193](https://github.com/gshost1/agent-desktop/pull/193): a `PERM_DENIED` read ends the wait at once instead of a `TIMEOUT`, unresponsive reads after the ref's window closed still reach the 5 s `wait_owner_unresponsive` check, a menu bar ref in a background app reads the app's front window, and overlays are read only from the ref's own process instance.
