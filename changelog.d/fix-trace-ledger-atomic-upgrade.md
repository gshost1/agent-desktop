### Bug Fixes

* A session trace's budget ledger that is new, empty or written by an older version is now replaced through a temporary file and a rename instead of being rewritten in place. A short write followed by an error, such as a full disk, left a ledger no reader parses, and recording stopped for the rest of the session with "Session trace recording ledger is unreadable". Steady-state events still overwrite the same-length record in place. ([#147](https://github.com/gshost1/agent-desktop/pull/147))
