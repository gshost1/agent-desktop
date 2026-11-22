### Features

* macOS `set-value --commit` opts in to ending an edit without focusing another control or pressing a default button; it may fire the edited field's own end-of-edit action, such as a SwiftUI `onSubmit`, verifies focus and value evidence, and reports `edit_commit_no_safe_peer` with recovery guidance when commitment is unobserved or `edit_commit_focus_delegated` if a container passes focus to a control. Linux, Windows, and web refs reject the flag with `ACTION_NOT_SUPPORTED` before writing.

### Bug Fixes

* macOS `set-value --commit` checks every focus destination, retains the focus restoration result after delegation, excludes actionable or unreadable containers, and requires stable pre-confirm neighbour evidence. It never sets window `AXMain` and skips window focus in headless mode. An unattempted end-edit fails with `edit_commit_not_attempted` instead of a looping hint; inline editors reject the flag before writing with `edit_commit_unsupported_target`.
* `find --surface menubar` no longer returns a `TIMEOUT` that more time cannot fix when part of the menubar cannot be read, such as a menubar Help menu's search field after a menu was opened: matches seen are listed with `complete: false`, and no match returns `ELEMENT_NOT_FOUND` with `details.kind: "locator_unreadable_part"`.
* An empty `find --surface menubar` answer carries `data.hint`, which says that closed menus are searched and that a pane or a window that is not open belongs to a window search.
