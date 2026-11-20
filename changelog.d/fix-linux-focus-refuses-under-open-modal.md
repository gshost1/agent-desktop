### Bug Fixes

* **linux:** `focus-window --window-id` on X11 now refuses a window whose modal dialog is already the active window before any activation request is sent: `ACTION_FAILED`, cause `blocked_by_modal_dialog`, `not_delivered`, `retry: safe`; before, the request was sent first, the parent was raised beneath the dialog, and the refusal was `delivered_unverified` and unsafe to retry ([#224](https://github.com/gshost1/agent-desktop/pull/224))
