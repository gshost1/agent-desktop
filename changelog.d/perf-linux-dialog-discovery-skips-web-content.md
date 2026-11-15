### Performance

* Linux `snapshot --surface sheet` and `--surface alert` look for a dialog drawn inside the window with a structural read that skips web page content, and give only the found dialog the full read: on a Chrome page of 3,800 nodes with no dialog the answer comes in 89 ms instead of 1,989 ms, and a libadwaita in-window alert in 60 ms instead of 103 ms; a window that is itself the dialog, such as a GTK3 or Qt file chooser, is still read once ([#209](https://github.com/gshost1/agent-desktop/pull/209))
