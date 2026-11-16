### Testing

* The Windows live E2E suite runs nightly on a hosted `windows-latest` runner through `scripts/run-windows-e2e-ci.ps1` (new `windows-e2e` workflow, never a pull-request gate), its fixture inventory declares the horizontal-scroll ids the Preconditions scenario checks for, and its Preconditions scenario counts a `find` match that carries no ref, since `find` saves no snapshot for one. ([#210](https://github.com/gshost1/agent-desktop/pull/210))
