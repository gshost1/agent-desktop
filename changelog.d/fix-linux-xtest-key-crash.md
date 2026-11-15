### Bug Fixes

* **linux:** X11 `press`, `--headed type` and other key input no longer crash GTK 3 applications and Chrome: the accessibility event listener each agent-desktop process registers now outlives the process by 1 s after it sent keys, instead of leaving while the application's at-spi2-atk key snooper is still answering them; `--headed press --window-id` into a GTK 3.24 `GtkEntry` crashed 3 of 8 times before and 0 of 12 after, Chrome 157 died 5 of 8 times before and 1 of 18 after, with no added command latency ([#NNN](https://github.com/gshost1/agent-desktop/pull/NNN))
