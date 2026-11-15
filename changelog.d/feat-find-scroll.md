### Features

* `find --root <list> --scroll [DIRECTION]` finds an element that a lazily loaded list has not built yet in one call: it pages the list headlessly, looks after each page, turns back at the end, and stops at the first match, at both ends, after 200 pages or at the deadline. `data.scroll` reports the pages sent, whether the view moved and why the search stopped; the view stays where the match is. Paging is supported on macOS; elsewhere a search that needs a page returns `PLATFORM_NOT_SUPPORTED`.
