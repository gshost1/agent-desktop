### Bug Fixes

* `snapshot --compact` now collapses empty single-child `group` wrappers on every adapter that reports bounds. It checked each wrapper before hiding its bounds and before `-i` pruned its children, so on macOS, Windows and X11 no wrapper collapsed and `--compact` changed almost nothing; on the macOS fixture app bare `group` lines drop from 5 to 1. Refs and their allocation order are unchanged. The FFI `ad_get_tree_exact` compact path had the same defect.
