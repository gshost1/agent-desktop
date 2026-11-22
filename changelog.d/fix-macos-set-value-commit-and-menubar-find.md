### Features

* macOS `set-value --commit` opts in to ending an edit without focusing another control or pressing a default button; it may fire the edited field's own end-of-edit action, such as a SwiftUI `onSubmit`, verifies focus and value evidence, and reports `edit_commit_no_safe_peer` with recovery guidance when commitment is unobserved or `edit_commit_focus_delegated` if a container passes focus to a control. Linux, Windows, and web refs reject the flag with `ACTION_NOT_SUPPORTED` before writing.

### Bug Fixes

* `find --surface menubar` no longer returns a `TIMEOUT` that more time cannot fix when part of the menubar cannot be read, such as a menubar Help menu's search field after a menu was opened: matches seen are listed with `complete: false`, and no match returns `ELEMENT_NOT_FOUND` with `details.kind: "locator_unreadable_part"`.
* An empty `find --surface menubar` answer carries `data.hint`, which says that closed menus are searched and that a pane or a window that is not open belongs to a window search.
