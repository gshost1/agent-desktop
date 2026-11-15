### Bug Fixes

* `find --first --allow-partial` keeps the match it found outside list and table rows when the follow-up pass that reads the rows cannot finish, and reports it with `complete: true`, the same answer `find --first` gives without the flag. It used to return that pass's own incomplete observation, which could have no match at all. ([#173](https://github.com/gshost1/agent-desktop/pull/173))
