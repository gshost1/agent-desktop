### Bug Fixes

* `session end` and `cursor-overlay disable` on macOS no longer fail with `INTERNAL` ("did not acknowledge the control") when the session's cursor overlay exits before acknowledging the stop; an overlay that is gone counts as stopped, and one still running at the deadline fails with `TIMEOUT`
