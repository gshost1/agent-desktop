### Testing

* Require `umockdev-run` only for the mock-mouse fixture, reporting affected legs unavailable without blocking other legs or using host hardware.

* Supply a virtual udev mouse to the Linux Settings Mouse fixture so pointer-speed checks do not depend on host input hardware, retaining the real Settings, GSettings, and reopen assertions.
