### Bug Fixes

* Restore fresh-snapshot recovery hints in safe stale-ref error envelopes without making complete-absence refusals retryable.
* Guide safe stale-ref recovery through an app-state check when the owner exited or crashed, including platform owner-exit refusals that cannot be retried as they are.
* Attach fresh-snapshot recovery to destroyed Linux targets, stamp exited owners directly on Linux and Windows, and settle complete owner-exit evidence without changing wait retryability.
* Keep non-retryable partial absence errors without snapshot-only recovery.

### Documentation

* Align the interaction skill, recovery reference and Linux dead-owner test contract with exited and crashed owners and platform owner-exit refusals.
