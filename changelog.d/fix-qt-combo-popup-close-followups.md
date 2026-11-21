### Bug Fixes

* Keep time in reserve to close pointer-opened combo popups after choice-read or commit timeouts, preserving the original error and opener delivery disposition. Report an already-hidden Qt combo popup as closed without Escape.
* Document `popup_closed: null` for unreadable state, failed Escape and a Qt pre-Escape recheck that no longer confirms visibility, and `popup_close_attempted` to distinguish whether Escape was sent.
* Give stale refs from crashed owners the same process-exit recovery suggestion as refs from exited owners.
* Document the early `popup_cleanup_budget_insufficient` refusal for pointer-only combos and Qt combo rows, and suggest increasing `--timeout-ms` to leave their respective 1000 ms and 500 ms cleanup reserves.
