### Bug Fixes

* Match Linux X11 dialogs by same-process geometry when native and accessibility titles differ, restoring dialog bounds, focus, and button actions while preserving ambiguous matches.
* Briefly recheck X11 activation before sending keys when the target reports an active accessibility window, avoiding refusals while a newly opened dialog activates.
