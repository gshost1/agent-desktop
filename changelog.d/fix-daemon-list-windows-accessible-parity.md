### Bug Fixes

* On macOS, global window inventory reads each app's accessibility windows with a 50 ms probe before retrying timed-out apps in order within one shared 500 ms budget, so unresponsive apps cannot each add a 500 ms retry.
* On macOS, the daemon declines commands that need an Accessibility or Screen Recording grant it lacks, so the caller runs them itself.
* `wait` no longer reports a `last_error` that only says its own deadline expired during the final poll.
