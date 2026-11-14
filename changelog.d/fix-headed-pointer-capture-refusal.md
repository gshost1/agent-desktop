### Bug Fixes

* `hover` and `drag` with `--headed` refuse a ref that was not proven unique when it was captured before they take the interaction lease or raise any window, under every `--timeout-ms` including `0`. They now answer with the `AMBIGUOUS_TARGET` or `STALE_REF` refusal that resolution gives (`not_delivered`, no `preparatory_focus`) instead of raising the target window first, and with `--timeout-ms 0` instead of a `TIMEOUT` after the raise. ([#NNN](https://github.com/gshost1/agent-desktop/pull/NNN))
