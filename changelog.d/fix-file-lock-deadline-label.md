### Bug Fixes

* A command whose deadline ran out before it ever found a state lock held (the ref store lock, the trace budget lock, a session lock, or the interaction lease's lock file on Linux, macOS and Windows) now fails `TIMEOUT` with `details.kind: deadline`, `timeout_ms` and `elapsed_ms`. It used to report `lock_timeout` with `contention_count: 0`, which sent agents looking for another agent holding a lock nobody held. A wait that found the lock held still reports `lock_timeout` with its `purpose`, `path` and count. ([#164](https://github.com/gshost1/agent-desktop/pull/164))
