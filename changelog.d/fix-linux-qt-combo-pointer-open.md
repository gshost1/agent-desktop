### Bug Fixes

* Open collapsed Qt combos before headed selection, then find the choice again and verify its committed value instead of attempting to click a hidden popup row.
* Refuse before input when a Qt combo does not advertise expansion; otherwise reserve cleanup time before opening and send Escape if expansion cannot be observed, retaining the unsafe retry disposition after input.
