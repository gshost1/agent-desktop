### Bug Fixes

* On Windows, a `set-value` or `clear` whose classic edit does not answer `EM_REPLACESEL` in time no longer keeps its text buffer for the rest of the process: a worker thread owns the buffer until the edit reads it or the edit's thread exits, at most 8 such writes wait per process, and a write past that is refused unsent with `not_delivered`. ([#225](https://github.com/gshost1/agent-desktop/pull/225))

### Testing

* Unit tests pin the edit write dispositions (a failed or unanswered `EM_SETSEL` is `not_delivered` and sends no text, an unanswered `EM_REPLACESEL` is `delivered_unverified`), and live Win32 fixture tests cover a held edit thread's late delivery, the pending-write bound, and disabled and over-limit edits keeping `ValuePattern`. ([#225](https://github.com/gshost1/agent-desktop/pull/225))
