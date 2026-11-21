### Bug Fixes

* Keep time in reserve to close pointer-opened combo popups after choice-read or commit timeouts, preserving the original error and opener delivery disposition. Report an already-hidden Qt combo popup as closed without Escape.
* Report `popup_closed: null` when closure cannot be read and `popup_close_attempted` to distinguish whether Escape was sent.
* Give stale refs from crashed owners the same process-exit recovery suggestion as refs from exited owners.
