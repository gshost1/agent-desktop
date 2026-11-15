### Performance

* Linux `snapshot --surface sheet` and `--surface alert` find a dialog drawn inside the window with a structural read that skips web page content, and give only the found dialog the full read: on a Chrome page of 3,800 nodes with no dialog the answer is `ELEMENT_NOT_FOUND` in 206 ms instead of `TIMEOUT` after 3,033 ms, and GTK3, GTK4, Qt and libadwaita windows answer 21 to 42 % sooner
