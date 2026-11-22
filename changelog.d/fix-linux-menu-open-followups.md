### Bug Fixes

* Return `TIMEOUT` for incomplete Linux menu reads, require menu-typed popup evidence fully containing at least one readable item, and tolerate items or X11 windows disappearing during a read.
* Detect open Linux menus when scrolled-out, screen-edge-clipped, or stale item bounds lie outside the popup.
* Retry newly mapped Linux menu windows until a complete accessibility read has run while that window was already mapped, without trusting stale menu-item bounds.

### Performance

* Stop retrying Linux menu snapshots after a complete AT-SPI read accounts for the mapped popup, and reuse one X11 connection across opening probes.
