### Bug Fixes

* macOS `select <textfield> <text>` now reads the field's value again after the selection write and verifies only when the text at the selected range is still the requested text, so a field edited between the search and the write fails `ACTION_FAILED` with `details.kind: selection_text_changed` and `retry: unsafe` instead of reporting a verified selection of other words; an unreadable or truncated value leaves it `delivered_unverified`.
* macOS `select <textfield> <text>` now counts overlapping occurrences, so `ana` in `banana` fails `AMBIGUOUS_TARGET` with both offsets instead of selecting the first one as if it were unique.
