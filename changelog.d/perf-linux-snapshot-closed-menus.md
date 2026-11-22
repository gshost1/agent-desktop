### Bug Fixes

* **linux:** items of a closed menubar menu are no longer in a window snapshot, a window `find` or a window `wait --text`/selector, and a ref captured earlier for such an item now gets `STALE_REF`; the menubar's top-level openers stay, so open the menu first, then read it with `snapshot --surface menu`

### Performance

* **linux:** a window snapshot no longer reads the items of closed menubar menus: LibreOffice Writer `snapshot -i` drops from 216 KB, 1,623 refs (1,502 ambiguous) and 810 ms to 27 KB, 202 refs (101 ambiguous) and 156 ms (median of 7)
