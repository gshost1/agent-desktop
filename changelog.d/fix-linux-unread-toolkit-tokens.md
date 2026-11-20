### Bug Fixes

* Linux: a snapshot withholds `Select` on an index-only list, row, tab list or tab when the application's toolkit read answers without a supported toolkit, matching the headless action's `selection_index_unproven` refusal. It keeps `Click` on an entry unless GTK 4 is identified, and keeps all tokens when the toolkit read times out or is not attempted, so a fresh live read can judge the route at action time. `docs/json-output.md` now says what `available_actions` promises on each platform ([#227](https://github.com/gshost1/agent-desktop/pull/227))
