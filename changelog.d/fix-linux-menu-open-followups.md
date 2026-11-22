### Bug Fixes

* Return `TIMEOUT` for incomplete Linux menu reads, require menu-typed popup evidence fully containing at least one readable item, and tolerate items or X11 windows disappearing during a read.
* Detect open Linux menus when scrolled-out, screen-edge-clipped, or stale item bounds lie outside the popup.
* Preserve completed empty Linux menu reads at deadline expiry, treat vanished owners as absent, and retain original read errors and observed popups after partial inventory reads.
* Retry newly mapped Linux menu windows until a complete accessibility read has run while that window was already mapped, without trusting stale menu-item bounds.
* Keep newly mapped Linux menu windows pending for at least 175 ms and a completed inventory read, restart settling after reappearance, and reconnect once after a failed X11 popup read.

### Performance

* Stop retrying Linux menu snapshots after a complete AT-SPI read accounts for a settled mapped popup, and reuse one X11 connection across opening probes unless a failed read requires reconnection.
