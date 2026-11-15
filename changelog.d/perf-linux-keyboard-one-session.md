### Performance

* **linux:** X11 `press` and `--headed type` read the keymap on the connection that sends the keys, so each keyboard action opens one XTest connection and sweeps the held-input journal once instead of twice: on a private Xvfb display, min of 21 interleaved runs, `--headed press` 70.5 to 69.5 ms (median 72.1 to 70.0 ms) and `--headed type` 71.4 to 70.4 ms (median 72.9 to 71.9 ms) ([#NNN](https://github.com/gshost1/agent-desktop/pull/NNN))

### Bug Fixes

* **linux:** X11 physical text now refuses a chunk with `ACTION_NOT_SUPPORTED`, `check: keyboard_layout`, when a layout replaced after planning moved a keysym on a keycode the chunk presses, instead of typing other text ([#NNN](https://github.com/gshost1/agent-desktop/pull/NNN))
