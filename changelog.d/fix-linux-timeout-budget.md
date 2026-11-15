### Bug Fixes

* Linux commands under a display that stopped answering return within their timeout plus one second again when a killed input call left keys or buttons held: the permission preflight no longer spends a detached second trying to release them on a display that did not answer its probe (`snapshot --timeout-ms 2000` 3,530 ms → 2,520 ms, budget 3,000 ms)
