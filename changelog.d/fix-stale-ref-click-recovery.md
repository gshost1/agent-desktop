### Bug Fixes

* Restore fresh-snapshot recovery hints in safe stale-ref error envelopes without making complete-absence refusals retryable.
* Guide safe stale-ref recovery through an app-state check when the owner exited or crashed, including platform owner-exit refusals that cannot be retried as they are.

### Documentation

* Align the interaction skill's stale-ref recovery guidance with exited and crashed owners and platform owner-exit refusals.
