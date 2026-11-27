### Features

* macOS: a `click` on a button inside a sheet or alert that the press alone could not verify now watches that surface's own window for up to 750 ms: `delivered_verified` with `details.surface_after_press.effect: "closed"` when it left the screen, `delivered_unverified` with `effect: "still_showing"` when it is still open, and `effect: "unobservable"` when the surface has no window of its own to watch; a working press and a dead one no longer look the same.
