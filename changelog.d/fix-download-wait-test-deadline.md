### Bug Fixes

* `wait --download` no longer reports TIMEOUT for a finished download when a slow host takes over 500 ms to open the download ledger lock; the ledger lock now waits up to 2 s.
