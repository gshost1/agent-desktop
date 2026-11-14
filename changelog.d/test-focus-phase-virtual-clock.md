### Testing

* The pointer focus-phase tests run their hover, drag and leased-wait polls on the virtual test clock, so a slow CI runner no longer runs out their 400 ms budget before a transient overlay clears; under a 2% CPU quota the old tests failed 19 of 20 runs and the new ones pass every run. ([#161](https://github.com/gshost1/agent-desktop/pull/161))
