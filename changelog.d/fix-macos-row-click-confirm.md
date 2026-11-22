### Bug Fixes

* macOS: a `click` on a table or outline row no longer fails `ACTION_FAILED` with `AXConfirm did not establish a verifiable effect`; it skips the text labels inside the row and succeeds when the row's `AXSelected` or its container's `AXSelectedRows` shows the row newly selected.

### Features

* macOS: `select @<row> "<label>"` selects a table or outline row by writing `AXSelected` or the container's `AXSelectedRows` and reading it back.
