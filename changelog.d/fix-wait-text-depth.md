### Bug Fixes

* `wait --text` searches as deep as `find --text`: 50 levels, up from the snapshot default of 10. Text nested deeper than 10 levels made the wait time out while `find --text` found it at once. One example is a GNOME Text Editor document body once the window title stops showing new words; its text view is 22 levels down. The snapshot the wait saves now covers the same depth. ([#181](https://github.com/gshost1/agent-desktop/pull/181))
