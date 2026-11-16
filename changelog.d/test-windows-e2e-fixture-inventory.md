### Testing

* The Windows live E2E suite runs nightly on a hosted `windows-latest` runner through `scripts/run-windows-e2e-ci.ps1` (new `windows-e2e` workflow, never a pull-request gate); its Preconditions scenario counts a `find` match that carries no ref, its foreground steals pick a visible window that takes foreground, and an aborted run prints the legs recorded so far. ([#210](https://github.com/gshost1/agent-desktop/pull/210))
