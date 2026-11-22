### Bug Fixes

* macOS: a new table or outline row selection can verify an unconfirmed `AXConfirm`, selection step, or dispatched headed click. Selection alone won't confirm an uncertain `AXOpen` or `AXPress`.

### Features

* macOS: `select @<row> "<label>"` selects a table or outline row and can replace the current selection. If the row is already selected, it makes no write and returns `not_delivered`.
