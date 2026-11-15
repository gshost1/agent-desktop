### Bug Fixes

* A headless ref action on a ref whose snapshot bounds were measured inside its window (`bounds_space: "window"`, for example GTK4 rows on Linux) no longer fails the `stable` check with "bounds changed since the previous observation": the pre-dispatch wait compares two live screen reads instead of the window-space snapshot hash, so a refusal names only its real reason, such as `supported_action`. ([#174](https://github.com/gshost1/agent-desktop/pull/174))
