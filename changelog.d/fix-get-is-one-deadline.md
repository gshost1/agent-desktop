### Bug Fixes

* `get` and `is` now resolve the ref and read the element under one 5 s budget. Before, the read started a second 5 s budget after the resolution, so a slow window could hold either command for up to 10 s; a read after a 4 s resolution now gets at most the 1 s left. ([#NNN](https://github.com/gshost1/agent-desktop/pull/NNN))
