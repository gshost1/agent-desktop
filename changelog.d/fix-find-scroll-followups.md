### Bug Fixes

* macOS `find --scroll` now puts the scroll bar back when the measured page write after its probe step fails, and ends the page with a `delivered_unverified` error (`details.kind: scroll_search_probe_not_restored` when the restore was not taken) when the bar cannot be put back, instead of reading the 0.2% probe shift as a moved page and walking a refusing list to 200 pages or both ends without loading it.
* `find --scroll` now counts a page sent at an end of the area that read unchanged as `data.scroll.pages_at_end`, and any error after such a page is `delivered_unverified` instead of retry-safe; on macOS a page whose settle reads are cut short by the deadline is `unconfirmed` instead of an end.
* `find --help` now says `--timeout-ms` defaults to 20000 with `--scroll`, as the skill and docs already did.
