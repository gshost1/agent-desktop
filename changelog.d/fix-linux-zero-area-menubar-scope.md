### Bug Fixes

* Keep Linux menubars with zero-area bounds unless the same window also exposes a presented menubar, preserving refs when no presented twin exists.

### Performance

* Avoid reading suppressed Linux menubar descendants while looking for a presented twin at a deeper tree level.
