### Bug Fixes

* Linux: `launch` prefers a new descendant with an accessible window over a windowless descendant registered first, such as a wrapper's helper or a restarting process, and reports `AMBIGUOUS_TARGET` when two descendants have windows. If its only new descendant never shows a window, it waits until the launch deadline before selecting it.
* Linux: `launch` waits the full launch deadline for a process that maps a window but never joins the accessibility bus, instead of returning after about 7 seconds.
