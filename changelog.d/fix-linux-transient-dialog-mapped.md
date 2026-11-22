### Bug Fixes

* Match Linux X11 dialogs by same-process geometry, including all four window-manager decoration edges, when native and accessibility titles differ, restoring dialog bounds, focus, and button actions while preserving ambiguous matches.
* Briefly recheck X11 activation while no active process can be identified, avoiding transient dialog activation refusals without accessibility registry reads or waits when another process owns the active window.
* Keep native activation and accessibility focus rechecks within one shared 300 ms budget, and preserve geometry ambiguity when reconciling unmatched active dialogs.
