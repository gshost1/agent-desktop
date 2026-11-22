### Bug Fixes

* Return `TIMEOUT` for incomplete Linux menu reads, require menu-typed popup evidence covering the menu's readable items, and tolerate items or X11 windows disappearing during a read.

### Performance

* Stop retrying Linux menu snapshots when AT-SPI already accounts for the mapped popup, and reuse one X11 connection across opening probes.
