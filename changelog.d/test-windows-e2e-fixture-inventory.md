### Testing

* The Windows live E2E suite runs nightly on a hosted `windows-latest` runner through `scripts/run-windows-e2e-ci.ps1` (new `windows-e2e` workflow, never a pull-request gate), and its fixture inventory declares the horizontal-scroll ids the Preconditions scenario checks for.
