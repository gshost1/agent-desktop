### Bug Fixes

* On macOS, a counted child read whose responder refused the range one longer than the count, and answered only the exact range, no longer counts as proof of the count. The recount runs, so a list that grew between the two reads is reported incomplete instead of complete. ([#140](https://github.com/gshost1/agent-desktop/pull/140))

### Documentation

* The batch key rule in the system commands reference names the three flags whose JSON key is not the flag name: `--ref` is `ref_id`, `--state` is `states` and `--arg` is `args`. Entries written with the old rule (`ref`, `state`, `arg`) were rejected. A test now parses every `agent-desktop batch` example in the skills. ([#192](https://github.com/gshost1/agent-desktop/pull/192))
* The interaction reference's fallback for opening a context menu, `--headed mouse-click --button right`, now includes the required `--xy X,Y`. Without it the command exits 2. ([#192](https://github.com/gshost1/agent-desktop/pull/192))
