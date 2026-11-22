### Bug Fixes

* macOS `set-value` on a plain text field now ends the edit with a focus round trip through another text field in the same window, so the app commits the value at once; it returns `application_commit: "committed"` with the neighbouring values the commit changed, and fails with `ACTION_FAILED` when the app did not keep the value, or with `details.kind: "edit_commit_refused"` when the app would not let the edit end, as when it opens an alert rejecting the value.
* `find` no longer returns a `TIMEOUT` that more time cannot fix when part of the tree cannot be read, such as a menubar Help menu's search field after a menu was opened: matches seen are listed with `complete: false`, and no match returns `ELEMENT_NOT_FOUND` with `details.kind: "locator_unreadable_part"`.
* An empty `find --surface menubar` answer carries `data.hint`, which says that closed menus are searched and that a pane or a window that is not open belongs to a window search.
