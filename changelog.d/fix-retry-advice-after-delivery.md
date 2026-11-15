### Bug Fixes

* An error whose input may already have landed (`disposition.retry: "unsafe"`) no longer tells the caller to retry. On macOS, a click, right-click or other ref action whose fallback chain sent a step without a confirmed effect now says to take a new snapshot before acting again, instead of naming another mechanism such as `mouse-click --button right`; a second right-click could land in a menu that opened late. Windows `dismiss-notification`, the Linux notification expand, and the Linux restore, focus and window-write refusals sent after the request say to read the state first.
* On Windows, an `open-system-surface` or Action Center close that times out or fails a read after its accelerator, invoke or Escape was sent is now `delivered_unverified` instead of `not_delivered` with `retryable: true` or the read's own disposition; a taskbar surface awaited without input stays `not_delivered`, and so is a close whose re-raise was sent before the surface declined the foreground. On Linux, an open or close of the message list or system menu that times out or fails a read after the toggle click is `delivered_unverified` instead of `unknown` or the read's own `not_delivered`. The surface's opener toggles, so a blind retry could undo a late first press.

### Testing

* `scripts/check-retry-advice.sh`, in the pre-commit hook and CI, fails when an error built as `delivered_unverified` or `uncertain`, or passed through a helper that stamps one, carries a suggestion to retry, rerun or act again without saying to look first.
