### Bug Fixes

* A ref whose element is gone (`STALE_REF` with `details.kind` `saved_ref_absent`, `resolve_no_candidate` or `resolution_complete_absence` and `details.complete: true`) carries the `refresh_snapshot_then_retry_original` recovery again, or `inspect_state_then_retry_original` for an exited owner; its `retryable: false` stamp had dropped the recovery from an envelope whose disposition says retry is safe, as the Windows live suite measured on a removed list row. ([#236](https://github.com/gshost1/agent-desktop/pull/236))
