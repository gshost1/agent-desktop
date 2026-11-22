### Bug Fixes

* Detect open Linux menus whose items expose VISIBLE without SHOWING, using valid screen bounds corroborated by an app-owned mapped X11 popup so stale closed-menu bounds do not count as an open menu.
* Retry an initially absent Linux menu surface for up to 500 ms before reporting it absent, without delaying a menu found on the first read.
