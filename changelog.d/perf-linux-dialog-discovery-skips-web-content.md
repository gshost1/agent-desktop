### Performance

* Linux `snapshot --surface sheet` and `--surface alert` find a dialog drawn inside the window with a structural read that skips web page content, and give only the found dialog the full read: on a Chrome page of 3,800 nodes with no dialog the answer comes in 74 ms instead of 1,948 ms, and GTK4, Qt and libadwaita windows answer 9 to 30 % sooner ([#209](https://github.com/gshost1/agent-desktop/pull/209))
