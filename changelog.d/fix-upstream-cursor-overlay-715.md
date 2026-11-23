### Bug Fixes

* macOS: the cursor overlay stays on screen across `--headed` commands and starts its next move from where it last landed, instead of jumping to a fixed offset beside each target; a cancelled drag still forgets its grab point, so the next move does not start from it. (from [vercel-labs/agent-desktop#715](https://github.com/vercel-labs/agent-desktop/pull/715))
