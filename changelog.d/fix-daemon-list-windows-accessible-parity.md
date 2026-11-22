### Bug Fixes

* On macOS, `list-windows` retries a timed-out accessibility labeling read once with a longer budget, so a slow first read can label inaccessible windows instead of leaving them marked accessible.
* On macOS, the daemon declines commands that need an Accessibility or Screen Recording grant it lacks, so the caller runs them itself.
* `wait` no longer reports a `last_error` that only says its own deadline expired during the final poll.
