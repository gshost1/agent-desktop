### Bug Fixes

* Linux `notification-action` now marks every error after its expand press, including a target that moved when a new notification arrived, with `retry: unsafe` and look-first advice, instead of `not_delivered`; the moved entry is still never pressed. A notification mutation whose message-list open may have pressed the clock menu is unsafe to retry and keeps the shell's `list-surfaces` advice instead of "open the message list by hand and retry", which now appears only on a `not_delivered` open that pressed nothing. ([#231](https://github.com/gshost1/agent-desktop/pull/231))
