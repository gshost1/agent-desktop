### Bug Fixes

* `find --surface menubar` no longer returns a `TIMEOUT` that more time cannot fix when part of the menubar cannot be read, such as a menubar Help menu's search field after a menu was opened: matches seen are listed with `complete: false`, a `--first` match with every node before it read is still answered, and no match or a pick the unread part could change (`--nth`, `--last`, `--count`, or a `--first` match after the unread part) returns non-retryable `ELEMENT_NOT_FOUND` with `details.kind: "locator_unreadable_part"`.
* An empty `find --surface menubar` answer carries `data.hint`, which says that closed menus are searched and that a pane or a window that is not open belongs to a window search.
