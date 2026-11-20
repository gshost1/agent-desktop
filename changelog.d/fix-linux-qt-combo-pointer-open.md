### Bug Fixes

* Open collapsed Qt combos before headed selection, then find the choice again and verify its committed value instead of attempting to click a hidden popup row.
* Refuse before input when a Qt combo does not advertise expansion; otherwise reserve cleanup time before opening and send Escape if expansion cannot be observed, retaining the unsafe retry disposition after input.
* Send cleanup Escape only to an observed visible popup, and retain unknown popup state instead of treating an unread list as proof that it stayed open.
* Recheck popup visibility immediately before cleanup Escape and skip the key if the popup has closed or become unreadable.
* Report a popup close attempt only when cleanup sent Escape, not when its visibility recheck skipped the key.
* Close Qt combo popups when choosing a row fails after opening, preserving opener input accounting and reporting whether cleanup sent Escape and observed closure.
* Give unobserved Qt combo popups the same recovery advice and Activities Overview diagnosis as other pointer-opened dropdowns.
