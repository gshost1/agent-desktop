### Bug Fixes

* `find` saves a snapshot only when it returns at least one ref. A `find --first` whose only match could not get a ref, and any `find` whose matches are all context matches, used to save an empty snapshot, report its `snapshot_id` and take one of the 128 snapshot slots the store prunes older snapshots to keep; it now reports no `snapshot_id` and saves nothing. ([#NNN](https://github.com/gshost1/agent-desktop/pull/NNN))
