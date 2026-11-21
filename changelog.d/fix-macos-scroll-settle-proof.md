### Bug Fixes

* macOS: a page scroll refused with `kAXErrorCannotComplete` is reported `not_delivered` and safe to retry only when the scroll area is also at its end that way, by its scroll bar value or, without a bar, a content size that fits; unchanged signals of an area with room left keep the uncertain disposition, so a retry cannot scroll twice past a pinned header or a late animation.
* macOS: the `scroll_step_overshoot` message says the last scroll step moved the target past the viewport, instead of claiming every page-sized step did.
